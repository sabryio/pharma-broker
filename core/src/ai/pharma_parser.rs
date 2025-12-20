//! Pharma AI Parser
//!
//! High-level wrapper around ai-client for pharma-specific parsing,
//! with circuit breaker and retry support.

use std::sync::Arc;

use ai_client::{Client, ClientConfig, Error as ClientError};
use tracing::{error, info, warn};

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use super::pharma_prompts::{SYSTEM_PROMPT, build_user_prompt_with_mappings};
use super::pharma_types::{ParseResult, ParsedItem};
use super::token_batcher::{BatchMessage, TokenBatchConfig, TokenBatcher};

/// Configuration for the PharmaParser
#[derive(Clone, Default, Debug)]
pub struct PharmaParserConfig {
    /// AI client configuration
    pub client: ClientConfig,
    /// Circuit breaker configuration
    pub circuit_breaker: CircuitBreakerConfig,
    /// Token batch configuration
    pub token_batch: TokenBatchConfig,
}

impl PharmaParserConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            client: ClientConfig::from_env(),
            circuit_breaker: CircuitBreakerConfig::from_env(),
            token_batch: TokenBatchConfig::default(),
        }
    }
}

/// Error type for PharmaParser
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Circuit breaker open")]
    CircuitOpen,
    #[error("AI client error: {0}")]
    Client(#[from] ClientError),
    #[error("Parse error: {0}")]
    Parse(String),
}

impl ParseError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ParseError::CircuitOpen => false,
            ParseError::Client(e) => e.is_retryable(),
            ParseError::Parse(_) => false,
        }
    }
}

/// Pharma AI parser with circuit breaker and batch support
pub struct PharmaParser {
    client: Client,
    circuit_breaker: Arc<CircuitBreaker>,
    token_batcher: TokenBatcher,
}

impl PharmaParser {
    /// Create a new parser with the given configuration
    pub fn new(config: PharmaParserConfig) -> Self {
        Self {
            client: Client::new(config.client),
            circuit_breaker: Arc::new(CircuitBreaker::new(config.circuit_breaker)),
            token_batcher: TokenBatcher::new(config.token_batch),
        }
    }

    /// Create a parser from environment variables
    pub fn from_env() -> Self {
        Self::new(PharmaParserConfig::from_env())
    }

    /// Parse a single message with optional medication mappings
    pub async fn parse(
        &self,
        content: &str,
        sender_name: Option<&str>,
        group_name: Option<&str>,
        reply_to: Option<&str>,
        mappings: Option<&[String]>,
    ) -> Result<Vec<ParsedItem>, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting request");
            return Err(ParseError::CircuitOpen);
        }

        let user_prompt =
            build_user_prompt_with_mappings(content, sender_name, group_name, reply_to, mappings);

        let result: Result<ParseResult, ClientError> = self
            .client
            .generate_object_with_system(SYSTEM_PROMPT, &user_prompt)
            .await;

        match result {
            Ok(parse_result) => {
                self.circuit_breaker.record_success();
                info!(items = parse_result.items.len(), "AI parsing complete");
                Ok(parse_result.items)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                error!(error = %e, "AI parsing failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Parse a batch of messages using token-aware batching
    pub async fn parse_batch(&self, messages: Vec<BatchMessage>) -> Vec<BatchParseResult> {
        // Split into token-aware batches
        let batches = self.token_batcher.split_into_batches(messages);
        let mut results = Vec::new();

        for batch in batches {
            for msg in batch {
                let result = self.parse(&msg.content, None, None, None, None).await;
                results.push(BatchParseResult {
                    message_id: msg.id,
                    result,
                });
            }
        }

        results
    }

    /// Generate an embedding for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting embed request");
            return Err(ParseError::CircuitOpen);
        }

        match self.client.generate_embedding(text).await {
            Ok(embedding) => {
                self.circuit_breaker.record_success();
                Ok(embedding)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                error!(error = %e, "Embedding generation failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Generate embeddings for multiple texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ParseError> {
        // Check circuit breaker
        if !self.circuit_breaker.allow_request() {
            warn!("Circuit breaker open, rejecting embed request");
            return Err(ParseError::CircuitOpen);
        }

        match self.client.generate_embeddings(texts).await {
            Ok(embeddings) => {
                self.circuit_breaker.record_success();
                Ok(embeddings)
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                error!(error = %e, "Embedding generation failed");
                Err(ParseError::Client(e))
            }
        }
    }

    /// Get circuit breaker state
    pub fn circuit_state(&self) -> super::circuit_breaker::CircuitState {
        self.circuit_breaker.state()
    }

    /// Get token batcher statistics
    pub fn batcher_stats(&self) -> super::token_batcher::TokenBatchStatsSnapshot {
        self.token_batcher.stats()
    }
}

/// Result for a single message in a batch
pub struct BatchParseResult {
    pub message_id: String,
    pub result: Result<Vec<ParsedItem>, ParseError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = PharmaParser::new(PharmaParserConfig::default());
        // Just test it compiles
        let _ = parser;
    }
}
