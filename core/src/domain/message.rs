//! RawMessage entity
//!
//! Ported from legacy/domain/entity/entity.go:54-71

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents an incoming WhatsApp message before AI processing
/// Ported from Go: RawMessage struct (entity.go:54-71)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RawMessage {
    pub id: String,
    pub external_id: String,
    pub group_jid: String,
    pub group_name: String,
    pub sender_jid: String,
    pub sender_phone: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,

    // Reply context
    pub reply_to_id: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
}

impl Default for RawMessage {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: String::new(),
            group_jid: String::new(),
            group_name: String::new(),
            sender_jid: String::new(),
            sender_phone: String::new(),
            sender_name: String::new(),
            content: String::new(),
            timestamp: Utc::now(),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
        }
    }
}

impl RawMessage {
    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }
}
