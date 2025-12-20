//! Weight History entity
//!
//! Stores historical weight configurations for auditing and rollback

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Weight history record
/// Stores each weight configuration change for auditing and rollback
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WeightHistory {
    pub id: Uuid,
    pub medication_weight: f64,
    pub dosage_weight: f64,
    pub quantity_weight: f64,
    pub price_weight: f64,
    pub recency_weight: f64,
    /// Source of the weight change
    pub source: String,
    /// Number of samples used to calculate these weights
    pub sample_count: i32,
    pub created_at: DateTime<Utc>,
}

impl WeightHistory {
    /// Create a new weight history entry
    pub fn new(
        medication_weight: f64,
        dosage_weight: f64,
        quantity_weight: f64,
        price_weight: f64,
        recency_weight: f64,
        source: String,
        sample_count: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            medication_weight,
            dosage_weight,
            quantity_weight,
            price_weight,
            recency_weight,
            source,
            sample_count,
            created_at: Utc::now(),
        }
    }

    /// Create from matching Weights struct
    pub fn from_weights(
        weights: &crate::matching::Weights,
        source: String,
        sample_count: i32,
    ) -> Self {
        Self::new(
            weights.medication,
            weights.dosage,
            weights.quantity,
            weights.price,
            weights.recency,
            source,
            sample_count,
        )
    }

    /// Convert to Weights struct
    pub fn to_weights(&self) -> crate::matching::Weights {
        crate::matching::Weights {
            medication: self.medication_weight,
            dosage: self.dosage_weight,
            quantity: self.quantity_weight,
            price: self.price_weight,
            recency: self.recency_weight,
        }
    }
}
