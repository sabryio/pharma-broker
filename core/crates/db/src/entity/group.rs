//! Group entity - WhatsApp groups for monitoring

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub jid: String,
    pub name: String,
    pub description: Option<String>,
    pub monitored: bool,
    pub added_at: DateTimeUtc,
    pub last_message: Option<DateTimeUtc>,
    pub message_count: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::raw_message::Entity")]
    Messages,
    #[sea_orm(has_many = "super::offer::Entity")]
    Offers,
    #[sea_orm(has_many = "super::request::Entity")]
    Requests,
    #[sea_orm(has_many = "super::participant_group::Entity")]
    ParticipantGroups,
}

impl Related<super::participant::Entity> for Entity {
    fn via() -> Option<RelationDef> {
        Some(Relation::ParticipantGroups.def())
    }

    fn to() -> RelationDef {
        super::participant_group::Relation::Participant.def()
    }
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl Related<super::offer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Offers.def()
    }
}

impl Related<super::request::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Requests.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
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
            id: Uuid::new_v4(),
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
            id: Uuid::new_v4(),
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
