//! Convex ReviewQueueRepository implementation

use std::sync::Arc;

use async_trait::async_trait;

use super::client::ConvexClient;
use super::error::ConvexError;
use crate::Result;
use crate::convex_args;
use crate::params::UpdateReviewStatusParams;
use crate::traits::{ReviewQueueModel, ReviewQueueRepository, ReviewQueueStats, ReviewStatus};

/// Convex-backed review queue repository
pub struct ConvexReviewQueueRepo {
    client: Arc<ConvexClient>,
}

impl ConvexReviewQueueRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ReviewQueueRepository for ConvexReviewQueueRepo {
    async fn save(&self, item: &ReviewQueueModel) -> Result<ReviewQueueModel> {
        // ReviewQueueModel: id (Uuid), raw_message_id, ai_result, confidence, reason, status, reviewed_by, review_notes, created_at, reviewed_at
        let _id: String = self
            .client
            .mutation(
                "reviewQueue:add",
                convex_args! {
                    "rawMessageId" => &item.raw_message_id,
                    "aiResult" => &item.ai_result,
                    "confidence" => item.confidence,
                    "reason" => &item.reason
                },
            )
            .await?;

        Ok(item.clone())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<ReviewQueueModel>> {
        self.client
            .query("reviewQueue:get", convex_args! { "id" => id })
            .await
    }

    async fn get_pending(&self, limit: i64, _offset: i64) -> Result<Vec<ReviewQueueModel>> {
        self.client
            .query("reviewQueue:listPending", convex_args! { "limit" => limit })
            .await
    }

    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        _offset: i64,
    ) -> Result<Vec<ReviewQueueModel>> {
        self.client
            .query(
                "reviewQueue:listByStatus",
                convex_args! {
                    "status" => format!("{:?}", status),
                    "limit" => limit
                },
            )
            .await
    }

    async fn update_status(
        &self,
        params: UpdateReviewStatusParams<'_>,
    ) -> Result<ReviewQueueModel> {
        // UpdateReviewStatusParams: id, status, reviewed_by, notes
        let fn_name = match params.status {
            ReviewStatus::Approved => "reviewQueue:complete",
            ReviewStatus::Rejected | ReviewStatus::Skipped => "reviewQueue:dismiss",
            _ => "reviewQueue:complete",
        };

        self.client
            .mutation_void(
                fn_name,
                convex_args! {
                    "id" => params.id,
                    "reviewedBy" => params.reviewed_by,
                    "notes" => params.notes
                },
            )
            .await?;

        self.get_by_id(params.id).await?.ok_or_else(|| {
            ConvexError::NotFound {
                entity_type: "ReviewQueue".to_string(),
                id: params.id.to_string(),
            }
            .into()
        })
    }

    async fn get_stats(&self) -> Result<ReviewQueueStats> {
        let stats: serde_json::Value = self
            .client
            .query("reviewQueue:stats", convex_args!())
            .await?;

        Ok(ReviewQueueStats {
            pending: stats["pending"].as_i64().unwrap_or(0),
            approved: stats["approved"].as_i64().unwrap_or(0),
            rejected: stats["rejected"].as_i64().unwrap_or(0),
            skipped: stats["skipped"].as_i64().unwrap_or(0),
        })
    }

    async fn count_pending(&self) -> Result<i64> {
        let stats = self.get_stats().await?;
        Ok(stats.pending)
    }

    async fn exists_for_message(&self, raw_message_id: &str) -> Result<bool> {
        let items: Vec<ReviewQueueModel> = self
            .client
            .query(
                "reviewQueue:getByRawMessage",
                convex_args! {
                    "rawMessageId" => raw_message_id
                },
            )
            .await?;
        Ok(!items.is_empty())
    }
}
