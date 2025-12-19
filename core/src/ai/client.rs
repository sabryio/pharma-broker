//! AI Gateway Client
//!
//! HTTP client for communicating with the TypeScript AI gateway

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing;

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
    pub raw_response: Option<String>,
    pub error: Option<String>,
}

/// Request body for /ai/parse
#[derive(Debug, Serialize)]
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

    /// Parse a message using the AI gateway
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
}

/// AI client error types
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Gateway error: {0}")]
    Gateway(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
