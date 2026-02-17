//! RetryQueue entity - Queue for retrying failed message processing

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Retry queue item status
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum RetryStatus {
    #[default]
    #[sea_orm(string_value = "PENDING")]
    Pending,
    #[sea_orm(string_value = "PROCESSING")]
    Processing,
    #[sea_orm(string_value = "COMPLETED")]
    Completed,
    #[sea_orm(string_value = "FAILED")]
    Failed,
    #[sea_orm(string_value = "CANCELLED")]
    Cancelled,
}

/// Failure reason category
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(30))")]
pub enum FailureReason {
    #[sea_orm(string_value = "CIRCUIT_BREAKER")]
    CircuitBreaker,
    #[sea_orm(string_value = "NETWORK_ERROR")]
    NetworkError,
    #[sea_orm(string_value = "INCOMPLETE_JSON")]
    IncompleteJson,
    #[sea_orm(string_value = "TIMEOUT")]
    Timeout,
    #[sea_orm(string_value = "PARSE_ERROR")]
    ParseError,
    #[sea_orm(string_value = "OTHER")]
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "retry_queue_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    /// Reference to the raw message that failed
    pub raw_message_id: Uuid,
    /// Current status of the retry
    pub status: RetryStatus,
    /// Priority (higher = processed first)
    pub priority: i32,
    /// Number of retry attempts made
    pub attempts: i32,
    /// Maximum retry attempts allowed
    pub max_attempts: i32,
    /// Categorized failure reason
    pub failure_reason: FailureReason,
    /// Original error message
    pub original_error: String,
    /// Last error from retry attempt
    pub last_error: Option<String>,
    /// When to attempt next retry
    pub next_attempt_at: DateTimeUtc,
    /// When the item was added to queue
    pub created_at: DateTimeUtc,
    /// Last update timestamp
    pub updated_at: DateTimeUtc,
    /// When processing completed (success or permanent failure)
    pub completed_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::raw_message::Entity",
        from = "Column::RawMessageId",
        to = "super::raw_message::Column::Id",
        on_delete = "Cascade"
    )]
    RawMessage,
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RawMessage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
