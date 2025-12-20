//! AI module for gateway communication
//!
//! Handles parsing messages via the TypeScript AI gateway

mod circuit_breaker;
mod client;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitOpenError, CircuitState};
pub use client::{AiClient, AiConfig, AiError, ParsedItem};
