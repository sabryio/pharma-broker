//! AI module for gateway communication
//!
//! Handles parsing messages via the TypeScript AI gateway

mod client;

pub use client::{AiClient, AiConfig, ParsedItem};
