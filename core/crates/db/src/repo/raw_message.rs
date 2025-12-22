//! RawMessage repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::raw_message::{self, Entity as RawMessage};
use crate::traits::RawMessageRepository;
use crate::{Error, Result};

/// SeaORM-based raw message repository
pub struct SeaOrmRawMessageRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmRawMessageRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RawMessageRepository for SeaOrmRawMessageRepo {
    async fn save(&self, model: &raw_message::Model) -> Result<raw_message::Model> {
        let active: raw_message::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<raw_message::Model>> {
        RawMessage::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .order_by_asc(raw_message::Column::Timestamp)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.processed_at = Set(Some(Utc::now()));
        active.error = Set(error.map(|e| e.to_string()));
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = RawMessage::delete_many()
            .filter(raw_message::Column::ProcessedAt.is_not_null())
            .filter(raw_message::Column::ProcessedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_raw_message};
    use sea_orm::EntityTrait;

    async fn create_raw_message(db: &TestDb) -> raw_message::Model {
        let msg = new_test_raw_message();
        let id = msg.id.clone().unwrap();
        raw_message::Entity::insert(msg)
            .exec(&*db.db)
            .await
            .expect("Insert raw message");

        raw_message::Entity::find_by_id(&id)
            .one(&*db.db)
            .await
            .expect("Find raw message")
            .expect("Raw message should exist")
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        let msg = create_raw_message(&db).await;

        let found = repo.get_by_id(&msg.id).await.expect("GetByID");
        assert!(found.is_some(), "Should find the message");
        assert_eq!(found.unwrap().id, msg.id);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        let found = repo.get_by_id("non-existent").await.expect("GetByID");
        assert!(found.is_none(), "Should return None");
    }

    #[tokio::test]
    async fn test_get_unprocessed() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        // Create 3 unprocessed messages
        create_raw_message(&db).await;
        create_raw_message(&db).await;
        create_raw_message(&db).await;

        let unprocessed = repo.get_unprocessed(10).await.expect("GetUnprocessed");
        assert_eq!(unprocessed.len(), 3, "Should have 3 unprocessed messages");
        assert!(unprocessed.iter().all(|m| m.processed_at.is_none()));
    }

    #[tokio::test]
    async fn test_mark_processed_success() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        let msg = create_raw_message(&db).await;
        assert!(msg.processed_at.is_none(), "Initially unprocessed");

        let processed = repo
            .mark_processed(&msg.id, None)
            .await
            .expect("MarkProcessed");
        assert!(
            processed.processed_at.is_some(),
            "Should be marked processed"
        );
        assert!(processed.error.is_none(), "Should have no error");
    }

    #[tokio::test]
    async fn test_mark_processed_with_error() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        let msg = create_raw_message(&db).await;

        let processed = repo
            .mark_processed(&msg.id, Some("Parse error"))
            .await
            .expect("MarkProcessed");
        assert!(
            processed.processed_at.is_some(),
            "Should be marked processed"
        );
        assert_eq!(
            processed.error,
            Some("Parse error".to_string()),
            "Should have error"
        );
    }

    #[tokio::test]
    async fn test_get_unprocessed_excludes_processed() {
        let db = TestDb::new().await;
        let repo = SeaOrmRawMessageRepo::new(db.db.clone());

        let msg1 = create_raw_message(&db).await;
        let msg2 = create_raw_message(&db).await;

        // Mark one as processed
        repo.mark_processed(&msg1.id, None)
            .await
            .expect("MarkProcessed");

        let unprocessed = repo.get_unprocessed(10).await.expect("GetUnprocessed");
        assert_eq!(unprocessed.len(), 1, "Should have 1 unprocessed message");
        assert_eq!(unprocessed[0].id, msg2.id, "Should be the unprocessed one");
    }
}
