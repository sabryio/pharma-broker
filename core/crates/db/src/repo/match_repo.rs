//! Match repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::prelude::Expr;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::match_::{self, Entity as Match, MatchStatus};
use crate::traits::MatchRepository;
use crate::{Error, Result};

/// SeaORM-based match repository
pub struct SeaOrmMatchRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmMatchRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MatchRepository for SeaOrmMatchRepo {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<match_::Model>> {
        Match::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .order_by_desc(match_::Column::Score)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_all(
        &self,
        limit: i64,
        offset: i64,
        status: Option<MatchStatus>,
    ) -> Result<Vec<match_::Model>> {
        let mut query = Match::find();

        if let Some(s) = status {
            query = query.filter(match_::Column::Status.eq(s));
        }

        query
            .order_by_desc(match_::Column::Score)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_all(&self, status: Option<MatchStatus>) -> Result<i64> {
        let mut query = Match::find();

        if let Some(s) = status {
            query = query.filter(match_::Column::Status.eq(s));
        }

        query
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn count_pending(&self) -> Result<i64> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn exists(&self, offer_id: Uuid, request_id: Uuid) -> Result<bool> {
        let count = Match::find()
            .filter(match_::Column::OfferId.eq(offer_id))
            .filter(match_::Column::RequestId.eq(request_id))
            .count(&*self.db)
            .await?;
        Ok(count > 0)
    }

    async fn save(&self, model: &match_::Model) -> Result<match_::Model> {
        let active: match_::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn update_status(
        &self,
        params: crate::params::UpdateMatchStatusParams,
    ) -> Result<match_::Model> {
        tracing::info!(
            match_id = %params.id,
            new_status = ?params.status,
            matched_by = %params.matched_by,
            ">>> [DEBUG] match_repo.update_status called"
        );

        let m = Match::find_by_id(params.id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", params.id)))?;

        tracing::info!(
            match_id = %m.id,
            current_status = ?m.status,
            ">>> [DEBUG] Found match, current status"
        );

        let mut active: match_::ActiveModel = m.into();
        active.status = Set(params.status);
        if !params.matched_by.is_empty() {
            active.matched_by = Set(Some(params.matched_by.to_string()));
        }
        if params.status == MatchStatus::Confirmed || params.status == MatchStatus::Rejected {
            active.confirmed_at = Set(Some(Utc::now()));
        }

        let result = active.update(&*self.db).await.map_err(Error::from)?;
        tracing::info!(
            match_id = %result.id,
            new_status = ?result.status,
            confirmed_at = ?result.confirmed_at,
            ">>> [DEBUG] Successfully updated match in database"
        );
        Ok(result)
    }

    async fn update_ai_review(&self, id: Uuid, ai_confidence: f64) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.ai_confidence = Set(Some(ai_confidence));

        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn update_score(&self, id: Uuid, score: f64, reasoning: &str) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.score = Set(score);
        active.reasoning = Set(Some(reasoning.to_string()));

        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn update_reasoning(&self, id: Uuid, reasoning: &str) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.reasoning = Set(Some(reasoning.to_string()));

        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = Match::delete_many()
            .filter(match_::Column::CreatedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn cancel_matches_for_offer(&self, offer_id: Uuid) -> Result<u64> {
        let result = Match::update_many()
            .col_expr(match_::Column::Status, Expr::value(MatchStatus::Expired))
            .filter(match_::Column::OfferId.eq(offer_id))
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn cancel_matches_for_request(&self, request_id: Uuid) -> Result<u64> {
        let result = Match::update_many()
            .col_expr(match_::Column::Status, Expr::value(MatchStatus::Expired))
            .filter(match_::Column::RequestId.eq(request_id))
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn delete_pending_matches_for_offer(&self, offer_id: Uuid) -> Result<u64> {
        let result = Match::delete_many()
            .filter(match_::Column::OfferId.eq(offer_id))
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn delete_pending_matches_for_request(&self, request_id: Uuid) -> Result<u64> {
        let result = Match::delete_many()
            .filter(match_::Column::RequestId.eq(request_id))
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn count_confirmed_today(&self) -> Result<i64> {
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let today: DateTime<Utc> = DateTime::from_naive_utc_and_offset(today, Utc);

        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Confirmed))
            .filter(match_::Column::ConfirmedAt.gte(today))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn count_rejected_today(&self) -> Result<i64> {
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let today: DateTime<Utc> = DateTime::from_naive_utc_and_offset(today, Utc);

        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Rejected))
            .filter(match_::Column::ConfirmedAt.gte(today))
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn avg_pending_score(&self) -> Result<f64> {
        use sea_orm::{ConnectionTrait, Statement};

        let result: Option<f64> = self
            .db
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT COALESCE(AVG(score), 0.0) as avg_score FROM matches WHERE status = 'PENDING'"
                    .to_string(),
            ))
            .await?
            .map(|row| row.try_get_by_index::<f64>(0).unwrap_or(0.0));

        Ok(result.unwrap_or(0.0))
    }

    async fn count_unique_pending_offers(&self) -> Result<i64> {
        use sea_orm::{ConnectionTrait, Statement};

        let result: Option<i64> = self
            .db
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT COUNT(DISTINCT offer_id) as unique_offers FROM matches WHERE status = 'PENDING'"
                    .to_string(),
            ))
            .await?
            .map(|row| row.try_get_by_index::<i64>(0).unwrap_or(0));

        Ok(result.unwrap_or(0))
    }

    async fn count_unique_pending_requests(&self) -> Result<i64> {
        use sea_orm::{ConnectionTrait, Statement};

        let result: Option<i64> = self
            .db
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT COUNT(DISTINCT request_id) as unique_requests FROM matches WHERE status = 'PENDING'"
                    .to_string(),
            ))
            .await?
            .map(|row| row.try_get_by_index::<i64>(0).unwrap_or(0));

        Ok(result.unwrap_or(0))
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::params::UpdateMatchStatusParams;
    use crate::testing::{
        TestDb, new_test_group, new_test_match, new_test_offer, new_test_raw_message,
        new_test_request,
    };
    use sea_orm::EntityTrait;

    /// Helper to create a match with all its dependencies (group, raw messages, offer, request)
    async fn create_match_with_deps(db: &TestDb) -> match_::Model {
        use crate::entity::{group, offer, raw_message, request};

        // Create group first (FK constraint)
        let group_am = new_test_group("test-group@g.us", "Test Group", true);
        group::Entity::insert(group_am).exec(&*db.db).await.ok(); // Ignore if exists

        // Create raw messages
        let msg1 = new_test_raw_message();
        let msg1_id = msg1.id.clone().unwrap();
        raw_message::Entity::insert(msg1)
            .exec(&*db.db)
            .await
            .expect("Insert raw message 1");

        let msg2 = new_test_raw_message();
        let msg2_id = msg2.id.clone().unwrap();
        raw_message::Entity::insert(msg2)
            .exec(&*db.db)
            .await
            .expect("Insert raw message 2");

        // Create offer
        let offer_am = new_test_offer(&msg1_id);
        let offer_id = offer_am.id.clone().unwrap();
        offer::Entity::insert(offer_am)
            .exec(&*db.db)
            .await
            .expect("Insert offer");

        // Create request
        let request_am = new_test_request(&msg2_id);
        let request_id = request_am.id.clone().unwrap();
        request::Entity::insert(request_am)
            .exec(&*db.db)
            .await
            .expect("Insert request");

        // Create match
        let match_am = new_test_match(&offer_id, &request_id);
        let match_id = match_am.id.clone().unwrap();
        match_::Entity::insert(match_am)
            .exec(&*db.db)
            .await
            .expect("Insert match");

        match_::Entity::find_by_id(&match_id)
            .one(&*db.db)
            .await
            .expect("Find match")
            .expect("Match should exist")
    }

    #[tokio::test]
    async fn test_get_by_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        let m = create_match_with_deps(&db).await;

        let found = repo.get_by_id(&m.id).await.expect("GetByID");
        assert!(found.is_some(), "Should find the match");
        assert_eq!(found.unwrap().id, m.id);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        let found = repo.get_by_id("non-existent").await.expect("GetByID");
        assert!(found.is_none(), "Should return None");
    }

    #[tokio::test]
    async fn test_get_pending() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        // Create 3 pending matches
        create_match_with_deps(&db).await;
        create_match_with_deps(&db).await;
        create_match_with_deps(&db).await;

        let pending = repo.get_pending(10, 0).await.expect("GetPending");
        assert_eq!(pending.len(), 3, "Should have 3 pending matches");
        assert!(pending.iter().all(|m| m.status == MatchStatus::Pending));
    }

    #[tokio::test]
    async fn test_count_pending() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        create_match_with_deps(&db).await;
        create_match_with_deps(&db).await;

        let count = repo.count_pending().await.expect("CountPending");
        assert_eq!(count, 2, "Should count 2 pending matches");
    }

    #[tokio::test]
    async fn test_exists() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        let m = create_match_with_deps(&db).await;

        assert!(
            repo.exists(&m.offer_id, &m.request_id)
                .await
                .expect("Exists"),
            "Should exist"
        );
        assert!(
            !repo.exists("other", &m.request_id).await.expect("Exists"),
            "Should not exist"
        );
    }

    #[tokio::test]
    async fn test_update_status() {
        let db = TestDb::new().await;
        let repo = SeaOrmMatchRepo::new(db.db.clone());

        let m = create_match_with_deps(&db).await;
        assert_eq!(m.status, MatchStatus::Pending);

        let updated = repo
            .update_status(UpdateMatchStatusParams::new(
                m.id,
                MatchStatus::Confirmed,
                "USER",
            ))
            .await
            .expect("UpdateStatus");

        assert_eq!(updated.status, MatchStatus::Confirmed);
        assert_eq!(updated.matched_by, Some("USER".to_string()));
        assert!(updated.confirmed_at.is_some());
    }
}
