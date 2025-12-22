//! Error types for PharmaBroker Core

use thiserror::Error;

/// Result type alias for PharmaBroker operations
pub type Result<T> = std::result::Result<T, Error>;

/// Core error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("SeaORM database error: {0}")]
    SeaOrm(#[from] pharma_db::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    pub fn not_found(entity: impl Into<String>) -> Self {
        Self::NotFound(entity.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}
