use crate::repository::{
    AuditLogRepository, FeedbackRepository, MatchQueueRepository, MatchRepository, OfferRepository,
    RequestRepository,
};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::Result;
use crate::domain::{
    AuditAction, AuditLog, ConfidenceBand, EntityType, Match as MatchEntity, MatchQueueItem,
    MatchStatus, Offer, Request,
};
use crate::matching::{MatchAction, MatchingEngine};
use crate::repository::FeedbackModel;
use crate::ws::WsEvent;

/// Repository dependencies for MatchProcessor
pub struct MatchProcessorRepos {
    pub match_queue: Arc<dyn MatchQueueRepository>,
    pub offer: Arc<dyn OfferRepository>,
    pub request: Arc<dyn RequestRepository>,
    pub match_repo: Arc<dyn MatchRepository>,
    pub audit_log: Arc<dyn AuditLogRepository>,
    pub feedback: Arc<dyn FeedbackRepository>,
}

/// MatchProcessor handles the background processing of match queue items
pub struct MatchProcessor {
    repos: MatchProcessorRepos,
    matching_engine: Arc<MatchingEngine>,
    ws_tx: broadcast::Sender<WsEvent>,
    worker_id: String,
}

impl MatchProcessor {
    pub fn new(
        repos: MatchProcessorRepos,
        matching_engine: Arc<MatchingEngine>,
        ws_tx: broadcast::Sender<WsEvent>,
    ) -> Self {
        Self {
            repos,
            matching_engine,
            ws_tx,
            worker_id: Uuid::new_v4().to_string(),
        }
    }

    /// Run the processor loop with shutdown support
    ///
    /// The processor will stop when the shutdown signal is received,
    /// completing any in-progress work before exiting.
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        info!(worker_id = %self.worker_id, "🚀 Match processor started");

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!(worker_id = %self.worker_id, "🛑 Match processor received shutdown signal");
                        break;
                    }
                }
                result = self.process_batch() => {
                    match result {
                        Ok(count) => {
                            if count == 0 {
                                // No items, sleep for a bit
                                sleep(Duration::from_secs(1)).await;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "❌ Error processing match batch");
                            sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }

        info!(worker_id = %self.worker_id, "👋 Match processor stopped gracefully");
    }

    /// Process a batch of pending items
    async fn process_batch(&self) -> Result<usize> {
        let items = self.repos.match_queue.fetch_batch(10).await?;
        let count = items.len();

        if count > 0 {
            info!(worker_id = %self.worker_id, count = count, "📦 Processing match queue batch");
        }

        for item in items {
            self.process_item(item).await;
        }

        Ok(count)
    }

    /// Process a single match queue item
    async fn process_item(&self, item: MatchQueueItem) {
        let request_id = item.request_id;

        // 1. Fetch the request
        let request = match self.repos.request.get_by_id(request_id).await {
            Ok(Some(req)) => req,
            Ok(None) => {
                warn!(request_id = %request_id, "Request not found, marking failed");
                let _ = self
                    .repos
                    .match_queue
                    .fail(item.id, "Request not found")
                    .await;
                return;
            }
            Err(e) => {
                error!(error = %e, request_id = %request_id, "Failed to fetch request");
                let _ = self.repos.match_queue.fail(item.id, &e.to_string()).await;
                return;
            }
        };

        // 2. Fetch active offers
        let active_offers = match self.repos.offer.get_active(100, 0).await {
            Ok(offers) => offers,
            Err(e) => {
                error!(error = %e, "Failed to fetch active offers");
                let _ = self
                    .repos
                    .match_queue
                    .fail(item.id, "Failed to fetch offers")
                    .await;
                return;
            }
        };

        // 3. Perform matching
        let mut matches_created = 0;

        for offer in active_offers {
            // Skip if already matched
            if let Ok(exists) = self.repos.match_repo.exists(offer.id, request.id).await
                && exists
            {
                continue;
            }

            // Calculate similarity (Logic ported from server.rs)
            let med_score = match (&offer.content_embedding, &request.content_embedding) {
                (Some(o), Some(r)) => {
                    crate::matching::cosine_similarity(o.as_slice(), r.as_slice()).unwrap_or(0.0)
                }
                _ => self.fallback_similarity(&offer, &request),
            };

            // Score match
            let (score, action) = self
                .matching_engine
                .score_match(&offer, &request, med_score, Some(&request.source_phone))
                .await;

            // Check if actionable
            if score.confidence != ConfidenceBand::None && action != MatchAction::Ignore {
                // Determine status based on action
                let (status, confirmed_at) = if action == MatchAction::AutoConfirm {
                    (MatchStatus::Confirmed, Some(Utc::now()))
                } else {
                    (MatchStatus::Pending, None)
                };

                // Create Match
                let match_entity = MatchEntity {
                    id: Uuid::new_v4(),
                    offer_id: offer.id,
                    request_id: request.id,
                    score: score.total,
                    reasoning: Some(score.breakdown.clone()),
                    matched_by: Some(format!("worker:{}", self.worker_id)),
                    status,
                    created_at: Utc::now(),
                    confirmed_at,
                    notes: None,
                };

                if let Err(e) = self.repos.match_repo.save(&match_entity).await {
                    error!(error = %e, "Failed to save match");
                } else {
                    matches_created += 1;

                    // Notify
                    let _ = self.ws_tx.send(WsEvent::NewMatch(match_entity.clone()));

                    // Create feedback record for auto-confirmed matches
                    if action == MatchAction::AutoConfirm {
                        let feedback =
                            FeedbackModel::confirmed(match_entity.id, "system:auto", score.total);
                        if let Err(e) = self.repos.feedback.save(&feedback).await {
                            error!(error = %e, match_id = %match_entity.id, "Failed to save auto-confirm feedback");
                        } else {
                            info!(match_id = %match_entity.id, score = %score.total, "✅ Auto-confirmed match with feedback");
                        }
                    }

                    // Audit Log
                    let audit_action = if action == MatchAction::AutoConfirm {
                        AuditAction::MatchAutoConfirmed
                    } else {
                        AuditAction::MatchCreated
                    };
                    let audit_log =
                        AuditLog::system(audit_action, EntityType::Match, match_entity.id)
                            .with_details(serde_json::json!({
                                "score": score.total,
                                "action": format!("{:?}", action),
                                "auto_confirmed": action == MatchAction::AutoConfirm
                            }));
                    let _ = self.repos.audit_log.save(&audit_log).await;
                }
            }
        }

        // 4. Mark completed
        if let Err(e) = self.repos.match_queue.complete(item.id).await {
            error!(error = %e, item_id = %item.id, "Failed to complete queue item");
        } else {
            info!(
                item_id = %item.id,
                request_id = %request_id,
                matches = matches_created,
                "✅ Match queue item completed"
            );
        }
    }

    fn fallback_similarity(&self, offer: &Offer, request: &Request) -> f64 {
        crate::matching::medication_similarity(&offer.medication, &request.medication)
    }
}
