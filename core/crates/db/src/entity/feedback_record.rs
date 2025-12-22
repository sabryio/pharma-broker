//! FeedbackRecord entity - User feedback on matches for learning

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

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
    /// Create a new feedback record
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
}
