//! Request entity - Medication demand requests

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use super::offer::{Status, UrgencyLevel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "requests")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub raw_message_id: String,
    pub source_phone: String,
    pub source_name: Option<String>,
    pub source_group: String,
    pub group_name: String,
    pub medication: String,
    pub medication_raw: String,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub max_price: Option<Decimal>,
    pub currency: Option<String>,
    pub urgent: bool,
    pub urgency_level: UrgencyLevel,
    pub expiry_requirement: Option<String>,
    pub ai_confidence: f64,
    pub notes: Option<String>,
    pub raw_message: Option<String>,
    pub status: Status,
    pub content_embedding: Option<PgVector>, // Vector(384) for semantic search
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::raw_message::Entity",
        from = "Column::RawMessageId",
        to = "super::raw_message::Column::Id"
    )]
    RawMessage,
    #[sea_orm(has_many = "super::match_::Entity")]
    Matches,
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RawMessage.def()
    }
}

impl Related<super::match_::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Matches.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        use chrono::Utc;
        Self {
            id: String::new(),
            raw_message_id: String::new(),
            source_phone: String::new(),
            source_name: None,
            source_group: String::new(),
            group_name: String::new(),
            medication: String::new(),
            medication_raw: String::new(),
            quantity: None,
            unit: None,
            max_price: None,
            currency: None,
            urgent: false,
            urgency_level: UrgencyLevel::Normal,
            expiry_requirement: None,
            ai_confidence: 0.0,
            notes: None,
            raw_message: None,
            status: Status::Active,
            content_embedding: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Model {
    /// Get quantity as f64
    pub fn quantity_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.quantity
            .as_ref()
            .and_then(|d| d.to_f64())
            .unwrap_or(0.0)
    }

    /// Get max_price as f64
    pub fn max_price_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.max_price
            .as_ref()
            .and_then(|d| d.to_f64())
            .unwrap_or(0.0)
    }

    /// Get source_name or empty string
    pub fn source_name_str(&self) -> &str {
        self.source_name.as_deref().unwrap_or("")
    }

    /// Get embedding as Vec<f32> if present
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.content_embedding.as_ref().map(|v| v.to_vec())
    }
}
