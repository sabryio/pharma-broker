//! Group entity
//!
//! Ported from legacy/domain/entity/entity.go:151-160

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a monitored WhatsApp group
/// Ported from Go: Group struct (entity.go:151-160)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Group {
    pub jid: String,
    pub name: String,
    pub description: Option<String>,
    pub monitored: bool,
    pub added_at: DateTime<Utc>,
    pub last_message: Option<DateTime<Utc>>,
    pub message_count: i64,
}

impl Group {
    /// Create a new group with default values
    pub fn new(jid: String, name: String) -> Self {
        Self {
            jid,
            name,
            description: None,
            monitored: false,
            added_at: Utc::now(),
            last_message: None,
            message_count: 0,
        }
    }

    /// Check if this group is being monitored
    pub fn is_monitored(&self) -> bool {
        self.monitored
    }
}
