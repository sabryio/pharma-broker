//! AI module for parsing messages
//!
//! Provides the PharmaParser for parsing pharmaceutical messages using AI,
//! along with utilities like circuit breaker and token batching.

mod circuit_breaker;
mod pharma_parser;
mod pharma_prompts;
mod pharma_types;
mod token_batcher;

// Circuit breaker for resilient network calls
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitOpenError, CircuitState};

// Token batching for efficient AI calls
pub use token_batcher::{
    BatchMessage, TokenBatchConfig, TokenBatchStats, TokenBatchStatsSnapshot, TokenBatcher,
};

// New direct AI client
pub use pharma_parser::{BatchParseResult, ParseError, PharmaParser, PharmaParserConfig};
pub use pharma_prompts::{SYSTEM_PROMPT, build_user_prompt_with_mappings};
pub use pharma_types::{ItemType, ParseResult, ParsedItem};

// Re-export ai-client crate for advanced usage
pub use ai_client::{Client as GenericClient, ClientConfig, generate_schema};
