//! MatchQueue repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::match_queue::{self, Entity as MatchQueue, QueueStatus};
use crate::traits::MatchQueueRepository;
use crate::{Error, Result};

/// SeaORM-based match queue repository
pub struct SeaOrmMatchQueueRepo {
    db: DatabaseConnection,
}

impl SeaOrmMatchQueueRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MatchQueueRepository for SeaOrmMatchQueueRepo {
    async fn enqueue(&self, request_id: &str, priority: i32) -> Result<match_queue::Model> {
        let model = match_queue::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            request_id: Set(request_id.to_string()),
            priority: Set(priority),
            status: Set(QueueStatus::Pending),
            attempts: Set(0),
            last_error: Set(None),
            next_attempt_at: Set(Utc::now()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        model.insert(&self.db).await.map_err(Error::from)
    }

    async fn fetch_batch(&self, limit: i64) -> Result<Vec<match_queue::Model>> {
        // Fetch pending items that are ready for processing
        let items = MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .filter(match_queue::Column::NextAttemptAt.lte(Utc::now()))
            .order_by_desc(match_queue::Column::Priority)
            .order_by_asc(match_queue::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await?;

        // Mark them as processing
        for item in &items {
            let mut active: match_queue::ActiveModel = item.clone().into();
            active.status = Set(QueueStatus::Processing);
            active.updated_at = Set(Utc::now());
            active.update(&self.db).await?;
        }

        Ok(items)
    }

    async fn complete(&self, id: &uuid::Uuid) -> Result<()> {
        let item = MatchQueue::find_by_id(*id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let mut active: match_queue::ActiveModel = item.into();
        active.status = Set(QueueStatus::Completed);
        active.updated_at = Set(Utc::now());
        active.update(&self.db).await?;
        Ok(())
    }

    async fn fail(&self, id: &uuid::Uuid, error: &str) -> Result<()> {
        let item = MatchQueue::find_by_id(*id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let retry_delay_secs = 60i64; // Default retry delay
        let mut active: match_queue::ActiveModel = item.clone().into();
        active.attempts = Set(item.attempts + 1);
        active.last_error = Set(Some(error.to_string()));
        active.next_attempt_at = Set(Utc::now() + chrono::Duration::seconds(retry_delay_secs));
        active.status = Set(if item.attempts >= 3 {
            QueueStatus::Failed
        } else {
            QueueStatus::Pending
        });
        active.updated_at = Set(Utc::now());
        active.update(&self.db).await?;
        Ok(())
    }

    async fn count_pending(&self) -> Result<i64> {
        MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = MatchQueue::delete_many()
            .filter(match_queue::Column::Status.eq(QueueStatus::Completed))
            .filter(match_queue::Column::UpdatedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
