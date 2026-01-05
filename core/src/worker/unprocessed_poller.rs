//! Unprocessed Message Poller Worker
//!
//! Polls for unprocessed messages and submits them to the BatchProcessor for AI parsing.
//! Includes intelligent backoff for failed messages to prevent retry storms.
//! Requirements: 6.1, 6.2, 7.1, 7.2

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::domain::RawMessage;
use crate::parsing::ParseJob;
use crate::repository::RawMessageRepository;

/// Prefix used to identify permanent context failures
const CONTEXT_EXCEEDED_PERMANENT_PREFIX: &str = "[PERMANENT] Context size exceeded";

/// Configuration for the UnprocessedPoller
#[derive(Debug, Clone)]
pub struct UnprocessedPollerConfig {
    /// Whether the poller is enabled
    pub enabled: bool,
    /// Interval between polls for unprocessed messages
    pub poll_interval: Duration,
    /// Maximum number of messages to fetch per poll
    pub batch_size: i64,
    /// Minimum age of failed messages before retry (backoff)
    pub failed_message_backoff: Duration,
    /// Maximum retries for failed messages before marking as permanent
    pub max_failed_retries: u32,
}

impl Default for UnprocessedPollerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval: Duration::from_secs(5),
            batch_size: 10,
            failed_message_backoff: Duration::from_secs(60), // Wait 1 minute before retrying failed messages
            max_failed_retries: 3,                           // After 3 failures, mark as permanent
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
            failed_message_backoff: Duration::from_secs(
                std::env::var("BATCH_PROCESSOR_FAILED_BACKOFF_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60),
            ),
            max_failed_retries: std::env::var("BATCH_PROCESSOR_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
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
    /// Implements intelligent backoff for failed messages
    /// Requirements: 6.1, 6.2
    async fn poll_and_submit(&self) {
        let backoff_secs = self.config.failed_message_backoff.as_secs() as i64;

        // Use the new get_pending_processing method that handles backoff at the DB level
        match self
            .raw_message_repo
            .get_pending_processing(
                self.config.batch_size,
                backoff_secs,
                CONTEXT_EXCEEDED_PERMANENT_PREFIX,
            )
            .await
        {
            Ok(messages) if !messages.is_empty() => {
                let mut submitted = 0;
                let mut skipped_max_retries = 0;

                for msg in messages {
                    // Check if this message has exceeded max retries
                    if let Some(ref error) = msg.error {
                        let retry_count = Self::extract_retry_count(error);
                        if retry_count >= self.config.max_failed_retries {
                            // Mark as permanent failure
                            warn!(
                                msg_id = %msg.id,
                                retry_count = retry_count,
                                max_retries = self.config.max_failed_retries,
                                "Message exceeded max retries, marking as permanent failure"
                            );
                            let permanent_error = format!(
                                "{}: Exceeded {} retry attempts. Last error: {}",
                                CONTEXT_EXCEEDED_PERMANENT_PREFIX,
                                self.config.max_failed_retries,
                                error
                            );
                            let _ = self
                                .raw_message_repo
                                .mark_processed(msg.id, Some(&permanent_error))
                                .await;
                            skipped_max_retries += 1;
                            continue;
                        }
                    }

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
                    } else {
                        submitted += 1;
                    }
                }

                if submitted > 0 || skipped_max_retries > 0 {
                    info!(
                        submitted = submitted,
                        skipped_max_retries = skipped_max_retries,
                        "📬 Processed pending messages batch"
                    );
                }
            }
            Ok(_) => {
                // No messages found, continue polling
            }
            Err(e) => {
                warn!(error = %e, "Failed to poll for pending messages");
            }
        }
    }

    /// Extract retry count from error message
    /// Looks for patterns like "[retry:N]" or counts "Context size" occurrences
    fn extract_retry_count(error: &str) -> u32 {
        // Look for explicit retry count marker
        if let Some(start) = error.find("[retry:")
            && let Some(end) = error[start..].find(']')
            && let Ok(count) = error[start + 7..start + end].parse::<u32>()
        {
            return count;
        }

        // Count context exceeded errors as implicit retry indicator
        let context_errors = error.matches("Context size").count()
            + error.matches("context_length_exceeded").count();

        context_errors as u32
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
        assert_eq!(config.failed_message_backoff, Duration::from_secs(60));
        assert_eq!(config.max_failed_retries, 3);
    }

    #[test]
    fn test_config_values() {
        let config = UnprocessedPollerConfig {
            enabled: false,
            poll_interval: Duration::from_secs(10),
            batch_size: 20,
            failed_message_backoff: Duration::from_secs(120),
            max_failed_retries: 5,
        };
        assert!(!config.enabled);
        assert_eq!(config.poll_interval, Duration::from_secs(10));
        assert_eq!(config.batch_size, 20);
        assert_eq!(config.failed_message_backoff, Duration::from_secs(120));
        assert_eq!(config.max_failed_retries, 5);
    }

    #[test]
    fn test_extract_retry_count_explicit() {
        assert_eq!(
            UnprocessedPoller::extract_retry_count("[retry:3] Some error"),
            3
        );
        assert_eq!(
            UnprocessedPoller::extract_retry_count("Error [retry:5] happened"),
            5
        );
        assert_eq!(
            UnprocessedPoller::extract_retry_count("[retry:0] First try"),
            0
        );
    }

    #[test]
    fn test_extract_retry_count_implicit() {
        assert_eq!(
            UnprocessedPoller::extract_retry_count("Context size exceeded"),
            1
        );
        assert_eq!(
            UnprocessedPoller::extract_retry_count("Context size exceeded; Context size exceeded"),
            2
        );
        assert_eq!(
            UnprocessedPoller::extract_retry_count("context_length_exceeded error"),
            1
        );
    }

    #[test]
    fn test_extract_retry_count_none() {
        assert_eq!(
            UnprocessedPoller::extract_retry_count("Some random error"),
            0
        );
        assert_eq!(UnprocessedPoller::extract_retry_count(""), 0);
    }

    #[test]
    fn test_permanent_failure_prefix() {
        let error = format!(
            "{}: Maximum chunking depth exceeded",
            CONTEXT_EXCEEDED_PERMANENT_PREFIX
        );
        assert!(error.starts_with(CONTEXT_EXCEEDED_PERMANENT_PREFIX));
    }
}
