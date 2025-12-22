//! WeightHistory repository implementation

use async_trait::async_trait;
use sea_orm::*;

use crate::entity::weight_history::{self, Entity as WeightHistory};
use crate::traits::WeightHistoryRepository;
use crate::{Error, Result};

/// SeaORM-based weight history repository
pub struct SeaOrmWeightHistoryRepo {
    db: DatabaseConnection,
}

impl SeaOrmWeightHistoryRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WeightHistoryRepository for SeaOrmWeightHistoryRepo {
    async fn save(&self, model: &weight_history::Model) -> Result<weight_history::Model> {
        let active: weight_history::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn get_current(&self) -> Result<Option<weight_history::Model>> {
        WeightHistory::find()
            .order_by_desc(weight_history::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_history(&self, limit: i64) -> Result<Vec<weight_history::Model>> {
        WeightHistory::find()
            .order_by_desc(weight_history::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<weight_history::Model>> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| Error::Validation(format!("Invalid UUID: {}", id)))?;
        WeightHistory::find_by_id(uuid)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count(&self) -> Result<i64> {
        WeightHistory::find()
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_weight_history};
    use sea_orm::EntityTrait;

    async fn create_weight_history(db: &TestDb, source: &str) -> weight_history::Model {
        let wh = new_test_weight_history(source);
        let id = wh.id.clone().unwrap();
        weight_history::Entity::insert(wh)
            .exec(&db.db)
            .await
            .expect("Insert weight history");

        weight_history::Entity::find_by_id(id)
            .one(&db.db)
            .await
            .expect("Find weight history")
            .expect("Weight history should exist")
    }

    #[tokio::test]
    async fn test_get_current() {
        let db = TestDb::new().await;
        let repo = SeaOrmWeightHistoryRepo::new(db.db.clone());

        // Initially none (migration inserts default)
        // Create a new one
        let _wh = create_weight_history(&db, "test_source").await;

        let current = repo.get_current().await.expect("GetCurrent");
        assert!(current.is_some(), "Should have current weights");
        // The most recent one should be ours or the initial
        let curr = current.unwrap();
        // Check it's either our test or initial
        assert!(curr.source == "test_source" || curr.source == "initial");
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmWeightHistoryRepo::new(db.db.clone());

        let wh = create_weight_history(&db, "test").await;

        let found = repo.get_by_id(&wh.id.to_string()).await.expect("GetByID");
        assert!(found.is_some(), "Should find by id");
        assert_eq!(found.unwrap().id, wh.id);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmWeightHistoryRepo::new(db.db.clone());

        let found = repo
            .get_by_id(&uuid::Uuid::new_v4().to_string())
            .await
            .expect("GetByID");
        assert!(found.is_none(), "Should return None");
    }

    #[tokio::test]
    async fn test_get_history() {
        let db = TestDb::new().await;
        let repo = SeaOrmWeightHistoryRepo::new(db.db.clone());

        create_weight_history(&db, "source1").await;
        create_weight_history(&db, "source2").await;
        create_weight_history(&db, "source3").await;

        let history = repo.get_history(10).await.expect("GetHistory");
        // At least 3 (we created) plus potentially the initial from migration
        assert!(history.len() >= 3, "Should have at least 3 history entries");
    }

    #[tokio::test]
    async fn test_count() {
        let db = TestDb::new().await;
        let repo = SeaOrmWeightHistoryRepo::new(db.db.clone());

        let initial = repo.count().await.expect("Count");

        create_weight_history(&db, "new").await;

        assert_eq!(
            repo.count().await.expect("Count"),
            initial + 1,
            "Should increment count"
        );
    }
}
