//! Participant entity - Represents a unique user/contact

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "participants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub jid: String,
    #[sea_orm(unique)]
    pub phone: String,
    pub push_name: Option<String>,
    pub display_name: Option<String>,
    pub label: Option<String>,
    pub notes: Option<String>,
    pub is_blocked: bool,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
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

impl Related<super::group::Entity> for Entity {
    fn via() -> Option<RelationDef> {
        Some(Relation::ParticipantGroups.def())
    }

    fn to() -> RelationDef {
        super::participant_group::Relation::Group.def()
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
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            jid: String::new(),
            phone: String::new(),
            push_name: None,
            display_name: None,
            label: None,
            notes: None,
            is_blocked: false,
            created_at: now,
            updated_at: now,
        }
    }
}
