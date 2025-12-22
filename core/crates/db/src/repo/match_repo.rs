//! Match repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::match_::{self, Entity as Match, MatchStatus};
use crate::traits::MatchRepository;
use crate::{Error, Result};

/// SeaORM-based match repository
pub struct SeaOrmMatchRepo {
    db: DatabaseConnection,
}

impl SeaOrmMatchRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MatchRepository for SeaOrmMatchRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<match_::Model>> {
        Match::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .order_by_desc(match_::Column::Score)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_pending(&self) -> Result<i64> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool> {
        let count = Match::find()
            .filter(match_::Column::OfferId.eq(offer_id))
            .filter(match_::Column::RequestId.eq(request_id))
            .count(&self.db)
            .await?;
        Ok(count > 0)
    }

    async fn save(&self, model: &match_::Model) -> Result<match_::Model> {
        let active: match_::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn update_status(
        &self,
        params: crate::params::UpdateMatchStatusParams<'_>,
    ) -> Result<match_::Model> {
        let m = Match::find_by_id(params.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", params.id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.status = Set(params.status.clone());
        if !params.matched_by.is_empty() {
            active.matched_by = Set(Some(params.matched_by.to_string()));
        }
        if !params.notes.is_empty() {
            active.notes = Set(Some(params.notes.to_string()));
        }
        if params.status == MatchStatus::Confirmed {
            active.confirmed_at = Set(Some(Utc::now()));
        }
        active.update(&self.db).await.map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = Match::delete_many()
            .filter(match_::Column::CreatedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
