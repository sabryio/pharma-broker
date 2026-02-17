//! Retry Queue Repository

use crate::Result;
use crate::entity::retry_queue;
use crate::traits::RetryQueueRepository;
use chrono::{Duration, Utc};
use sea_orm::*;
use std::sync::Arc;
use uuid::Uuid;

pub struct SeaOrmRetryQueueRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmRetryQueueRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl RetryQueueRepository for SeaOrmRetryQueueRepo {
    async fn enqueue(
        &self,
        raw_message_id: Uuid,
        failure_reason: retry_queue::FailureReason,
        original_error: &str,
        priority: i32,
    ) -> Result<retry_queue::Model> {
        // Check if already in queue
        if let Some(existing) = retry_queue::Entity::find()
            .filter(retry_queue::Column::RawMessageId.eq(raw_message_id))
            .filter(retry_queue::Column::Status.is_in([
                retry_queue::RetryStatus::Pending,
                retry_queue::RetryStatus::Processing,
            ]))
            .one(self.db.as_ref())
            .await?
        {
            return Ok(existing);
        }

        let now = Utc::now();
        let next_attempt = now + Duration::seconds(30); // First retry after 30 seconds

        let item = retry_queue::ActiveModel {
            id: Set(Uuid::new_v4()),
            raw_message_id: Set(raw_message_id),
            status: Set(retry_queue::RetryStatus::Pending),
            priority: Set(priority),
            attempts: Set(0),
            max_attempts: Set(3),
            failure_reason: Set(failure_reason),
            original_error: Set(original_error.to_string()),
            last_error: Set(None),
            next_attempt_at: Set(next_attempt),
            created_at: Set(now),
            updated_at: Set(now),
            completed_at: Set(None),
        };

        let result = item.insert(self.db.as_ref()).await?;
        Ok(result)
    }

    async fn get_pending(&self, limit: i64) -> Result<Vec<retry_queue::Model>> {
        let now = Utc::now();

        retry_queue::Entity::find()
            .filter(retry_queue::Column::Status.eq(retry_queue::RetryStatus::Pending))
            .filter(retry_queue::Column::NextAttemptAt.lte(now))
            .order_by_desc(retry_queue::Column::Priority)
            .order_by_asc(retry_queue::Column::NextAttemptAt)
            .limit(limit as u64)
            .all(self.db.as_ref())
            .await
            .map_err(Into::into)
    }

    async fn mark_processing(&self, id: Uuid) -> Result<retry_queue::Model> {
        let item = retry_queue::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await?
            .ok_or_else(|| crate::Error::NotFound(format!("Retry queue item {}", id)))?;

        let mut active: retry_queue::ActiveModel = item.into();
        active.status = Set(retry_queue::RetryStatus::Processing);
        active.updated_at = Set(Utc::now());

        active.update(self.db.as_ref()).await.map_err(Into::into)
    }

    async fn mark_completed(&self, id: Uuid) -> Result<retry_queue::Model> {
        let item = retry_queue::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await?
            .ok_or_else(|| crate::Error::NotFound(format!("Retry queue item {}", id)))?;

        let mut active: retry_queue::ActiveModel = item.into();
        active.status = Set(retry_queue::RetryStatus::Completed);
        active.completed_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());

        active.update(self.db.as_ref()).await.map_err(Into::into)
    }

    async fn mark_failed(&self, id: Uuid, error: &str, retry: bool) -> Result<retry_queue::Model> {
        let item = retry_queue::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await?
            .ok_or_else(|| crate::Error::NotFound(format!("Retry queue item {}", id)))?;

        let mut active: retry_queue::ActiveModel = item.clone().into();
        let new_attempts = item.attempts + 1;
        active.attempts = Set(new_attempts);
        active.last_error = Set(Some(error.to_string()));
        active.updated_at = Set(Utc::now());

        if retry && new_attempts < item.max_attempts {
            // Schedule next retry with exponential backoff
            let backoff_seconds = 30 * 2_i64.pow(new_attempts as u32);
            let next_attempt = Utc::now() + Duration::seconds(backoff_seconds);

            active.status = Set(retry_queue::RetryStatus::Pending);
            active.next_attempt_at = Set(next_attempt);
        } else {
            // Max attempts reached or not retryable
            active.status = Set(retry_queue::RetryStatus::Failed);
            active.completed_at = Set(Some(Utc::now()));
        }

        active.update(self.db.as_ref()).await.map_err(Into::into)
    }

    async fn cancel(&self, id: Uuid) -> Result<retry_queue::Model> {
        let item = retry_queue::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await?
            .ok_or_else(|| crate::Error::NotFound(format!("Retry queue item {}", id)))?;

        let mut active: retry_queue::ActiveModel = item.into();
        active.status = Set(retry_queue::RetryStatus::Cancelled);
        active.completed_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());

        active.update(self.db.as_ref()).await.map_err(Into::into)
    }

    async fn get_by_raw_message_id(
        &self,
        raw_message_id: Uuid,
    ) -> Result<Option<retry_queue::Model>> {
        retry_queue::Entity::find()
            .filter(retry_queue::Column::RawMessageId.eq(raw_message_id))
            .order_by_desc(retry_queue::Column::CreatedAt)
            .one(self.db.as_ref())
            .await
            .map_err(Into::into)
    }

    async fn count_by_status(&self, status: retry_queue::RetryStatus) -> Result<i64> {
        retry_queue::Entity::find()
            .filter(retry_queue::Column::Status.eq(status))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Into::into)
    }

    async fn count_by_failure_reason(&self, reason: retry_queue::FailureReason) -> Result<i64> {
        retry_queue::Entity::find()
            .filter(retry_queue::Column::FailureReason.eq(reason))
            .filter(retry_queue::Column::Status.is_in([
                retry_queue::RetryStatus::Pending,
                retry_queue::RetryStatus::Processing,
            ]))
            .count(self.db.as_ref())
            .await
            .map(|c| c as i64)
            .map_err(Into::into)
    }

    async fn get_stats(&self) -> Result<RetryQueueStats> {
        let pending = self
            .count_by_status(retry_queue::RetryStatus::Pending)
            .await?;
        let processing = self
            .count_by_status(retry_queue::RetryStatus::Processing)
            .await?;
        let completed = self
            .count_by_status(retry_queue::RetryStatus::Completed)
            .await?;
        let failed = self
            .count_by_status(retry_queue::RetryStatus::Failed)
            .await?;

        let circuit_breaker = self
            .count_by_failure_reason(retry_queue::FailureReason::CircuitBreaker)
            .await?;
        let network_error = self
            .count_by_failure_reason(retry_queue::FailureReason::NetworkError)
            .await?;
        let incomplete_json = self
            .count_by_failure_reason(retry_queue::FailureReason::IncompleteJson)
            .await?;

        Ok(RetryQueueStats {
            pending,
            processing,
            completed,
            failed,
            circuit_breaker,
            network_error,
            incomplete_json,
        })
    }

    async fn cleanup_old(&self, days: i64) -> Result<u64> {
        let cutoff = Utc::now() - Duration::days(days);

        let result = retry_queue::Entity::delete_many()
            .filter(retry_queue::Column::CompletedAt.is_not_null())
            .filter(retry_queue::Column::CompletedAt.lt(cutoff))
            .exec(self.db.as_ref())
            .await?;

        Ok(result.rows_affected)
    }
}

#[derive(Debug, Clone)]
pub struct RetryQueueStats {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
    pub circuit_breaker: i64,
    pub network_error: i64,
    pub incomplete_json: i64,
}
