//! Convex MatchQueueRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{MatchQueueModel, MatchQueueRepository, QueueStatus};

/// Convex-backed match queue repository
pub struct ConvexMatchQueueRepo {
    client: Arc<ConvexClient>,
}

impl ConvexMatchQueueRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MatchQueueRepository for ConvexMatchQueueRepo {
    async fn enqueue(&self, request_id: &str, priority: i32) -> Result<MatchQueueModel> {
        let _id: String = self
            .client
            .mutation(
                "matchQueue:add",
                convex_args! {
                    "requestId" => request_id,
                    "priority" => priority
                },
            )
            .await?;

        // MatchQueueModel: id (Uuid), request_id, status, priority, attempts, last_error, next_attempt_at, created_at, updated_at
        let now = Utc::now();
        Ok(MatchQueueModel {
            id: Uuid::new_v4(),
            request_id: request_id.to_string(),
            status: QueueStatus::Pending,
            priority,
            attempts: 0,
            last_error: None,
            next_attempt_at: now,
            created_at: now,
            updated_at: now,
        })
    }

    async fn fetch_batch(&self, limit: i64) -> Result<Vec<MatchQueueModel>> {
        self.client
            .query("matchQueue:listPending", convex_args! { "limit" => limit })
            .await
    }

    async fn complete(&self, id: &Uuid) -> Result<()> {
        self.client
            .mutation_void(
                "matchQueue:markProcessed",
                convex_args! { "id" => id.to_string() },
            )
            .await
    }

    async fn fail(&self, id: &Uuid, error: &str) -> Result<()> {
        self.client
            .mutation_void(
                "matchQueue:fail",
                convex_args! {
                    "id" => id.to_string(),
                    "error" => error
                },
            )
            .await
    }

    async fn count_pending(&self) -> Result<i64> {
        let stats: serde_json::Value = self
            .client
            .query("matchQueue:stats", convex_args!())
            .await?;
        Ok(stats["pending"].as_i64().unwrap_or(0))
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        self.client
            .mutation(
                "matchQueue:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
