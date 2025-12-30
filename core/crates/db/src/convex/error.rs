//! Convex-specific error types

use thiserror::Error;

/// Errors that can occur when interacting with Convex
#[derive(Debug, Error)]
pub enum ConvexError {
    #[error("Convex connection error: {0}")]
    Connection(String),

    #[error("Convex query '{function}' failed: {message}")]
    Query { function: String, message: String },

    #[error("Convex mutation '{function}' failed: {message}")]
    Mutation { function: String, message: String },

    #[error("Convex action '{function}' failed: {message}")]
    Action { function: String, message: String },

    #[error("Serialization error in {context}: {message}")]
    Serialization { context: String, message: String },

    #[error("Deserialization error in {context}: {message}")]
    Deserialization { context: String, message: String },

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<ConvexError> for crate::Error {
    fn from(err: ConvexError) -> Self {
        match err {
            ConvexError::NotFound { entity_type, id } => {
                crate::Error::NotFound(format!("{} with id {}", entity_type, id))
            }
            other => crate::Error::Internal(other.to_string()),
        }
    }
}
