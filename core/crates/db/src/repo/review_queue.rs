//! ReviewQueue repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::review_queue::{self, Entity as ReviewQueue, ReviewStatus};
use crate::traits::{ReviewQueueRepository, ReviewQueueStats};
use crate::{Error, Result};

/// SeaORM-based review queue repository
pub struct SeaOrmReviewQueueRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmReviewQueueRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ReviewQueueRepository for SeaOrmReviewQueueRepo {
    async fn save(&self, model: &review_queue::Model) -> Result<review_queue::Model> {
        let active: review_queue::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<review_queue::Model>> {
        ReviewQueue::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<review_queue::Model>> {
        ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .order_by_asc(review_queue::Column::Confidence)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
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
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn update_status(
        &self,
        params: crate::params::UpdateReviewStatusParams,
    ) -> Result<review_queue::Model> {
        let item = ReviewQueue::find_by_id(params.id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Review item not found: {}", params.id)))?;

        let mut active: review_queue::ActiveModel = item.into();
        active.status = Set(params.status);
        active.reviewed_by = Set(Some(params.reviewed_by.to_string()));
        active.review_notes = Set(params.notes.map(|s| s.to_string()));
        active.reviewed_at = Set(Some(Utc::now()));
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn get_stats(&self) -> Result<ReviewQueueStats> {
        let pending = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Pending))
            .count(&*self.db)
            .await? as i64;

        let approved = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Approved))
            .count(&*self.db)
            .await? as i64;

        let rejected = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Rejected))
            .count(&*self.db)
            .await? as i64;

        let skipped = ReviewQueue::find()
            .filter(review_queue::Column::Status.eq(ReviewStatus::Skipped))
            .count(&*self.db)
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
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn exists_for_message(&self, raw_message_id: Uuid) -> Result<bool> {
        let count = ReviewQueue::find()
            .filter(review_queue::Column::RawMessageId.eq(raw_message_id))
            .count(&*self.db)
            .await?;
        Ok(count > 0)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::params::UpdateReviewStatusParams;
    use crate::testing::{TestDb, new_test_raw_message, new_test_review_queue};
    use sea_orm::EntityTrait;

    async fn create_review_queue_item(db: &TestDb) -> review_queue::Model {
        use crate::entity::raw_message;

        // Create raw message
        let msg = new_test_raw_message();
        let msg_id = msg.id.clone().unwrap();
        raw_message::Entity::insert(msg)
            .exec(&*db.db)
            .await
            .expect("Insert raw message");

        // Create review queue item
        let item = new_test_review_queue(&msg_id);
        let item_id = item.id.clone().unwrap();
        review_queue::Entity::insert(item)
            .exec(&*db.db)
            .await
            .expect("Insert review item");

        review_queue::Entity::find_by_id(item_id)
            .one(&*db.db)
            .await
            .expect("Find review item")
            .expect("Review item should exist")
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        let item = create_review_queue_item(&db).await;

        let found = repo.get_by_id(&item.id.to_string()).await.expect("GetByID");
        assert!(found.is_some(), "Should find review item");
        assert_eq!(found.unwrap().id, item.id);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        let found = repo
            .get_by_id(&uuid::Uuid::new_v4().to_string())
            .await
            .expect("GetByID");
        assert!(found.is_none(), "Should return None");
    }

    #[tokio::test]
    async fn test_get_pending() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        create_review_queue_item(&db).await;
        create_review_queue_item(&db).await;
        create_review_queue_item(&db).await;

        let pending = repo.get_pending(10, 0).await.expect("GetPending");
        assert_eq!(pending.len(), 3, "Should have 3 pending items");
        assert!(pending.iter().all(|i| i.status == ReviewStatus::Pending));
    }

    #[tokio::test]
    async fn test_count_pending() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        create_review_queue_item(&db).await;
        create_review_queue_item(&db).await;

        assert_eq!(repo.count_pending().await.expect("CountPending"), 2);
    }

    #[tokio::test]
    async fn test_exists_for_message() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        let item = create_review_queue_item(&db).await;

        assert!(
            repo.exists_for_message(&item.raw_message_id)
                .await
                .expect("Exists"),
            "Should exist"
        );
        assert!(
            !repo
                .exists_for_message("non-existent")
                .await
                .expect("Exists"),
            "Should not exist"
        );
    }

    #[tokio::test]
    async fn test_get_stats() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        // Create 3 pending items
        create_review_queue_item(&db).await;
        create_review_queue_item(&db).await;
        create_review_queue_item(&db).await;

        let stats = repo.get_stats().await.expect("GetStats");
        assert_eq!(stats.pending, 3, "Should have 3 pending");
        assert_eq!(stats.approved, 0, "Should have 0 approved");
    }

    #[tokio::test]
    async fn test_update_status() {
        let db = TestDb::new().await;
        let repo = SeaOrmReviewQueueRepo::new(db.db.clone());

        let item = create_review_queue_item(&db).await;
        assert_eq!(item.status, ReviewStatus::Pending);

        let updated = repo
            .update_status(UpdateReviewStatusParams {
                id: &item.id.to_string(),
                status: ReviewStatus::Approved,
                reviewed_by: "test-reviewer",
                notes: Some("Looks good"),
            })
            .await
            .expect("UpdateStatus");

        assert_eq!(updated.status, ReviewStatus::Approved);
        assert_eq!(updated.reviewed_by, Some("test-reviewer".to_string()));
        assert!(updated.reviewed_at.is_some());
    }
}
