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
