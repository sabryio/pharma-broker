//! Unprocessed Message Poller Worker
//!
//! Polls for unprocessed messages and submits them to the BatchProcessor for AI parsing.
//! Requirements: 6.1, 6.2, 7.1, 7.2

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::domain::RawMessage;
use crate::parsing::ParseJob;
use crate::repository::RawMessageRepository;

/// Configuration for the UnprocessedPoller
#[derive(Debug, Clone)]
pub struct UnprocessedPollerConfig {
    /// Whether the poller is enabled
    pub enabled: bool,
    /// Interval between polls for unprocessed messages
    pub poll_interval: Duration,
    /// Maximum number of messages to fetch per poll
    pub batch_size: i64,
}

impl Default for UnprocessedPollerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval: Duration::from_secs(5),
            batch_size: 10,
        }
    }
}

impl UnprocessedPollerConfig {
    /// Create configuration from environment variables
    /// Requirements: 6.5
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("BATCH_PROCESSOR_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            poll_interval: Duration::from_secs(
                std::env::var("BATCH_PROCESSOR_POLL_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5),
            ),
            batch_size: std::env::var("BATCH_PROCESSOR_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// Polls for unprocessed messages and submits them to the batch processor
/// Requirements: 6.1, 6.2, 7.1, 7.2
pub struct UnprocessedPoller {
    config: UnprocessedPollerConfig,
    raw_message_repo: Arc<dyn RawMessageRepository>,
    batch_processor_tx: mpsc::Sender<ParseJob>,
}

impl UnprocessedPoller {
    /// Create a new UnprocessedPoller
    pub fn new(
        config: UnprocessedPollerConfig,
        raw_message_repo: Arc<dyn RawMessageRepository>,
        batch_processor_tx: mpsc::Sender<ParseJob>,
    ) -> Self {
        Self {
            config,
            raw_message_repo,
            batch_processor_tx,
        }
    }

    /// Run the polling loop with graceful shutdown support
    /// Requirements: 6.1, 6.2, 6.4
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        if !self.config.enabled {
            info!("📭 Unprocessed poller disabled, not starting");
            return;
        }

        info!(
            poll_interval_secs = self.config.poll_interval.as_secs(),
            batch_size = self.config.batch_size,
            "📬 Unprocessed message poller started"
        );

        let mut ticker = interval(self.config.poll_interval);

        loop {
            tokio::select! {
                // Shutdown signal - complete gracefully
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("📭 Unprocessed poller received shutdown signal");
                        break;
                    }
                }
                // Poll for unprocessed messages
                _ = ticker.tick() => {
                    self.poll_and_submit().await;
                }
            }
        }

        info!("👋 Unprocessed poller stopped gracefully");
    }

    /// Poll for unprocessed messages and submit them to the batch processor
    /// Requirements: 6.1, 6.2
    async fn poll_and_submit(&self) {
        match self
            .raw_message_repo
            .get_unprocessed(self.config.batch_size)
            .await
        {
            Ok(messages) if !messages.is_empty() => {
                info!(count = messages.len(), "📬 Found unprocessed messages");
                for msg in messages {
                    // Convert from Model to domain type
                    let raw_message = RawMessage {
                        id: msg.id,
                        external_id: msg.external_id,
                        content: msg.content,
                        group_id: msg.group_id,
                        participant_id: msg.participant_id,
                        reply_to_id: msg.reply_to_id,
                        reply_to_content: msg.reply_to_content,
                        reply_to_sender: msg.reply_to_sender,
                        timestamp: msg.timestamp,
                        processed_at: msg.processed_at,
                        error: msg.error,
                        created_at: msg.created_at,
                    };

                    let job = ParseJob::new(raw_message);
                    if let Err(e) = self.batch_processor_tx.send(job).await {
                        error!(error = %e, msg_id = %msg.id, "Failed to submit message to batch processor");
                    }
                }
            }
            Ok(_) => {
                // No messages found, continue polling
            }
            Err(e) => {
                warn!(error = %e, "Failed to poll for unprocessed messages");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = UnprocessedPollerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.poll_interval, Duration::from_secs(5));
        assert_eq!(config.batch_size, 10);
    }

    #[test]
    fn test_config_values() {
        let config = UnprocessedPollerConfig {
            enabled: false,
            poll_interval: Duration::from_secs(10),
            batch_size: 20,
        };
        assert!(!config.enabled);
        assert_eq!(config.poll_interval, Duration::from_secs(10));
        assert_eq!(config.batch_size, 20);
    }
}
