use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumString, Display, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchQueueStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MatchQueueItem {
    pub id: Uuid,
    pub request_id: String,
    pub status: MatchQueueStatus,
    pub priority: i32,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub next_attempt_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MatchQueueItem {
    pub fn new(request_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            request_id,
            status: MatchQueueStatus::Pending,
            priority: 0,
            attempts: 0,
            last_error: None,
            next_attempt_at: now,
            created_at: now,
            updated_at: now,
        }
    }
}
