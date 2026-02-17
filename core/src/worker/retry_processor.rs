//! Retry Processor Worker
//!
//! Processes failed messages from the retry queue and attempts to reprocess them.

use crate::ai::PharmaParser;
use crate::repository::{
    OfferRepository, RawMessageRepository, RequestRepository, RetryQueueRepository,
};
use pharma_db::entity::retry_queue::FailureReason;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::Result;

/// Repository dependencies for RetryProcessor
pub struct RetryProcessorRepos {
    pub retry_queue: Arc<dyn RetryQueueRepository>,
    pub raw_message: Arc<dyn RawMessageRepository>,
    pub offer: Arc<dyn OfferRepository>,
    pub request: Arc<dyn RequestRepository>,
}

/// RetryProcessor handles background processing of failed messages
pub struct RetryProcessor {
    repos: RetryProcessorRepos,
    ai_client: Arc<PharmaParser>,
    worker_id: String,
}

impl RetryProcessor {
    pub fn new(repos: RetryProcessorRepos, ai_client: Arc<PharmaParser>) -> Self {
        Self {
            repos,
            ai_client,
            worker_id: Uuid::new_v4().to_string(),
        }
    }

    /// Run the processor loop with shutdown support
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(worker_id = %self.worker_id, "🚀 Retry processor started");

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(worker_id = %self.worker_id, "🛑 Retry processor received shutdown signal");
                        break;
                    }
                }
                result = self.process_batch() => {
                    match result {
                        Ok(count) => {
                            if count == 0 {
                                // No items, sleep for 30 seconds
                                sleep(Duration::from_secs(30)).await;
                            } else {
                                // Processed some items, check again soon
                                sleep(Duration::from_secs(5)).await;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "❌ Error processing retry batch");
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                }
            }
        }

        info!(worker_id = %self.worker_id, "👋 Retry processor stopped gracefully");
    }

    /// Process a batch of pending retry items
    async fn process_batch(&self) -> Result<usize> {
        let items = self.repos.retry_queue.get_pending(10).await?;
        let count = items.len();

        if count > 0 {
            info!(
                worker_id = %self.worker_id,
                count = count,
                "📦 Processing retry queue batch"
            );
        }

        for item in items {
            // Mark as processing
            if let Err(e) = self.repos.retry_queue.mark_processing(item.id).await {
                error!(
                    error = %e,
                    item_id = %item.id,
                    "Failed to mark retry item as processing"
                );
                continue;
            }

            // Get the raw message
            let raw_message = match self
                .repos
                .raw_message
                .get_by_id(item.raw_message_id)
                .await?
            {
                Some(msg) => msg,
                None => {
                    warn!(
                        item_id = %item.id,
                        raw_message_id = %item.raw_message_id,
                        "Raw message not found, cancelling retry"
                    );
                    let _ = self.repos.retry_queue.cancel(item.id).await;
                    continue;
                }
            };

            info!(
                item_id = %item.id,
                raw_message_id = %item.raw_message_id,
                attempts = item.attempts + 1,
                max_attempts = item.max_attempts,
                failure_reason = ?item.failure_reason,
                "🔄 Retrying failed message"
            );

            // Attempt to reprocess the message
            // For now, we'll just try AI parsing again
            // In a full implementation, this would integrate with the full message processing pipeline
            match self
                .ai_client
                .parse(
                    &raw_message.content,
                    None, // sender_name
                    "",   // group_name
                    raw_message.reply_to_content.as_deref(),
                    None, // medication_mappings
                )
                .await
            {
                Ok(_result) => {
                    info!(
                        item_id = %item.id,
                        raw_message_id = %item.raw_message_id,
                        "✅ Retry successful"
                    );

                    // Mark as completed
                    if let Err(e) = self.repos.retry_queue.mark_completed(item.id).await {
                        error!(error = %e, item_id = %item.id, "Failed to mark retry as completed");
                    }

                    // Clear the error on the raw message
                    if let Err(e) = self
                        .repos
                        .raw_message
                        .mark_processed(item.raw_message_id, None)
                        .await
                    {
                        error!(error = %e, raw_message_id = %item.raw_message_id, "Failed to clear raw message error");
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        item_id = %item.id,
                        raw_message_id = %item.raw_message_id,
                        attempts = item.attempts + 1,
                        "❌ Retry failed"
                    );

                    // Determine if we should retry again
                    let should_retry = match item.failure_reason {
                        FailureReason::CircuitBreaker
                        | FailureReason::NetworkError
                        | FailureReason::Timeout => {
                            // Transient errors - retry
                            true
                        }
                        FailureReason::IncompleteJson | FailureReason::ParseError => {
                            // Might be transient (chunked response) - retry
                            true
                        }
                        FailureReason::Other => {
                            // Unknown error - retry cautiously
                            true
                        }
                    };

                    // Mark as failed (will schedule next retry if should_retry is true)
                    if let Err(e) = self
                        .repos
                        .retry_queue
                        .mark_failed(item.id, &e.to_string(), should_retry)
                        .await
                    {
                        error!(error = %e, item_id = %item.id, "Failed to mark retry as failed");
                    }
                }
            }
        }

        Ok(count)
    }
}
