//! FeedbackRecord entity - User feedback on matches for learning
//!
//! Stores operator feedback (confirm/reject) on matches to enable
//! the weight learning system to improve matching accuracy.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::feedback_params::{CreateFeedbackParams, FeedbackScores};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "feedback_records")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub match_id: String,
    pub user_id: String,
    pub confirmed: bool,
    pub medication_score: f64,
    pub dosage_score: f64,
    pub quantity_score: f64,
    pub price_score: f64,
    pub recency_score: f64,
    pub total_score: f64,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::match_::Entity",
        from = "Column::MatchId",
        to = "super::match_::Column::Id"
    )]
    Match,
}

impl Related<super::match_::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Match.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new feedback record from parameters struct (preferred)
    pub fn from_params(params: CreateFeedbackParams) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: params.match_id,
            user_id: params.user_id,
            confirmed: params.confirmed,
            medication_score: params.scores.medication,
            dosage_score: params.scores.dosage,
            quantity_score: params.scores.quantity,
            price_score: params.scores.price,
            recency_score: params.scores.recency,
            total_score: params.scores.total,
            created_at: chrono::Utc::now(),
        }
    }

    /// Create a new feedback record with individual arguments
    ///
    /// Prefer using `from_params` with `CreateFeedbackParams` for cleaner code.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        confirmed: bool,
        medication_score: f64,
        dosage_score: f64,
        quantity_score: f64,
        price_score: f64,
        recency_score: f64,
        total_score: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: match_id.into(),
            user_id: user_id.into(),
            confirmed,
            medication_score,
            dosage_score,
            quantity_score,
            price_score,
            recency_score,
            total_score,
            created_at: chrono::Utc::now(),
        }
    }

    /// Create a confirmation feedback record with estimated scores
    pub fn confirmed(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        total_score: f64,
    ) -> Self {
        Self::from_params(CreateFeedbackParams::confirmed(
            match_id,
            user_id,
            total_score,
        ))
    }

    /// Create a rejection feedback record with estimated scores
    pub fn rejected(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        total_score: f64,
    ) -> Self {
        Self::from_params(CreateFeedbackParams::rejected(
            match_id,
            user_id,
            total_score,
        ))
    }

    /// Get the scores as a FeedbackScores struct
    pub fn scores(&self) -> FeedbackScores {
        FeedbackScores {
            medication: self.medication_score,
            dosage: self.dosage_score,
            quantity: self.quantity_score,
            price: self.price_score,
            recency: self.recency_score,
            total: self.total_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_params() {
        let params = CreateFeedbackParams::confirmed("match-123", "user-456", 0.85);
        let record = Model::from_params(params);

        assert_eq!(record.match_id, "match-123");
        assert_eq!(record.user_id, "user-456");
        assert!(record.confirmed);
        assert!((record.total_score - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_confirmed_shorthand() {
        let record = Model::confirmed("match-1", "user-1", 0.9);
        assert!(record.confirmed);
        assert!((record.total_score - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_rejected_shorthand() {
        let record = Model::rejected("match-2", "user-2", 0.5);
        assert!(!record.confirmed);
        assert!((record.total_score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_scores_extraction() {
        let record = Model::new("m", "u", true, 0.9, 0.8, 0.7, 0.6, 0.5, 0.75);
        let scores = record.scores();

        assert!((scores.medication - 0.9).abs() < 0.001);
        assert!((scores.dosage - 0.8).abs() < 0.001);
        assert!((scores.total - 0.75).abs() < 0.001);
    }
}
