//! Match Audit Record repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

use crate::entity::match_audit_record::{self, Entity as MatchAuditRecord};
use crate::traits::MatchAuditRecordRepository;
use crate::{Error, Result};

/// SeaORM-based match audit record repository
pub struct SeaOrmMatchAuditRecordRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmMatchAuditRecordRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MatchAuditRecordRepository for SeaOrmMatchAuditRecordRepo {
    async fn insert(&self, model: &match_audit_record::Model) -> Result<match_audit_record::Model> {
        let active: match_audit_record::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<match_audit_record::Model>> {
        MatchAuditRecord::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_match_id(&self, match_id: Uuid) -> Result<Option<match_audit_record::Model>> {
        MatchAuditRecord::find()
            .filter(match_audit_record::Column::MatchId.eq(match_id))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_session(&self, session_id: &str) -> Result<Vec<match_audit_record::Model>> {
        MatchAuditRecord::find()
            .filter(match_audit_record::Column::SessionId.eq(session_id))
            .order_by_desc(match_audit_record::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn list_recent(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<match_audit_record::Model>> {
        MatchAuditRecord::find()
            .order_by_desc(match_audit_record::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64> {
        let result = MatchAuditRecord::delete_many()
            .filter(match_audit_record::Column::CreatedAt.lt(cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn count(&self) -> Result<i64> {
        MatchAuditRecord::find()
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn update_review_status(
        &self,
        id: Uuid,
        status: &str,
        reviewed_by: Uuid,
        notes: Option<&str>,
    ) -> Result<match_audit_record::Model> {
        let record = MatchAuditRecord::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match audit record {} not found", id)))?;

        let mut active: match_audit_record::ActiveModel = record.into();
        active.review_status = Set(Some(status.to_string()));
        active.reviewed_by = Set(Some(reviewed_by));
        active.reviewed_at = Set(Some(Utc::now()));
        active.review_notes = Set(notes.map(|s| s.to_string()));

        active.update(&*self.db).await.map_err(Error::from)
    }
}
