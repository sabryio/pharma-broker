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

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::params::AuditByEntityParams;
    use crate::testing::{TestDb, new_test_audit_log};
    use sea_orm::EntityTrait;

    async fn create_audit_log(db: &TestDb, action: &str, entity_id: &str) -> audit_log::Model {
        let log = new_test_audit_log(action, entity_id);
        let id = log.id.clone().unwrap();
        audit_log::Entity::insert(log)
            .exec(&db.db)
            .await
            .expect("Insert audit log");

        audit_log::Entity::find_by_id(id)
            .one(&db.db)
            .await
            .expect("Find audit log")
            .expect("Audit log should exist")
    }

    #[tokio::test]
    async fn test_get_by_entity() {
        let db = TestDb::new().await;
        let repo = SeaOrmAuditLogRepo::new(db.db.clone());

        create_audit_log(&db, "confirm", "match-123").await;
        create_audit_log(&db, "reject", "match-123").await;
        create_audit_log(&db, "confirm", "match-other").await;

        let logs = repo
            .get_by_entity(AuditByEntityParams {
                entity_type: "match",
                entity_id: "match-123",
                limit: 10,
            })
            .await
            .expect("GetByEntity");

        assert_eq!(logs.len(), 2, "Should find 2 logs for match-123");
    }

    #[tokio::test]
    async fn test_get_by_actor() {
        let db = TestDb::new().await;
        let repo = SeaOrmAuditLogRepo::new(db.db.clone());

        // All test logs have actor "test-user"
        create_audit_log(&db, "action1", "e1").await;
        create_audit_log(&db, "action2", "e2").await;

        let logs = repo
            .get_by_actor("test-user", 10)
            .await
            .expect("GetByActor");
        assert_eq!(logs.len(), 2, "Should find 2 logs for test-user");
    }

    #[tokio::test]
    async fn test_get_by_action() {
        let db = TestDb::new().await;
        let repo = SeaOrmAuditLogRepo::new(db.db.clone());

        create_audit_log(&db, "confirm", "e1").await;
        create_audit_log(&db, "confirm", "e2").await;
        create_audit_log(&db, "reject", "e3").await;

        let confirms = repo
            .get_by_action("confirm", 10)
            .await
            .expect("GetByAction");
        assert_eq!(confirms.len(), 2, "Should find 2 confirm actions");
    }

    #[tokio::test]
    async fn test_get_recent_pagination() {
        let db = TestDb::new().await;
        let repo = SeaOrmAuditLogRepo::new(db.db.clone());

        for i in 0..5 {
            create_audit_log(&db, &format!("action{}", i), &format!("e{}", i)).await;
        }

        let page1 = repo.get_recent(2, 0).await.expect("GetRecent page 1");
        assert_eq!(page1.len(), 2, "Should have 2 on first page");

        let page2 = repo.get_recent(2, 2).await.expect("GetRecent page 2");
        assert_eq!(page2.len(), 2, "Should have 2 on second page");
    }

    #[tokio::test]
    async fn test_count() {
        let db = TestDb::new().await;
        let repo = SeaOrmAuditLogRepo::new(db.db.clone());

        assert_eq!(repo.count().await.expect("Count"), 0, "Initially 0");

        create_audit_log(&db, "a", "e").await;
        create_audit_log(&db, "b", "e").await;

        assert_eq!(repo.count().await.expect("Count"), 2, "Should count 2");
    }
}
