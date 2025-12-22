//! AuditLog repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::audit_log::{self, Entity as AuditLog};
use crate::traits::AuditLogRepository;
use crate::{Error, Result};

/// SeaORM-based audit log repository
pub struct SeaOrmAuditLogRepo {
    db: DatabaseConnection,
}

impl SeaOrmAuditLogRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuditLogRepository for SeaOrmAuditLogRepo {
    async fn save(&self, model: &audit_log::Model) -> Result<audit_log::Model> {
        let active: audit_log::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn get_by_entity(
        &self,
        params: crate::params::AuditByEntityParams<'_>,
    ) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::EntityType.eq(params.entity_type))
            .filter(audit_log::Column::EntityId.eq(params.entity_id))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(params.limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_actor(&self, actor: &str, limit: i64) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::Actor.eq(actor))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_action(&self, action: &str, limit: i64) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::Action.eq(action))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_recent(&self, limit: i64, offset: i64) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<audit_log::Model>> {
        AuditLog::find()
            .filter(audit_log::Column::CreatedAt.gte(start))
            .filter(audit_log::Column::CreatedAt.lte(end))
            .order_by_desc(audit_log::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count(&self) -> Result<i64> {
        AuditLog::find()
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = AuditLog::delete_many()
            .filter(audit_log::Column::CreatedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
