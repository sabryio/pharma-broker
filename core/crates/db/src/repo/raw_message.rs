//! RawMessage repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::raw_message::{self, Entity as RawMessage};
use crate::traits::RawMessageRepository;
use crate::{Error, Result};

/// SeaORM-based raw message repository
pub struct SeaOrmRawMessageRepo {
    db: DatabaseConnection,
}

impl SeaOrmRawMessageRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RawMessageRepository for SeaOrmRawMessageRepo {
    async fn save(&self, model: &raw_message::Model) -> Result<raw_message::Model> {
        let active: raw_message::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<raw_message::Model>> {
        RawMessage::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .order_by_asc(raw_message::Column::Timestamp)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.processed_at = Set(Some(Utc::now()));
        active.error = Set(error.map(|e| e.to_string()));
        active.update(&self.db).await.map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = RawMessage::delete_many()
            .filter(raw_message::Column::ProcessedAt.is_not_null())
            .filter(raw_message::Column::ProcessedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
