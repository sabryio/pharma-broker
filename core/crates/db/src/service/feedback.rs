//! Feedback Service - User feedback on matches for learning

use sea_orm::*;

use crate::entity::feedback_record::{self, Entity as FeedbackRecord};
use crate::{Error, Result};

/// Service for feedback record operations
pub struct FeedbackService;

impl FeedbackService {
    /// Save a new feedback record
    pub async fn save(
        db: &DatabaseConnection,
        model: feedback_record::ActiveModel,
    ) -> Result<feedback_record::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get feedback by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: uuid::Uuid,
    ) -> Result<Option<feedback_record::Model>> {
        FeedbackRecord::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get feedback by match ID
    pub async fn get_by_match(
        db: &DatabaseConnection,
        match_id: &str,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::MatchId.eq(match_id))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get feedback by user ID
    pub async fn get_by_user(
        db: &DatabaseConnection,
        user_id: &str,
        limit: u64,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::UserId.eq(user_id))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get recent feedback
    pub async fn get_recent(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .order_by_desc(feedback_record::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get confirmed feedback (for training)
    pub async fn get_confirmed(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::Confirmed.eq(true))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get rejected feedback (for training)
    pub async fn get_rejected(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::Confirmed.eq(false))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count total feedback
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        FeedbackRecord::find().count(db).await.map_err(Error::from)
    }

    /// Count confirmed feedback
    pub async fn count_confirmed(db: &DatabaseConnection) -> Result<u64> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::Confirmed.eq(true))
            .count(db)
            .await
            .map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_service_exists() {
        // Basic compile test
    }
}
