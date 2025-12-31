//! Match entity - Offer-Request matches
//!
//! Represents potential or confirmed matches between offers and requests.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export common types for backward compatibility
pub use super::common::MatchStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "matches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub offer_id: Uuid,
    pub request_id: Uuid,
    #[sea_orm(column_type = "Double")]
    pub score: f64,
    pub reasoning: Option<String>,
    pub matched_by: Option<String>,
    pub status: MatchStatus,
    pub created_at: DateTimeUtc,
    pub confirmed_at: Option<DateTimeUtc>,
    pub notes: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::offer::Entity",
        from = "Column::OfferId",
        to = "super::offer::Column::Id"
    )]
    Offer,
    #[sea_orm(
        belongs_to = "super::request::Entity",
        from = "Column::RequestId",
        to = "super::request::Column::Id"
    )]
    Request,
    #[sea_orm(has_many = "super::feedback_record::Entity")]
    FeedbackRecords,
}

impl Related<super::offer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Offer.def()
    }
}

impl Related<super::request::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Request.def()
    }
}

impl Related<super::feedback_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FeedbackRecords.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        use chrono::Utc;
        Self {
            id: Uuid::new_v4(),
            offer_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            score: 0.0,
            reasoning: None,
            matched_by: None,
            status: MatchStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
            notes: None,
        }
    }
}

impl Model {
    /// Get reasoning or default empty string
    pub fn reasoning_str(&self) -> &str {
        self.reasoning.as_deref().unwrap_or("")
    }

    /// Check if the match is still pending
    pub fn is_pending(&self) -> bool {
        self.status.is_pending()
    }

    /// Check if the match was confirmed
    pub fn is_confirmed(&self) -> bool {
        self.status.is_confirmed()
    }

    /// Get score as a percentage string
    pub fn score_percentage(&self) -> String {
        format!("{:.1}%", self.score * 100.0)
    }
}
