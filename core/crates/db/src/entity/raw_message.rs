//! RawMessage entity - Incoming WhatsApp messages

use chrono::Utc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "raw_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub external_id: Option<String>,
    pub group_jid: String,
    pub group_name: String,
    pub sender_jid: String,
    pub sender_phone: Option<String>,
    pub sender_name: Option<String>,
    pub content: String,
    pub timestamp: DateTimeUtc,
    pub processed_at: Option<DateTimeUtc>,
    pub error: Option<String>,
    pub reply_to_id: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
    pub created_at: DateTimeUtc,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: String::new(),
            external_id: None,
            group_jid: String::new(),
            group_name: String::new(),
            sender_jid: String::new(),
            sender_phone: None,
            sender_name: None,
            content: String::new(),
            timestamp: Utc::now(),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: Utc::now(),
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::offer::Entity")]
    Offers,
    #[sea_orm(has_many = "super::request::Entity")]
    Requests,
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
