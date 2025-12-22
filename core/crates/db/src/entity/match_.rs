//! Match entity - Offer-Request matches

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Match status enum
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum MatchStatus {
    #[default]
    #[sea_orm(string_value = "PENDING")]
    Pending,
    #[sea_orm(string_value = "CONFIRMED")]
    Confirmed,
    #[sea_orm(string_value = "REJECTED")]
    Rejected,
    #[sea_orm(string_value = "EXPIRED")]
    Expired,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "matches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub offer_id: String,
    pub request_id: String,
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
            id: String::new(),
            offer_id: String::new(),
            request_id: String::new(),
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
    /// Get reasoning or default
    pub fn reasoning_str(&self) -> &str {
        self.reasoning.as_deref().unwrap_or("")
    }
}
