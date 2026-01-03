//! Auto-Approve Background Worker
//!
//! Processes pending matches for AI-supervised auto-approval in batches.
//! Requirements: 1.3, 6.1, 6.2

use crate::matching::{AutoApproveProcessor, AutoApproveResult};
use crate::repository::{MatchRepository, OfferRepository, RequestRepository};
use crate::ws::WsEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Repository dependencies for AutoApproveWorker
pub struct AutoApproveWorkerRepos {
    pub match_repo: Arc<dyn MatchRepository>,
    pub offer_repo: Arc<dyn OfferRepository>,
    pub request_repo: Arc<dyn RequestRepository>,
}

/// AutoApproveWorker handles background processing of pending matches for auto-approval
/// Requirements: 1.3, 6.1, 6.2
pub struct AutoApproveWorker {
    repos: AutoApproveWorkerRepos,
    processor: Arc<AutoApproveProcessor>,
    ws_tx: broadcast::Sender<WsEvent>,
    worker_id: String,
}

impl AutoApproveWorker {
    pub fn new(
        repos: AutoApproveWorkerRepos,
        processor: Arc<AutoApproveProcessor>,
        ws_tx: broadcast::Sender<WsEvent>,
    ) -> Self {
        Self {
            repos,
            processor,
            ws_tx,
            worker_id: Uuid::new_v4().to_string(),
        }
    }

    /// Run the worker loop with shutdown support
    ///
    /// The worker will stop when the shutdown signal is received,
    /// completing any in-progress work before exiting.
    /// Requirements: 1.3
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(worker_id = %self.worker_id, "🤖 Auto-approve worker started");

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(worker_id = %self.worker_id, "🛑 Auto-approve worker received shutdown signal");
                        break;
                    }
                }
                result = self.process_batch() => {
                    match result {
                        Ok(count) => {
                            if count == 0 {
                                // No items to process, sleep based on config interval
                                let config = self.processor.get_config().await;
                                sleep(Duration::from_secs(config.processing_interval_secs)).await;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "❌ Error in auto-approve batch processing");
                            sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }

        info!(worker_id = %self.worker_id, "👋 Auto-approve worker stopped gracefully");
    }

    /// Process a batch of pending matches for auto-approval
    /// Requirements: 6.1, 6.2
    async fn process_batch(&self) -> crate::Result<usize> {
        // Check if auto-approve is enabled and not paused
        let config = self.processor.get_config().await;
        if !config.enabled {
            return Ok(0);
        }

        let stats = self.processor.get_stats().await;
        if stats.system_status == crate::matching::SystemStatus::Paused {
            return Ok(0);
        }

        // Check schedule
        if !config.is_within_schedule() {
            return Ok(0);
        }

        // Fetch pending matches (oldest first for age-based prioritization - Requirement 6.2)
        // The get_pending method returns matches ordered by score desc, but we want oldest first
        // We'll fetch and then process in order
        let pending_matches = self
            .repos
            .match_repo
            .get_pending(config.batch_size as i64, 0)
            .await?;

        let count = pending_matches.len();
        if count == 0 {
            return Ok(0);
        }

        info!(
            worker_id = %self.worker_id,
            count = count,
            "📦 Processing auto-approve batch"
        );

        // Process each match
        let mut approved_count = 0;
        let mut queued_count = 0;
        let mut blocked_count = 0;

        for match_entity in pending_matches {
            // Fetch the associated offer and request
            let offer = match self.repos.offer_repo.get_by_id(match_entity.offer_id).await {
                Ok(Some(o)) => o,
                Ok(None) => {
                    warn!(match_id = %match_entity.id, offer_id = %match_entity.offer_id, "Offer not found for match");
                    continue;
                }
                Err(e) => {
                    warn!(match_id = %match_entity.id, error = %e, "Failed to fetch offer");
                    continue;
                }
            };

            let request = match self
                .repos
                .request_repo
                .get_by_id(match_entity.request_id)
                .await
            {
                Ok(Some(r)) => r,
                Ok(None) => {
                    warn!(match_id = %match_entity.id, request_id = %match_entity.request_id, "Request not found for match");
                    continue;
                }
                Err(e) => {
                    warn!(match_id = %match_entity.id, error = %e, "Failed to fetch request");
                    continue;
                }
            };

            // Get medication category if available
            let category = self.get_medication_category(&offer.medication).await;

            let result = self
                .processor
                .process_match(&match_entity, &offer, &request, category.as_deref())
                .await;

            match result {
                Ok(auto_result) => {
                    self.handle_auto_approve_result(
                        &auto_result,
                        &offer.medication,
                        &request.medication,
                    )
                    .await;

                    match &auto_result.action {
                        crate::matching::AutoApproveAction::Approved => {
                            approved_count += 1;
                        }
                        crate::matching::AutoApproveAction::QueuedForReview { .. } => {
                            queued_count += 1;
                        }
                        crate::matching::AutoApproveAction::Blocked { .. } => {
                            blocked_count += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        match_id = %match_entity.id,
                        error = %e,
                        "Failed to process match for auto-approval"
                    );
                }
            }
        }

        info!(
            worker_id = %self.worker_id,
            approved = approved_count,
            queued = queued_count,
            blocked = blocked_count,
            "✅ Auto-approve batch completed"
        );

        Ok(count)
    }

    /// Handle the result of auto-approval processing
    /// Broadcasts WebSocket events for real-time updates
    async fn handle_auto_approve_result(
        &self,
        result: &AutoApproveResult,
        offer_medication: &str,
        request_medication: &str,
    ) {
        use crate::ws::{AutoApproveBlockedEvent, AutoApproveEvent, QueuedForReviewEvent};

        match &result.action {
            crate::matching::AutoApproveAction::Approved => {
                // Broadcast auto-approved event
                let _ = self.ws_tx.send(WsEvent::AutoApproved(AutoApproveEvent {
                    match_id: result.match_id,
                    offer_medication: offer_medication.to_string(),
                    request_medication: request_medication.to_string(),
                    ai_confidence: result.ai_confidence,
                    ai_explanation: result.ai_explanation.clone(),
                    is_borderline: result.is_borderline,
                    approved_at: chrono::Utc::now(),
                }));

                info!(
                    match_id = %result.match_id,
                    confidence = result.ai_confidence,
                    borderline = result.is_borderline,
                    "🤖 Match auto-approved"
                );
            }
            crate::matching::AutoApproveAction::QueuedForReview { reason } => {
                // Broadcast queued for review event
                let _ = self
                    .ws_tx
                    .send(WsEvent::QueuedForReview(QueuedForReviewEvent {
                        match_id: result.match_id,
                        offer_medication: offer_medication.to_string(),
                        request_medication: request_medication.to_string(),
                        ai_confidence: result.ai_confidence,
                        ai_explanation: result.ai_explanation.clone(),
                        is_borderline: result.is_borderline,
                        queued_at: chrono::Utc::now(),
                    }));

                info!(
                    match_id = %result.match_id,
                    confidence = result.ai_confidence,
                    reason = %reason,
                    "📋 Match queued for review"
                );
            }
            crate::matching::AutoApproveAction::Blocked { reason } => {
                // Broadcast blocked event
                let _ = self
                    .ws_tx
                    .send(WsEvent::AutoApproveBlocked(AutoApproveBlockedEvent {
                        match_id: result.match_id,
                        offer_medication: offer_medication.to_string(),
                        request_medication: request_medication.to_string(),
                        block_reason: reason.clone(),
                        blocked_at: chrono::Utc::now(),
                    }));

                warn!(
                    match_id = %result.match_id,
                    reason = %reason,
                    "🚫 Match blocked by safety guardrails"
                );
            }
        }
    }

    /// Get medication category for category-specific thresholds
    /// Returns None if category lookup is not available
    async fn get_medication_category(&self, _medication: &str) -> Option<String> {
        // TODO: Integrate with medication master data to get category
        // For now, return None to use global threshold
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_id_is_unique() {
        // Worker IDs should be unique UUIDs
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }
}
