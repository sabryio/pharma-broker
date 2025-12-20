//! AI module for gateway communication
//!
//! Handles parsing messages via the TypeScript AI gateway

mod circuit_breaker;
mod client;
mod token_batcher;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitOpenError, CircuitState};
pub use client::{AiClient, AiConfig, AiError, BatchParseResult, ParsedItem};
pub use token_batcher::{
    BatchMessage, TokenBatchConfig, TokenBatchStats, TokenBatchStatsSnapshot, TokenBatcher,
};
