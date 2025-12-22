//! MatchQueue entity - Async matching queue

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Queue item status
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum QueueStatus {
    #[default]
    #[sea_orm(string_value = "PENDING")]
    Pending,
    #[sea_orm(string_value = "PROCESSING")]
    Processing,
    #[sea_orm(string_value = "COMPLETED")]
    Completed,
    #[sea_orm(string_value = "FAILED")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "match_queue_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub request_id: String,
    pub status: QueueStatus,
    pub priority: i32,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub next_attempt_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::request::Entity",
        from = "Column::RequestId",
        to = "super::request::Column::Id"
    )]
    Request,
}

impl Related<super::request::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Request.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
