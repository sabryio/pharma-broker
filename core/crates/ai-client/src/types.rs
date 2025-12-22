//! OpenAI-compatible API types

use serde::{Deserialize, Serialize};

/// Chat completion request
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// Message role
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Response format specification
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResponseFormat {
    #[serde(rename = "json_schema")]
    JsonSchema { json_schema: JsonSchemaSpec },
    #[serde(rename = "json_object")]
    JsonObject,
}

/// JSON schema specification for structured output
#[derive(Debug, Clone, Serialize)]
pub struct JsonSchemaSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Chat completion response
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Response choice
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

/// Response message
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    pub role: Role,
    pub content: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl ChatMessage {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

// =============================================================================
// Embedding Types
// =============================================================================

/// Embedding request
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
}

/// Embedding input variants
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    Single(String),
    Batch(Vec<String>),
}

/// Embedding response
#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,
    #[serde(default)]
    pub usage: Option<EmbeddingUsage>,
}

/// Single embedding data
#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingData {
    pub index: u32,
    pub embedding: Vec<f32>,
}

/// Embedding token usage
#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}
