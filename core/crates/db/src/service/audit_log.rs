//! AuditLog Service - Audit trail for compliance

use sea_orm::*;

use crate::entity::audit_log::{self, Entity as AuditLog};
use crate::{Error, Result};

/// Service for audit log operations
pub struct AuditLogService;

impl AuditLogService {
    /// Save a new audit log entry
    pub async fn save(
        db: &DatabaseConnection,
        model: audit_log::ActiveModel,
    ) -> Result<audit_log::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get log entry by ID (scans partitions if timestamp not provided)
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: uuid::Uuid,
    ) -> Result<Option<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::Id.eq(id))
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get logs by entity
    pub async fn get_by_entity(
        db: &DatabaseConnection,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::EntityType.eq(entity_type))
            .filter(audit_log::Column::EntityId.eq(entity_id))
            .order_by_desc(audit_log::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get logs by actor
    pub async fn get_by_actor(
        db: &DatabaseConnection,
        actor: &str,
        limit: u64,
    ) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::Actor.eq(actor))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get logs by action type
    pub async fn get_by_action(
        db: &DatabaseConnection,
        action: &str,
        limit: u64,
    ) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::Action.eq(action))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get recent logs
    pub async fn get_recent(db: &DatabaseConnection, limit: u64) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count logs by entity type
    pub async fn count_by_entity_type(db: &DatabaseConnection, entity_type: &str) -> Result<u64> {
        AuditLog::find()
            .filter(audit_log::Column::EntityType.eq(entity_type))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Delete old logs
    pub async fn cleanup_old(db: &DatabaseConnection, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let result = AuditLog::delete_many()
            .filter(audit_log::Column::CreatedAt.lt(cutoff))
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
