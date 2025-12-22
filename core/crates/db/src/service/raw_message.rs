//! RawMessage Service - Incoming WhatsApp message management

use sea_orm::*;

use crate::entity::raw_message::{self, Entity as RawMessage};
use crate::{Error, Result};

/// Service for raw message operations
pub struct RawMessageService;

impl RawMessageService {
    /// Save a new raw message
    pub async fn save(
        db: &DatabaseConnection,
        model: raw_message::ActiveModel,
    ) -> Result<raw_message::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get message by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: &str,
    ) -> Result<Option<raw_message::Model>> {
        RawMessage::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get message by external ID
    pub async fn get_by_external_id(
        db: &DatabaseConnection,
        external_id: &str,
    ) -> Result<Option<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ExternalId.eq(external_id))
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get unprocessed messages
    pub async fn get_unprocessed(db: &DatabaseConnection) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .order_by_asc(raw_message::Column::Timestamp)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get unprocessed messages with limit
    pub async fn get_unprocessed_batch(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .order_by_asc(raw_message::Column::Timestamp)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Mark message as processed
    pub async fn mark_processed(db: &DatabaseConnection, id: &str) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.processed_at = Set(Some(chrono::Utc::now()));
        active.error = Set(None);
        active.update(db).await.map_err(Error::from)
    }

    /// Mark message as failed with error
    pub async fn mark_failed(
        db: &DatabaseConnection,
        id: &str,
        error: &str,
    ) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.error = Set(Some(error.to_string()));
        active.update(db).await.map_err(Error::from)
    }

    /// Get messages by group JID
    pub async fn get_by_group(
        db: &DatabaseConnection,
        group_jid: &str,
        limit: u64,
    ) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::GroupJid.eq(group_jid))
            .order_by_desc(raw_message::Column::Timestamp)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count unprocessed messages
    pub async fn count_unprocessed(db: &DatabaseConnection) -> Result<u64> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Count failed messages
    pub async fn count_failed(db: &DatabaseConnection) -> Result<u64> {
        RawMessage::find()
            .filter(raw_message::Column::Error.is_not_null())
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Delete old processed messages
    pub async fn delete_old_processed(db: &DatabaseConnection, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let result = RawMessage::delete_many()
            .filter(raw_message::Column::ProcessedAt.is_not_null())
            .filter(raw_message::Column::ProcessedAt.lt(cutoff))
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
