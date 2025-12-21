//! Offer entity
//!
//! Ported from legacy/domain/entity/entity.go:86-106

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{ItemStatus, UrgencyLevel};

/// Represents a medication supply offer
/// Ported from Go: Offer struct (entity.go:86-106)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Offer {
    pub id: String,
    pub raw_message_id: String,
    pub source_phone: String,
    pub source_name: String,
    pub source_group: String,
    pub group_name: String,
    pub medication: String,
    pub medication_raw: String,
    pub quantity: f64,
    pub unit: Option<String>,
    pub price: f64,
    pub currency: Option<String>,
    pub expiry_date: Option<DateTime<Utc>>,
    pub batch_number: Option<String>,
    pub notes: Option<String>,
    pub raw_message: String,
    pub status: ItemStatus,
    pub content_embedding: Option<Vec<f32>>,
    /// Urgency flag (seller urgency to sell)
    #[serde(default)]
    pub urgent: bool,
    /// Granular urgency level
    #[serde(default)]
    pub urgency_level: UrgencyLevel,
    /// Expiry info from AI extraction (YYYY-MM or description)
    pub expiry_info: Option<String>,
    /// AI extraction confidence score
    #[serde(default)]
    pub ai_confidence: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Offer {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            raw_message_id: String::new(),
            source_phone: String::new(),
            source_name: String::new(),
            source_group: String::new(),
            group_name: String::new(),
            medication: String::new(),
            medication_raw: String::new(),
            quantity: 0.0,
            unit: None,
            price: 0.0,
            currency: None,
            expiry_date: None,
            batch_number: None,
            notes: None,
            raw_message: String::new(),
            status: ItemStatus::Active,
            content_embedding: None,
            urgent: false,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            created_at: now,
            updated_at: now,
        }
    }
}
