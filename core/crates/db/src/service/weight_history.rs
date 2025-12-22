//! WeightHistory Service - Historical weight configurations

use sea_orm::*;

use crate::entity::weight_history::{self, Entity as WeightHistory};
use crate::{Error, Result};

/// Service for weight history operations
pub struct WeightHistoryService;

impl WeightHistoryService {
    /// Save a new weight configuration
    pub async fn save(
        db: &DatabaseConnection,
        model: weight_history::ActiveModel,
    ) -> Result<weight_history::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get weight config by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: uuid::Uuid,
    ) -> Result<Option<weight_history::Model>> {
        WeightHistory::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get latest weight configuration
    pub async fn get_latest(db: &DatabaseConnection) -> Result<Option<weight_history::Model>> {
        WeightHistory::find()
            .order_by_desc(weight_history::Column::CreatedAt)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get weight history (most recent first)
    pub async fn get_history(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<weight_history::Model>> {
        WeightHistory::find()
            .order_by_desc(weight_history::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get weights by source
    pub async fn get_by_source(
        db: &DatabaseConnection,
        source: &str,
    ) -> Result<Vec<weight_history::Model>> {
        WeightHistory::find()
            .filter(weight_history::Column::Source.eq(source))
            .order_by_desc(weight_history::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get weights with minimum sample count
    pub async fn get_with_min_samples(
        db: &DatabaseConnection,
        min_samples: i32,
    ) -> Result<Vec<weight_history::Model>> {
        WeightHistory::find()
            .filter(weight_history::Column::SampleCount.gte(min_samples))
            .order_by_desc(weight_history::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count weight configurations
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        WeightHistory::find().count(db).await.map_err(Error::from)
    }

    /// Delete old weight configurations (keep N most recent)
    pub async fn cleanup_keep_recent(db: &DatabaseConnection, keep: u64) -> Result<u64> {
        // Get IDs to keep
        let to_keep: Vec<uuid::Uuid> = WeightHistory::find()
            .order_by_desc(weight_history::Column::CreatedAt)
            .limit(keep)
            .all(db)
            .await?
            .into_iter()
            .map(|w| w.id)
            .collect();

        if to_keep.is_empty() {
            return Ok(0);
        }

        let result = WeightHistory::delete_many()
            .filter(weight_history::Column::Id.is_not_in(to_keep))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_service_exists() {
        // Basic compile test
    }
}
