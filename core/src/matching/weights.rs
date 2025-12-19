//! Scoring weights configuration
//!
//! Ported from legacy/matching/interface.go

use serde::{Deserialize, Serialize};

/// Scoring weights for multi-field matching
/// Ported from Go: Weights struct (interface.go)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weights {
    pub medication: f64,
    pub dosage: f64,
    pub quantity: f64,
    pub price: f64,
    pub recency: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            medication: 0.45,
            dosage: 0.10,
            quantity: 0.20,
            price: 0.15,
            recency: 0.10,
        }
    }
}

/// Confidence thresholds
/// Ported from Go: Thresholds struct (interface.go)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    pub auto: f64,
    pub suggest: f64,
    pub review: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            auto: 0.90,
            suggest: 0.70,
            review: 0.50,
        }
    }
}
