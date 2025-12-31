//! Request repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

use crate::entity::offer::Status;
use crate::entity::request::{self, Entity as Request};
use crate::traits::RequestRepository;
use crate::{Error, Result};

/// SeaORM-based request repository
pub struct SeaOrmRequestRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmRequestRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RequestRepository for SeaOrmRequestRepo {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<request::Model>> {
        Request::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<request::Model>> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .order_by_desc(request::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<request::Model>> {
        let pattern = format!("%{}%", query);
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::Medication.like(&pattern))
            .order_by_desc(request::Column::CreatedAt)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_active(&self) -> Result<i64> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn find_recent_duplicate(
        &self,
        params: crate::params::FindDuplicateParams<'_>,
    ) -> Result<Option<request::Model>> {
        let cutoff = Utc::now() - params.within;
        Request::find()
            .filter(request::Column::ParticipantId.eq(params.participant_id))
            .filter(request::Column::Medication.eq(params.medication))
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::CreatedAt.gte(cutoff))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &request::Model) -> Result<request::Model> {
        let active: request::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn update_status(&self, id: Uuid, status: Status) -> Result<request::Model> {
        let request = Request::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Request not found: {}", id)))?;

        let mut active: request::ActiveModel = request.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn find_semantic_duplicates(
        &self,
        params: crate::params::SemanticDuplicateParams<'_>,
    ) -> Result<Vec<request::Model>> {
        let cutoff = Utc::now() - params.within;
        let embedding_str = format!(
            "[{}]",
            params
                .embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        Request::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM requests 
                WHERE status = 'ACTIVE' 
                AND content_embedding IS NOT NULL
                AND created_at >= $3
                AND 1 - (content_embedding <=> $1::vector) > $2
                ORDER BY content_embedding <=> $1::vector
                "#,
                [
                    embedding_str.into(),
                    params.similarity_threshold.into(),
                    cutoff.into(),
                ],
            ))
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = Request::delete_many()
            .filter(request::Column::CreatedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_group, new_test_raw_message, new_test_request};
    use sea_orm::EntityTrait;

    /// Helper to create a request with its required raw message and group
    async fn create_request_with_deps(db: &TestDb) -> request::Model {
        use crate::entity::{group, raw_message};

        // Create group first (FK constraint)
        let group = new_test_group("test-group@g.us", "Test Group", true);
        group::Entity::insert(group).exec(&*db.db).await.ok(); // Ignore if already exists

        // Create raw message
        let msg = new_test_raw_message();
        let msg_id = msg.id.clone().unwrap();
        raw_message::Entity::insert(msg)
            .exec(&*db.db)
            .await
            .expect("Save raw message");

        // Create and return request model
        let request_am = new_test_request(&msg_id);
        let request_id = request_am.id.clone().unwrap();
        request::Entity::insert(request_am)
            .exec(&*db.db)
            .await
            .expect("Save request");

        request::Entity::find_by_id(&request_id)
            .one(&*db.db)
            .await
            .expect("Find request")
            .expect("Request should exist")
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        let request = create_request_with_deps(&db).await;

        let found = repo
            .get_by_id(&request.id)
            .await
            .expect("GetByID should succeed");
        assert!(found.is_some(), "Should find the request");
        assert_eq!(found.unwrap().id, request.id, "ID should match");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        let found = repo
            .get_by_id("non-existent-id")
            .await
            .expect("GetByID should not error");
        assert!(found.is_none(), "Should return None for non-existent ID");
    }

    #[tokio::test]
    async fn test_get_active_pagination() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        // Create 5 active requests
        for _ in 0..5 {
            create_request_with_deps(&db).await;
        }

        // Get first page
        let page1 = repo.get_active(2, 0).await.expect("GetActive page 1");
        assert_eq!(page1.len(), 2, "Should have 2 requests on first page");

        // Get second page
        let page2 = repo.get_active(2, 2).await.expect("GetActive page 2");
        assert_eq!(page2.len(), 2, "Should have 2 requests on second page");

        // Get third page
        let page3 = repo.get_active(2, 4).await.expect("GetActive page 3");
        assert_eq!(page3.len(), 1, "Should have 1 request on third page");
    }

    #[tokio::test]
    async fn test_count_active() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        // Create 3 active requests
        for _ in 0..3 {
            create_request_with_deps(&db).await;
        }

        let count = repo.count_active().await.expect("CountActive");
        assert_eq!(count, 3, "Should count all active requests");
    }

    #[tokio::test]
    async fn test_update_status() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        let request = create_request_with_deps(&db).await;
        assert_eq!(
            request.status,
            Status::Active,
            "Initial status should be Active"
        );

        // Update status
        let updated = repo
            .update_status(&request.id, Status::Matched)
            .await
            .expect("UpdateStatus");
        assert_eq!(updated.status, Status::Matched, "Status should be updated");

        // Verify via get_by_id
        let found = repo.get_by_id(&request.id).await.expect("GetByID").unwrap();
        assert_eq!(found.status, Status::Matched, "Status should be persisted");
    }

    #[tokio::test]
    async fn test_search_by_medication() {
        let db = TestDb::new().await;
        let repo = SeaOrmRequestRepo::new(db.db.clone());

        // Create request with specific medication
        let request = create_request_with_deps(&db).await;

        // Search for the medication
        let results = repo.search("Augmentin", 10, 0).await.expect("Search");
        assert!(!results.is_empty(), "Should find requests matching search");
        assert!(
            results.iter().any(|r| r.id == request.id),
            "Should find the created request"
        );
    }
}
