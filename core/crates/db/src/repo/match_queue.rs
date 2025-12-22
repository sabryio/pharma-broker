//! MatchQueue repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::match_queue::{self, Entity as MatchQueue, QueueStatus};
use crate::traits::MatchQueueRepository;
use crate::{Error, Result};

/// SeaORM-based match queue repository
pub struct SeaOrmMatchQueueRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmMatchQueueRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
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
        model.insert(&*self.db).await.map_err(Error::from)
    }

    async fn fetch_batch(&self, limit: i64) -> Result<Vec<match_queue::Model>> {
        // Fetch pending items that are ready for processing
        let items = MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .filter(match_queue::Column::NextAttemptAt.lte(Utc::now()))
            .order_by_desc(match_queue::Column::Priority)
            .order_by_asc(match_queue::Column::CreatedAt)
            .limit(limit as u64)
            .all(&*self.db)
            .await?;

        // Mark them as processing
        for item in &items {
            let mut active: match_queue::ActiveModel = item.clone().into();
            active.status = Set(QueueStatus::Processing);
            active.updated_at = Set(Utc::now());
            active.update(&*self.db).await?;
        }

        Ok(items)
    }

    async fn complete(&self, id: &uuid::Uuid) -> Result<()> {
        let item = MatchQueue::find_by_id(*id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let mut active: match_queue::ActiveModel = item.into();
        active.status = Set(QueueStatus::Completed);
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await?;
        Ok(())
    }

    async fn fail(&self, id: &uuid::Uuid, error: &str) -> Result<()> {
        let item = MatchQueue::find_by_id(*id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Queue item not found: {}", id)))?;

        let retry_delay_secs = 60i64; // Default retry delay
        let mut active: match_queue::ActiveModel = item.clone().into();
        let new_attempts = item.attempts + 1;
        active.attempts = Set(new_attempts);
        active.last_error = Set(Some(error.to_string()));
        active.next_attempt_at = Set(Utc::now() + chrono::Duration::seconds(retry_delay_secs));
        active.status = Set(if new_attempts >= 3 {
            QueueStatus::Failed
        } else {
            QueueStatus::Pending
        });
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await?;
        Ok(())
    }

    async fn count_pending(&self) -> Result<i64> {
        MatchQueue::find()
            .filter(match_queue::Column::Status.eq(QueueStatus::Pending))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = MatchQueue::delete_many()
            .filter(match_queue::Column::Status.eq(QueueStatus::Completed))
            .filter(match_queue::Column::UpdatedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_group, new_test_raw_message, new_test_request};
    use sea_orm::EntityTrait;

    /// Helper to create a request and return its ID for queue operations
    async fn create_request(db: &TestDb) -> String {
        use crate::entity::{group, raw_message, request};

        // Create group
        let group_am = new_test_group("test-group@g.us", "Test Group", true);
        group::Entity::insert(group_am).exec(&*db.db).await.ok();

        // Create raw message
        let msg = new_test_raw_message();
        let msg_id = msg.id.clone().unwrap();
        raw_message::Entity::insert(msg)
            .exec(&*db.db)
            .await
            .expect("Insert msg");

        // Create request
        let req = new_test_request(&msg_id);
        let req_id = req.id.clone().unwrap();
        request::Entity::insert(req)
            .exec(&*db.db)
            .await
            .expect("Insert request");

        req_id
    }

    #[tokio::test]
    async fn test_enqueue() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchQueueRepo::new(db.db.clone());

        let req_id = create_request(&db).await;

        let item = repo.enqueue(&req_id, 5).await.expect("Enqueue");
        assert_eq!(item.request_id, req_id);
        assert_eq!(item.priority, 5);
        assert_eq!(item.status, QueueStatus::Pending);
        assert_eq!(item.attempts, 0);
    }

    #[tokio::test]
    async fn test_count_pending() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchQueueRepo::new(db.db.clone());

        let req1 = create_request(&db).await;
        let req2 = create_request(&db).await;

        repo.enqueue(&req1, 1).await.expect("Enqueue 1");
        repo.enqueue(&req2, 2).await.expect("Enqueue 2");

        assert_eq!(repo.count_pending().await.expect("Count"), 2);
    }

    #[tokio::test]
    async fn test_fetch_batch() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchQueueRepo::new(db.db.clone());

        let req1 = create_request(&db).await;
        let req2 = create_request(&db).await;
        let req3 = create_request(&db).await;

        repo.enqueue(&req1, 1).await.expect("Enqueue 1");
        repo.enqueue(&req2, 5).await.expect("Enqueue 2"); // Higher priority
        repo.enqueue(&req3, 3).await.expect("Enqueue 3");

        let batch = repo.fetch_batch(2).await.expect("Fetch batch");
        assert_eq!(batch.len(), 2, "Should fetch 2 items");
        // Highest priority first
        assert_eq!(batch[0].priority, 5, "First should be highest priority");

        // Items should now be in Processing status
        // Remaining pending should be 1
        assert_eq!(repo.count_pending().await.expect("Count"), 1);
    }

    #[tokio::test]
    async fn test_complete() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchQueueRepo::new(db.db.clone());

        let req_id = create_request(&db).await;
        let item = repo.enqueue(&req_id, 1).await.expect("Enqueue");

        repo.complete(&item.id).await.expect("Complete");

        // Check status changed
        let found = match_queue::Entity::find_by_id(item.id)
            .one(&*db.db)
            .await
            .expect("Find")
            .expect("Should exist");
        assert_eq!(found.status, QueueStatus::Completed);
    }

    #[tokio::test]
    async fn test_fail_retries() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchQueueRepo::new(db.db.clone());

        let req_id = create_request(&db).await;
        let item = repo.enqueue(&req_id, 1).await.expect("Enqueue");

        // First failure - should still be pending (retry)
        repo.fail(&item.id, "Error 1").await.expect("Fail 1");
        let found = match_queue::Entity::find_by_id(item.id)
            .one(&*db.db)
            .await
            .expect("Find")
            .unwrap();
        assert_eq!(found.attempts, 1);
        assert_eq!(
            found.status,
            QueueStatus::Pending,
            "Should still be pending after 1 failure"
        );

        // Second failure
        repo.fail(&item.id, "Error 2").await.expect("Fail 2");
        let found = match_queue::Entity::find_by_id(item.id)
            .one(&*db.db)
            .await
            .expect("Find")
            .unwrap();
        assert_eq!(found.attempts, 2);

        // Third failure
        repo.fail(&item.id, "Error 3").await.expect("Fail 3");
        let found = match_queue::Entity::find_by_id(item.id)
            .one(&*db.db)
            .await
            .expect("Find")
            .unwrap();
        assert_eq!(found.attempts, 3);
        assert_eq!(
            found.status,
            QueueStatus::Failed,
            "Should be failed after 3 attempts"
        );
    }
}
