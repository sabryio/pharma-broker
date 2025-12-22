//! ReviewQueue Service - AI parse results requiring human review

use sea_orm::*;

use crate::entity::review_queue::{self, Entity as ReviewQueue, ReviewStatus};
use crate::{Error, Result};

/// Service for review queue operations
pub struct ReviewQueueService;

impl ReviewQueueService {
    /// Save a new review item
    pub async fn save(
        db: &DatabaseConnection,
        model: review_queue::ActiveModel,
    ) -> Result<review_queue::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get item by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: uuid::Uuid,
    ) -> Result<Option<review_queue::Model>> {
        ReviewQueue::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get pending reviews
    pub async fn get_pending(db: &DatabaseConnection) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .order_by_asc(review_queue::Column::Confidence)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get pending reviews with limit
    pub async fn get_pending_batch(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .order_by_asc(review_queue::Column::Confidence)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Approve a review item
    pub async fn approve(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        reviewer: &str,
        notes: Option<&str>,
    ) -> Result<review_queue::Model> {
        let item = ReviewQueue::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Review item not found: {}", id)))?;

        let mut active: review_queue::ActiveModel = item.into();
        active.status = Set(ReviewStatus::Approved);
        active.reviewed_by = Set(Some(reviewer.to_string()));
        active.review_notes = Set(notes.map(|s| s.to_string()));
        active.reviewed_at = Set(Some(chrono::Utc::now()));
        active.update(db).await.map_err(Error::from)
    }

    /// Reject a review item
    pub async fn reject(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        reviewer: &str,
        notes: Option<&str>,
    ) -> Result<review_queue::Model> {
        let item = ReviewQueue::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Review item not found: {}", id)))?;

        let mut active: review_queue::ActiveModel = item.into();
        active.status = Set(ReviewStatus::Rejected);
        active.reviewed_by = Set(Some(reviewer.to_string()));
        active.review_notes = Set(notes.map(|s| s.to_string()));
        active.reviewed_at = Set(Some(chrono::Utc::now()));
        active.update(db).await.map_err(Error::from)
    }

    /// Skip a review item
    pub async fn skip(
        db: &DatabaseConnection,
        id: uuid::Uuid,
        reviewer: &str,
    ) -> Result<review_queue::Model> {
        let item = ReviewQueue::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Review item not found: {}", id)))?;

        let mut active: review_queue::ActiveModel = item.into();
        active.status = Set(ReviewStatus::Skipped);
        active.reviewed_by = Set(Some(reviewer.to_string()));
        active.reviewed_at = Set(Some(chrono::Utc::now()));
        active.update(db).await.map_err(Error::from)
    }

    /// Count pending reviews
    pub async fn count_pending(db: &DatabaseConnection) -> Result<u64> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Get reviews by raw message ID
    pub async fn get_by_raw_message(
        db: &DatabaseConnection,
        raw_message_id: &str,
    ) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::RawMessageId.eq(raw_message_id))
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Delete old reviewed items
    pub async fn cleanup_old(db: &DatabaseConnection, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let result = ReviewQueue::delete_many()
            .filter(review_queue::Column::Status.ne(ReviewStatus::Pending))
            .filter(review_queue::Column::ReviewedAt.lt(cutoff))
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
        assert_eq!(ReviewStatus::default(), ReviewStatus::Pending);
    }
}
