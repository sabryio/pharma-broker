//! Match Service - Offer-Request match management

use sea_orm::*;
use uuid::Uuid;

use crate::entity::match_::{self, Entity as Match, MatchStatus};
use crate::{Error, Result};

/// Service for match operations
pub struct MatchService;

impl MatchService {
    /// Save a new match
    pub async fn save(
        db: &DatabaseConnection,
        model: match_::ActiveModel,
    ) -> Result<match_::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get match by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<match_::Model>> {
        Match::find_by_id(id).one(db).await.map_err(Error::from)
    }

    /// Get pending matches
    pub async fn get_pending(db: &DatabaseConnection) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .order_by_desc(match_::Column::Score)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get pending matches with limit
    pub async fn get_pending_batch(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .order_by_desc(match_::Column::Score)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get matches by offer ID
    pub async fn get_by_offer(
        db: &DatabaseConnection,
        offer_id: &str,
    ) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::OfferId.eq(offer_id))
            .order_by_desc(match_::Column::Score)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get matches by request ID
    pub async fn get_by_request(
        db: &DatabaseConnection,
        request_id: &str,
    ) -> Result<Vec<match_::Model>> {
        Match::find()
            .filter(match_::Column::RequestId.eq(request_id))
            .order_by_desc(match_::Column::Score)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Confirm a match
    pub async fn confirm(db: &DatabaseConnection, id: Uuid) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.status = Set(MatchStatus::Confirmed);
        active.confirmed_at = Set(Some(chrono::Utc::now()));
        active.update(db).await.map_err(Error::from)
    }

    /// Reject a match
    pub async fn reject(
        db: &DatabaseConnection,
        id: Uuid,
        notes: Option<&str>,
    ) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.status = Set(MatchStatus::Rejected);
        if let Some(n) = notes {
            active.notes = Set(Some(n.to_string()));
        }
        active.update(db).await.map_err(Error::from)
    }

    /// Expire a match
    pub async fn expire(db: &DatabaseConnection, id: Uuid) -> Result<match_::Model> {
        let m = Match::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Match not found: {}", id)))?;

        let mut active: match_::ActiveModel = m.into();
        active.status = Set(MatchStatus::Expired);
        active.update(db).await.map_err(Error::from)
    }

    /// Count pending matches
    pub async fn count_pending(db: &DatabaseConnection) -> Result<u64> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Pending))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Count confirmed matches
    pub async fn count_confirmed(db: &DatabaseConnection) -> Result<u64> {
        Match::find()
            .filter(match_::Column::Status.eq(MatchStatus::Confirmed))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Delete a match
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Match::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_default() {
        assert_eq!(MatchStatus::default(), MatchStatus::Pending);
    }
}
