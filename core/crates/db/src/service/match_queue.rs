//! MatchQueue Service - Async matching queue management

use sea_orm::{prelude::Expr, *};

use crate::entity::match_queue::{self, Entity as MatchQueue, QueueStatus};
use crate::{Error, Result};

/// Service for match queue operations
pub struct MatchQueueService;

impl MatchQueueService {
    /// Enqueue a request for matching
    pub async fn enqueue(
        db: &DatabaseConnection,
        model: match_queue::ActiveModel,
    ) -> Result<match_queue::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get item by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: uuid::Uuid,
    ) -> Result<Option<match_queue::Model>> {
        MatchQueue::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Peek at next pending item without claiming
    pub async fn peek(db: &DatabaseConnection) -> Result<Option<match_queue::Model>> {
        MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .filter(match_queue::Column::NextAttemptAt.lte(chrono::Utc::now()))
            .order_by_desc(match_queue::Column::Priority)
            .order_by_asc(match_queue::Column::CreatedAt)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Dequeue next item (claim for processing)
    pub async fn dequeue(db: &DatabaseConnection) -> Result<Option<match_queue::Model>> {
        let item = Self::peek(db).await?;
        if let Some(item) = item {
            let mut active: match_queue::ActiveModel = item.into();
            active.status = Set(QueueStatus::Processing);
            active.updated_at = Set(chrono::Utc::now());
            let updated = active.update(db).await?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    /// Mark item as completed
    pub async fn complete(db: &DatabaseConnection, id: uuid::Uuid) -> Result<match_queue::Model> {
        let item = MatchQueue::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let mut active: match_queue::ActiveModel = item.into();
        active.status = Set(QueueStatus::Completed);
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await.map_err(Error::from)
    }

    /// Mark item as failed with retry
    pub async fn fail(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        error: &str,
        retry_delay_secs: i64,
    ) -> Result<match_queue::Model> {
        let item = MatchQueue::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let mut active: match_queue::ActiveModel = item.clone().into();
        active.attempts = Set(item.attempts + 1);
        active.last_error = Set(Some(error.to_string()));
        active.next_attempt_at =
            Set(chrono::Utc::now() + chrono::Duration::seconds(retry_delay_secs));
        active.status = Set(if item.attempts >= 3 {
            QueueStatus::Failed
        } else {
            QueueStatus::Pending
        });
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await.map_err(Error::from)
    }

    /// Get pending items count
    pub async fn count_pending(db: &DatabaseConnection) -> Result<u64> {
        MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Get processing items count
    pub async fn count_processing(db: &DatabaseConnection) -> Result<u64> {
        MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Processing))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Reset stuck processing items (for recovery)
    pub async fn reset_stuck(db: &DatabaseConnection, timeout_secs: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(timeout_secs);
        let result = MatchQueue::update_many()
            .col_expr(
                match_queue::Column::Status,
                Expr::value(QueueStatus::Pending),
            )
            .col_expr(
                match_queue::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(match_queue::Column::Status.eq(QueueStatus::Processing))
            .filter(match_queue::Column::UpdatedAt.lt(cutoff))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete completed items older than N days
    pub async fn cleanup_old(db: &DatabaseConnection, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let result = MatchQueue::delete_many()
            .filter(match_queue::Column::Status.eq(QueueStatus::Completed))
            .filter(match_queue::Column::UpdatedAt.lt(cutoff))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_default() {
        assert_eq!(QueueStatus::default(), QueueStatus::Pending);
    }
}
