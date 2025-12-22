//! ReviewQueue entity - AI parse results requiring human review

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Review status
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum ReviewStatus {
    #[default]
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    #[sea_orm(string_value = "skipped")]
    Skipped,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "review_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub raw_message_id: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub ai_result: serde_json::Value,
    pub confidence: f64,
    pub reason: String,
    pub status: ReviewStatus,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub created_at: DateTimeUtc,
    pub reviewed_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a review queue item for low confidence parse results
    pub fn for_low_confidence(
        raw_message_id: &str,
        ai_result: serde_json::Value,
        confidence: f64,
        reason: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            raw_message_id: raw_message_id.to_string(),
            ai_result,
            confidence,
            reason: reason.to_string(),
            status: ReviewStatus::Pending,
            reviewed_by: None,
            review_notes: None,
            created_at: chrono::Utc::now(),
            reviewed_at: None,
        }
    }
}
