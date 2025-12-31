//! RawMessage entity - Incoming WhatsApp messages

use chrono::Utc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "raw_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub external_id: Option<String>,
    pub participant_id: Uuid,
    pub group_id: Uuid,
    pub content: String,
    pub timestamp: DateTimeUtc,
    pub processed_at: Option<DateTimeUtc>,
    pub error: Option<String>,
    pub reply_to_id: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::participant::Entity",
        from = "Column::ParticipantId",
        to = "super::participant::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Participant,
    #[sea_orm(
        belongs_to = "super::group::Entity",
        from = "Column::GroupId",
        to = "super::group::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Group,
    #[sea_orm(has_many = "super::offer::Entity")]
    Offers,
    #[sea_orm(has_many = "super::request::Entity")]
    Requests,
}

impl Related<super::participant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Participant.def()
    }
}

impl Related<super::group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Group.def()
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
            external_id: None,
            participant_id: Uuid::nil(),
            group_id: Uuid::nil(),
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
