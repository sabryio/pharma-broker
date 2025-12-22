//! Offer entity - Medication supply offers

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Offer status enum
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum Status {
    #[default]
    #[sea_orm(string_value = "ACTIVE")]
    Active,
    #[sea_orm(string_value = "MATCHED")]
    Matched,
    #[sea_orm(string_value = "EXPIRED")]
    Expired,
    #[sea_orm(string_value = "CANCELLED")]
    Cancelled,
    #[sea_orm(string_value = "DUPLICATE")]
    Duplicate,
}

/// Urgency level enum
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum UrgencyLevel {
    #[default]
    #[sea_orm(string_value = "NORMAL")]
    Normal,
    #[sea_orm(string_value = "SOON")]
    Soon,
    #[sea_orm(string_value = "URGENT")]
    Urgent,
    #[sea_orm(string_value = "CRITICAL")]
    Critical,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offers")]
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
    pub price: Option<Decimal>,
    pub currency: Option<String>,
    pub expiry_date: Option<Date>,
    pub batch_number: Option<String>,
    pub notes: Option<String>,
    pub raw_message: Option<String>,
    pub status: Status,
    pub urgent: bool,
    pub urgency_level: UrgencyLevel,
    pub expiry_info: Option<String>,
    pub ai_confidence: f64,
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
            price: None,
            currency: None,
            expiry_date: None,
            batch_number: None,
            notes: None,
            raw_message: None,
            status: Status::Active,
            urgent: false,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl UrgencyLevel {
    /// Convert from boolean urgent flag (backward compatibility)
    pub fn from_bool(urgent: bool) -> Self {
        if urgent {
            UrgencyLevel::Urgent
        } else {
            UrgencyLevel::Normal
        }
    }

    /// Check if this is any level of urgency
    pub fn is_urgent(&self) -> bool {
        !matches!(self, UrgencyLevel::Normal)
    }

    /// Get priority score (0.0 = normal, 1.0 = critical)
    pub fn priority_score(&self) -> f64 {
        match self {
            UrgencyLevel::Normal => 0.0,
            UrgencyLevel::Soon => 0.3,
            UrgencyLevel::Urgent => 0.7,
            UrgencyLevel::Critical => 1.0,
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

    /// Get price as f64
    pub fn price_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.price.as_ref().and_then(|d| d.to_f64()).unwrap_or(0.0)
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
