//! Generic OpenAI-compatible AI Client
//!
//! A reusable Rust client for OpenAI-compatible APIs with structured output support.
//!
//! # Features
//! - Generic structured output via `schemars::JsonSchema`
//! - Retry with exponential backoff
//! - Circuit breaker pattern
//! - Compatible with OpenAI, Docker Model Runner, vLLM, and other OpenAI-compatible APIs
//!
//! # Example
//! ```rust,ignore
//! use ai_client::{Client, ClientConfig};
//! use schemars::JsonSchema;
//! use serde::Deserialize;
//!
//! #[derive(JsonSchema, Deserialize)]
//! struct MyOutput {
//!     items: Vec<String>,
//! }
//!
//! let client = Client::new(ClientConfig::default());
//! let result: MyOutput = client.generate_object("Extract items from: hello world").await?;
//! ```

mod client;
mod error;
mod prompts;
mod retry;
mod schema;
mod types;

pub use client::{AIContext, Client, ClientConfig};
pub use error::Error;
pub use prompts::PromptBuilder;
pub use retry::RetryConfig;
pub use schema::generate_schema;
pub use types::*;
