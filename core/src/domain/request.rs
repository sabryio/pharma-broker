//! Request entity
//!
//! Ported from legacy/domain/entity/entity.go:109-128

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ItemStatus;

/// Represents a medication demand request
/// Ported from Go: Request struct (entity.go:109-128)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Request {
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
    pub max_price: f64,
    pub currency: Option<String>,
    pub urgent: bool,
    pub notes: Option<String>,
    pub raw_message: String,
    pub status: ItemStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Request {
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
            max_price: 0.0,
            currency: None,
            urgent: false,
            notes: None,
            raw_message: String::new(),
            status: ItemStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }
}
