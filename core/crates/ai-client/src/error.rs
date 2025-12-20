//! Error types for the AI client

use thiserror::Error;

/// AI client error types
#[derive(Debug, Error)]
pub enum Error {
    /// Network errors (connection failed, timeout)
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// API errors (non-2xx status codes)
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    /// JSON parsing errors
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// Schema validation errors
    #[error("Schema error: {0}")]
    Schema(String),

    /// Retry exhausted
    #[error("Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted { attempts: u32, last_error: String },

    /// Circuit breaker open
    #[error("Circuit breaker open")]
    CircuitOpen,

    /// Empty response from model
    #[error("Empty response from model")]
    EmptyResponse,
}

impl Error {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Network(_) => true,
            Error::Api { status, .. } => *status >= 500 || *status == 429,
            Error::Parse(_) => false,
            Error::Schema(_) => false,
            Error::RetryExhausted { .. } => false,
            Error::CircuitOpen => false,
            Error::EmptyResponse => true,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
