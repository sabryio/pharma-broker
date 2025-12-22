//! Group entity - WhatsApp groups for monitoring

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub jid: String,
    pub name: String,
    pub description: Option<String>,
    pub monitored: bool,
    pub added_at: DateTimeUtc,
    pub last_message: Option<DateTimeUtc>,
    pub message_count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        Self {
            jid: String::new(),
            name: String::new(),
            description: None,
            monitored: false,
            added_at: chrono::Utc::now(),
            last_message: None,
            message_count: 0,
        }
    }
}

impl Model {
    /// Create a new group
    pub fn new(jid: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            jid: jid.into(),
            name: name.into(),
            description: None,
            monitored: false,
            added_at: chrono::Utc::now(),
            last_message: None,
            message_count: 0,
        }
    }

    /// Create a monitored group
    pub fn monitored(jid: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            jid: jid.into(),
            name: name.into(),
            description: None,
            monitored: true,
            added_at: chrono::Utc::now(),
            last_message: None,
            message_count: 0,
        }
    }
}
