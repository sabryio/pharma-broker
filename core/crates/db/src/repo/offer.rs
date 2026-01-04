//! Offer repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

use crate::entity::offer::{self, Entity as Offer, Status};
use crate::traits::OfferRepository;
use crate::{Error, Result};

/// SeaORM-based offer repository
pub struct SeaOrmOfferRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmOfferRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl OfferRepository for SeaOrmOfferRepo {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<offer::Model>> {
        Offer::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<offer::Model>> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .order_by_desc(offer::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<offer::Model>> {
        let pattern = format!("%{}%", query);
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::Medication.like(&pattern))
            .order_by_desc(offer::Column::CreatedAt)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_active(&self) -> Result<i64> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn count_by_raw_message_id(&self, raw_message_id: Uuid) -> Result<i64> {
        Offer::find()
            .filter(offer::Column::RawMessageId.eq(raw_message_id))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn find_recent_duplicate(
        &self,
        params: crate::params::FindDuplicateParams<'_>,
    ) -> Result<Option<offer::Model>> {
        let cutoff = Utc::now() - params.within;
        Offer::find()
            .filter(offer::Column::ParticipantId.eq(params.participant_id))
            .filter(offer::Column::Medication.eq(params.medication))
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::CreatedAt.gte(cutoff))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &offer::Model) -> Result<offer::Model> {
        let active: offer::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn update_status(&self, id: Uuid, status: Status) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn increment_match_count(&self, id: Uuid) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.clone().into();
        active.confirmed_match_count = Set(offer.confirmed_match_count + 1);
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn decrement_match_count(&self, id: Uuid) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.clone().into();
        // Ensure we don't go below 0
        active.confirmed_match_count = Set(offer.confirmed_match_count.saturating_sub(1));
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn update_medication(
        &self,
        id: Uuid,
        medication: &str,
        medication_raw: &str,
        ai_confidence: Option<f64>,
    ) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.into();
        active.medication = Set(medication.to_string());
        active.medication_raw = Set(medication_raw.to_string());
        active.medication_curated = Set(true);
        if let Some(conf) = ai_confidence {
            active.ai_confidence = Set(conf);
        }
        active.updated_at = Set(Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn find_semantic_duplicates(
        &self,
        params: crate::params::SemanticDuplicateParams<'_>,
    ) -> Result<Vec<offer::Model>> {
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

        Offer::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM offers 
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
        let result = Offer::delete_many()
            .filter(offer::Column::CreatedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_group, new_test_offer, new_test_raw_message};
    use sea_orm::EntityTrait;

    /// Helper to create an offer with its required raw message and group
    async fn create_offer_with_deps(db: &TestDb) -> offer::Model {
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

        // Create and return offer model
        let offer_am = new_test_offer(&msg_id);
        let offer_id = offer_am.id.clone().unwrap();
        offer::Entity::insert(offer_am)
            .exec(&*db.db)
            .await
            .expect("Save offer");

        offer::Entity::find_by_id(&offer_id)
            .one(&*db.db)
            .await
            .expect("Find offer")
            .expect("Offer should exist")
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        let offer = create_offer_with_deps(&db).await;

        let found = repo
            .get_by_id(&offer.id)
            .await
            .expect("GetByID should succeed");
        assert!(found.is_some(), "Should find the offer");
        assert_eq!(found.unwrap().id, offer.id, "ID should match");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        let found = repo
            .get_by_id("non-existent-id")
            .await
            .expect("GetByID should not error");
        assert!(found.is_none(), "Should return None for non-existent ID");
    }

    #[tokio::test]
    async fn test_get_active_pagination() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        // Create 5 active offers
        for _ in 0..5 {
            create_offer_with_deps(&db).await;
        }

        // Get first page
        let page1 = repo.get_active(2, 0).await.expect("GetActive page 1");
        assert_eq!(page1.len(), 2, "Should have 2 offers on first page");

        // Get second page
        let page2 = repo.get_active(2, 2).await.expect("GetActive page 2");
        assert_eq!(page2.len(), 2, "Should have 2 offers on second page");

        // Get third page
        let page3 = repo.get_active(2, 4).await.expect("GetActive page 3");
        assert_eq!(page3.len(), 1, "Should have 1 offer on third page");
    }

    #[tokio::test]
    async fn test_count_active() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        // Create 3 active offers
        for _ in 0..3 {
            create_offer_with_deps(&db).await;
        }

        let count = repo.count_active().await.expect("CountActive");
        assert_eq!(count, 3, "Should count all active offers");
    }

    #[tokio::test]
    async fn test_update_status() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        let offer = create_offer_with_deps(&db).await;
        assert_eq!(
            offer.status,
            Status::Active,
            "Initial status should be Active"
        );

        // Update status
        let updated = repo
            .update_status(&offer.id, Status::Matched)
            .await
            .expect("UpdateStatus");
        assert_eq!(updated.status, Status::Matched, "Status should be updated");

        // Verify via get_by_id
        let found = repo.get_by_id(&offer.id).await.expect("GetByID").unwrap();
        assert_eq!(found.status, Status::Matched, "Status should be persisted");
    }

    #[tokio::test]
    async fn test_search_by_medication() {
        let db = TestDb::new().await;
        let repo = SeaOrmOfferRepo::new(db.db.clone());

        // Create offer with specific medication
        let offer = create_offer_with_deps(&db).await;

        // Search for the medication
        let results = repo.search("Augmentin", 10, 0).await.expect("Search");
        assert!(!results.is_empty(), "Should find offers matching search");
        assert!(
            results.iter().any(|o| o.id == offer.id),
            "Should find the created offer"
        );
    }
}
