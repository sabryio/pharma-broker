//! Parsing module for batch message processing
//!
//! Provides batch accumulation and multi-pass parsing capabilities.
//! Ported from legacy/parsing/processor.go

mod config;
mod params;
mod processor;

pub use config::{BatchConfig, MultiPassConfig, ParsePass};
pub use params::{
    BatchProcessorBuilder, BatchProcessorConfig, BatchProcessorDeps, BatchProcessorRepositories,
};
pub use processor::BatchProcessor;
use uuid::Uuid;

use crate::domain::RawMessage;
use tokio::time::Instant;

/// Message to be processed by the batch processor
#[derive(Debug, Clone)]
pub struct ParseJob {
    pub message: RawMessage,
    pub received_at: Instant,
}

impl ParseJob {
    pub fn new(message: RawMessage) -> Self {
        Self {
            message,
            received_at: Instant::now(),
        }
    }
}

/// Result of parsing a single message (job-level result)
#[derive(Debug, Clone)]
pub struct ParseJobResult {
    pub message_id: Uuid,
    pub items: Vec<crate::ai::ParsedItem>,
    pub error: Option<String>,
    pub pass: ParsePass,
}

impl ParseJobResult {
    pub fn success(message_id: Uuid, items: Vec<crate::ai::ParsedItem>, pass: ParsePass) -> Self {
        Self {
            message_id,
            items,
            error: None,
            pass,
        }
    }

    pub fn error(message_id: Uuid, error: String) -> Self {
        Self {
            message_id,
            items: vec![],
            error: Some(error),
            pass: ParsePass::Strict,
        }
    }
}

/// Statistics for the batch processor
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    pub messages_received: u64,
    pub batches_processed: u64,
    pub items_extracted: u64,
    pub errors: u64,
    pub pass2_retries: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_new() {
        let msg = RawMessage {
            id: Uuid::new_v4(),
            external_id: Some("ext-123".to_string()),
            content: "Test message".to_string(),
            sender_phone: Some("+123".to_string()),
            sender_jid: "123@s.whatsapp.net".to_string(),
            sender_name: Some("Test".to_string()),
            group_jid: "group@s.whatsapp.net".to_string(),
            group_name: "Test Group".to_string(),
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            timestamp: chrono::Utc::now(),
            processed_at: None,
            error: None,
            created_at: chrono::Utc::now(),
        };

        let _job = ParseJob::new(msg.clone());
    }

    #[test]
    fn test_parse_result_success() {
        let result = ParseJobResult::success(Uuid::new_v4(), vec![], ParsePass::Strict);
        assert!(result.error.is_none());
        assert_eq!(result.pass, ParsePass::Strict);
    }

    #[test]
    fn test_parse_result_error() {
        let result = ParseJobResult::error(Uuid::new_v4(), "AI failed".to_string());
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "AI failed");
        assert!(result.items.is_empty());
    }

    #[test]
    fn test_batch_stats_default() {
        let stats = BatchStats::default();
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.items_extracted, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.pass2_retries, 0);
    }
}
