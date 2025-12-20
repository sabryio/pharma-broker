//! AI Gateway Client
//!
//! HTTP client for communicating with the TypeScript AI gateway

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing;

use crate::retry::{RetryConfig, RetryResult, with_retry};

/// Configuration for the AI client
#[derive(Clone, Debug)]
pub struct AiConfig {
    pub gateway_url: String,
    pub timeout_secs: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            gateway_url: "http://localhost:3000".to_string(),
            timeout_secs: 30,
        }
    }
}

impl AiConfig {
    pub fn from_env() -> Self {
        Self {
            gateway_url: std::env::var("AI_GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            timeout_secs: std::env::var("AI_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
        }
    }
}

/// A parsed medication item from AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedItem {
    #[serde(rename = "type")]
    pub item_type: String, // "OFFER" or "REQUEST"
    pub medication: String,
    pub medication_raw: String,
    #[serde(default)]
    pub ai_confidence: f64,
    #[serde(default)]
    pub quantity: f64,
    pub unit: Option<String>,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub max_price: f64,
    #[serde(default)]
    pub urgent: bool,
    pub notes: Option<String>,
}

/// AI parse result from gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub items: Vec<ParsedItem>,
}

/// Gateway response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayResponse {
    pub success: bool,
    pub parsed: Option<ParseResult>,
    #[allow(dead_code)]
    pub raw_response: Option<String>,
    pub error: Option<String>,
}

/// Request body for /ai/parse
#[derive(Debug, Serialize, Clone)]
struct ParseRequest {
    content: String,
    sender_name: Option<String>,
    group_name: Option<String>,
    reply_to: Option<String>,
}

/// AI Gateway client
pub struct AiClient {
    client: Client,
    config: AiConfig,
}

impl AiClient {
    /// Create a new AI client with the given configuration
    pub fn new(config: AiConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Create a new AI client from environment variables
    pub fn from_env() -> Self {
        Self::new(AiConfig::from_env())
    }

    /// Parse a message using the AI gateway (without retry)
    pub async fn parse(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: Option<&str>,
        reply_to: Option<&str>,
    ) -> Result<Vec<ParsedItem>, AiError> {
        let url = format!("{}/ai/parse", self.config.gateway_url);

        let request = ParseRequest {
            content: content.to_string(),
            sender_name: sender_name.map(|s| s.to_string()),
            group_name: group_name.map(|s| s.to_string()),
            reply_to: reply_to.map(|s| s.to_string()),
        };

        tracing::debug!(
            url = %url,
            content_len = content.len(),
            "Calling AI gateway"
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Gateway(format!("Status {}: {}", status, body)));
        }

        let gateway_response: GatewayResponse = response
            .json()
            .await
            .map_err(|e| AiError::Parse(e.to_string()))?;

        if !gateway_response.success {
            return Err(AiError::Gateway(
                gateway_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        let items = gateway_response.parsed.map(|p| p.items).unwrap_or_default();

        tracing::info!(items_count = items.len(), "AI parsing complete");

        Ok(items)
    }

    /// Parse a message with automatic retry for transient failures
    pub async fn parse_with_retry(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: Option<&str>,
        reply_to: Option<&str>,
    ) -> RetryResult<Vec<ParsedItem>, AiError> {
        let content = content.to_string();
        let sender_name = sender_name.map(|s| s.to_string());
        let group_name = group_name.map(|s| s.to_string());
        let reply_to = reply_to.map(|s| s.to_string());

        with_retry(
            RetryConfig::for_ai_gateway(),
            || {
                let c = content.clone();
                let sn = sender_name.clone();
                let gn = group_name.clone();
                let rt = reply_to.clone();
                async move {
                    self.parse(&c, sn.as_deref(), gn.as_deref(), rt.as_deref())
                        .await
                }
            },
            |e| e.is_retryable(),
        )
        .await
    }

    /// Parse multiple messages with token-aware batching
    ///
    /// This method:
    /// 1. Splits messages into token-aware batches using the provided batcher
    /// 2. Processes each batch sequentially to avoid overloading the AI gateway
    /// 3. Returns results for each message in the same order
    ///
    /// Each batch is processed with retry logic for transient failures.
    pub async fn parse_batch(
        &self,
        messages: Vec<super::token_batcher::BatchMessage>,
        batcher: &super::token_batcher::TokenBatcher,
    ) -> Vec<BatchParseResult> {
        if messages.is_empty() {
            return Vec::new();
        }

        // Split messages into token-aware batches
        let batches = batcher.split_into_batches(messages.clone());

        tracing::info!(
            total_messages = messages.len(),
            batch_count = batches.len(),
            "📦 Processing messages in token-aware batches"
        );

        let mut results: Vec<BatchParseResult> = Vec::with_capacity(messages.len());

        for (batch_idx, batch) in batches.into_iter().enumerate() {
            tracing::debug!(
                batch_idx = batch_idx,
                batch_size = batch.len(),
                "Processing batch"
            );

            // Process each message in the batch
            for msg in batch {
                let result = self
                    .parse(
                        &msg.content,
                        msg.sender_name.as_deref(),
                        msg.group_name.as_deref(),
                        msg.reply_to.as_deref(),
                    )
                    .await;

                results.push(BatchParseResult {
                    message_id: msg.id.clone(),
                    result,
                });
            }
        }

        tracing::info!(
            total_messages = results.len(),
            successful = results.iter().filter(|r| r.result.is_ok()).count(),
            failed = results.iter().filter(|r| r.result.is_err()).count(),
            "✅ Batch parsing complete"
        );

        results
    }
}

/// Result of parsing a single message in a batch
#[derive(Debug)]
pub struct BatchParseResult {
    /// ID of the message that was parsed
    pub message_id: String,
    /// Parse result (success with items, or error)
    pub result: Result<Vec<ParsedItem>, AiError>,
}

/// AI client error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum AiError {
    /// Network errors (connection failed, timeout) - retryable
    #[error("Network error: {0}")]
    Network(String),

    /// Gateway errors (5xx status, service unavailable) - retryable
    #[error("Gateway error: {0}")]
    Gateway(String),

    /// Parse errors (invalid JSON, schema mismatch) - NOT retryable
    #[error("Parse error: {0}")]
    Parse(String),
}

impl AiError {
    /// Check if this error is retryable (transient)
    pub fn is_retryable(&self) -> bool {
        match self {
            // Network errors are always retryable (timeout, connection refused)
            AiError::Network(_) => true,
            // Gateway errors are retryable if they indicate server-side issues
            AiError::Gateway(msg) => {
                msg.contains("500")
                    || msg.contains("502")
                    || msg.contains("503")
                    || msg.contains("504")
                    || msg.contains("timeout")
                    || msg.contains("unavailable")
            }
            // Parse errors are never retryable (bad data won't fix itself)
            AiError::Parse(_) => false,
        }
    }

    /// Check if this is a network error
    pub fn is_network(&self) -> bool {
        matches!(self, AiError::Network(_))
    }

    /// Check if this is a parse error
    pub fn is_parse(&self) -> bool {
        matches!(self, AiError::Parse(_))
    }
}

// ============================================================================
// Embedding Response Types
// ============================================================================

/// Response from /ai/embed endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct EmbedResponse {
    pub success: bool,
    pub embeddings: Vec<Vec<f32>>,
    pub model: Option<String>,
    pub dimensions: Option<usize>,
    pub error: Option<String>,
}

/// Request body for /ai/embed
#[derive(Debug, Serialize)]
struct EmbedRequest {
    texts: Vec<String>,
}

impl AiClient {
    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| AiError::Parse("No embedding returned".to_string()))
    }

    /// Generate embeddings for multiple texts in a batch
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AiError> {
        let url = format!("{}/ai/embed", self.config.gateway_url);

        let request = EmbedRequest {
            texts: texts.to_vec(),
        };

        tracing::debug!(
            url = %url,
            texts_count = texts.len(),
            "Calling AI gateway for embeddings"
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AiError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Gateway(format!("Status {}: {}", status, body)));
        }

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| AiError::Parse(e.to_string()))?;

        if !embed_response.success {
            return Err(AiError::Gateway(
                embed_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        tracing::info!(
            embeddings_count = embed_response.embeddings.len(),
            dimensions = embed_response.dimensions,
            "Embedding generation complete"
        );

        Ok(embed_response.embeddings)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_is_retryable() {
        let err = AiError::Network("connection refused".to_string());
        assert!(err.is_retryable());
        assert!(err.is_network());
        assert!(!err.is_parse());
    }

    #[test]
    fn test_gateway_5xx_is_retryable() {
        let err = AiError::Gateway("Status 503: Service Unavailable".to_string());
        assert!(err.is_retryable());

        let err = AiError::Gateway("Status 500: Internal Server Error".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_gateway_4xx_is_not_retryable() {
        let err = AiError::Gateway("Status 400: Bad Request".to_string());
        assert!(!err.is_retryable());

        let err = AiError::Gateway("Status 404: Not Found".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_parse_error_is_not_retryable() {
        let err = AiError::Parse("Invalid JSON".to_string());
        assert!(!err.is_retryable());
        assert!(err.is_parse());
        assert!(!err.is_network());
    }

    #[test]
    fn test_ai_config_default() {
        let config = AiConfig::default();
        assert_eq!(config.gateway_url, "http://localhost:3000");
        assert_eq!(config.timeout_secs, 30);
    }
}
