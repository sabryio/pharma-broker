//! ReviewQueue repository implementation

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::*;

use crate::entity::review_queue::{self, Entity as ReviewQueue, ReviewStatus};
use crate::traits::{ReviewQueueRepository, ReviewQueueStats};
use crate::{Error, Result};

/// SeaORM-based review queue repository
pub struct SeaOrmReviewQueueRepo {
    db: DatabaseConnection,
}

impl SeaOrmReviewQueueRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ReviewQueueRepository for SeaOrmReviewQueueRepo {
    async fn save(&self, model: &review_queue::Model) -> Result<review_queue::Model> {
        let active: review_queue::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<review_queue::Model>> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| Error::Validation(format!("Invalid UUID: {}", id)))?;
        ReviewQueue::find_by_id(uuid)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .order_by_asc(review_queue::Column::Confidence)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(status))
            .order_by_desc(review_queue::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn update_status(
        &self,
        params: crate::params::UpdateReviewStatusParams<'_>,
    ) -> Result<review_queue::Model> {
        let uuid = uuid::Uuid::parse_str(params.id)
            .map_err(|_| Error::Validation(format!("Invalid UUID: {}", params.id)))?;

        let item = ReviewQueue::find_by_id(uuid)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Review item not found: {}", params.id)))?;

        let mut active: review_queue::ActiveModel = item.into();
        active.status = Set(params.status);
        active.reviewed_by = Set(Some(params.reviewed_by.to_string()));
        active.review_notes = Set(params.notes.map(|s| s.to_string()));
        active.reviewed_at = Set(Some(Utc::now()));
        active.update(&self.db).await.map_err(Error::from)
    }

    async fn get_stats(&self) -> Result<ReviewQueueStats> {
        let pending = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .count(&self.db)
            .await? as i64;

        let approved = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Approved))
            .count(&self.db)
            .await? as i64;

        let rejected = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Rejected))
            .count(&self.db)
            .await? as i64;

        let skipped = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Skipped))
            .count(&self.db)
            .await? as i64;

        Ok(ReviewQueueStats {
            pending,
            approved,
            rejected,
            skipped,
        })
    }

    async fn count_pending(&self) -> Result<i64> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn exists_for_message(&self, raw_message_id: &str) -> Result<bool> {
        let count = ReviewQueue::find()
            .filter(review_queue::Column::RawMessageId.eq(raw_message_id))
            .count(&self.db)
            .await?;
        Ok(count > 0)
    }
}
