//! Scoring weights configuration
//!
//! Ported from legacy/matching/interface.go
//! Updated with safety-critical weight rebalancing per Requirements 8.1

use serde::{Deserialize, Serialize};

/// Weight validation tolerance for sum check
const WEIGHT_SUM_TOLERANCE: f64 = 0.001;

/// Error type for weight validation
#[derive(Debug, Clone, PartialEq)]
pub enum WeightError {
    /// Weights do not sum to 1.0 within tolerance
    InvalidSum { actual: f64, expected: f64 },
    /// Individual weight is negative
    NegativeWeight { field: String, value: f64 },
}

impl std::fmt::Display for WeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeightError::InvalidSum { actual, expected } => {
                write!(
                    f,
                    "Weights sum to {:.4} but must equal {:.1} (within {:.3} tolerance)",
                    actual, expected, WEIGHT_SUM_TOLERANCE
                )
            }
            WeightError::NegativeWeight { field, value } => {
                write!(f, "Weight '{}' is negative: {:.4}", field, value)
            }
        }
    }
}

impl std::error::Error for WeightError {}

/// Scoring weights for multi-field matching
/// Ported from Go: Weights struct (interface.go)
/// Updated with new factors: expiry and supplier (Requirements 8.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weights {
    /// Medication name similarity weight (default: 0.60)
    pub medication: f64,
    /// Dosage match weight - increased for safety (default: 0.15)
    pub dosage: f64,
    /// Quantity fulfillment weight (default: 0.05)
    pub quantity: f64,
    /// Price within budget weight (default: 0.05)
    pub price: f64,
    /// Recency/freshness weight - reduced (default: 0.05)
    pub recency: f64,
    /// Expiry date validation weight (default: 0.05) - NEW
    pub expiry: f64,
    /// Supplier reliability weight (default: 0.05) - NEW
    pub supplier: f64,
    /// AI logic score weight (default: 0.0, disabled)
    pub ai_logic: f64,
}

impl Default for Weights {
    fn default() -> Self {
        // Updated weights per Requirements 8.1:
        // medication (60%), dosage (15%), quantity (5%), price (5%),
        // recency (5%), expiry (5%), supplier (5%)
        Self {
            medication: 0.60, // Reduced from 0.75 to accommodate new factors
            dosage: 0.15,     // Increased from 0.05 for safety
            quantity: 0.05,   // Unchanged
            price: 0.05,      // Unchanged
            recency: 0.05,    // Reduced from 0.10
            expiry: 0.05,     // NEW: expiry date validation
            supplier: 0.05,   // NEW: supplier reliability (reserved)
            ai_logic: 0.0,    // Disabled by default
        }
    }
}

impl Weights {
    /// Calculate the sum of all weights
    pub fn sum(&self) -> f64 {
        self.medication
            + self.dosage
            + self.quantity
            + self.price
            + self.recency
            + self.expiry
            + self.supplier
            + self.ai_logic
    }

    /// Validate that weights sum to 1.0 and none are negative
    /// Returns error if weights don't sum to 1.0 (within tolerance) or any weight is negative
    /// Requirements: 8.3
    pub fn validate(&self) -> Result<(), WeightError> {
        // Check for negative weights
        if self.medication < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "medication".to_string(),
                value: self.medication,
            });
        }
        if self.dosage < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "dosage".to_string(),
                value: self.dosage,
            });
        }
        if self.quantity < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "quantity".to_string(),
                value: self.quantity,
            });
        }
        if self.price < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "price".to_string(),
                value: self.price,
            });
        }
        if self.recency < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "recency".to_string(),
                value: self.recency,
            });
        }
        if self.expiry < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "expiry".to_string(),
                value: self.expiry,
            });
        }
        if self.supplier < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "supplier".to_string(),
                value: self.supplier,
            });
        }
        if self.ai_logic < 0.0 {
            return Err(WeightError::NegativeWeight {
                field: "ai_logic".to_string(),
                value: self.ai_logic,
            });
        }

        // Check sum equals 1.0 within tolerance
        let sum = self.sum();
        if (sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
            return Err(WeightError::InvalidSum {
                actual: sum,
                expected: 1.0,
            });
        }

        Ok(())
    }

    /// Normalize weights so they sum to 1.0
    /// Useful for auto-correcting weights that don't sum correctly
    pub fn normalize(&mut self) {
        let sum = self.sum();
        if sum > 0.0 && (sum - 1.0).abs() > WEIGHT_SUM_TOLERANCE {
            self.medication /= sum;
            self.dosage /= sum;
            self.quantity /= sum;
            self.price /= sum;
            self.recency /= sum;
            self.expiry /= sum;
            self.supplier /= sum;
            self.ai_logic /= sum;
        }
    }

    /// Create a normalized copy of the weights
    pub fn normalized(&self) -> Self {
        let mut copy = self.clone();
        copy.normalize();
        copy
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights_sum_to_one() {
        let weights = Weights::default();
        let sum = weights.sum();
        assert!(
            (sum - 1.0).abs() < WEIGHT_SUM_TOLERANCE,
            "Default weights sum to {} but should be 1.0",
            sum
        );
    }

    #[test]
    fn test_default_weights_validate() {
        let weights = Weights::default();
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_sum() {
        let weights = Weights {
            medication: 0.5,
            dosage: 0.1,
            quantity: 0.1,
            price: 0.1,
            recency: 0.1,
            expiry: 0.05,
            supplier: 0.1, // Changed from 0.05 to make sum = 1.05
            ai_logic: 0.0,
        };
        let result = weights.validate();
        assert!(result.is_err());
        match result {
            Err(WeightError::InvalidSum { actual, .. }) => {
                assert!((actual - 1.0).abs() > WEIGHT_SUM_TOLERANCE);
            }
            _ => panic!("Expected InvalidSum error"),
        }
    }

    #[test]
    fn test_validate_negative_weight() {
        let weights = Weights {
            medication: -0.1,
            dosage: 0.15,
            quantity: 0.05,
            price: 0.05,
            recency: 0.05,
            expiry: 0.05,
            supplier: 0.05,
            ai_logic: 0.7,
        };
        let result = weights.validate();
        assert!(result.is_err());
        match result {
            Err(WeightError::NegativeWeight { field, .. }) => {
                assert_eq!(field, "medication");
            }
            _ => panic!("Expected NegativeWeight error"),
        }
    }

    #[test]
    fn test_normalize_weights() {
        let mut weights = Weights {
            medication: 0.6,
            dosage: 0.15,
            quantity: 0.05,
            price: 0.05,
            recency: 0.05,
            expiry: 0.05,
            supplier: 0.05,
            ai_logic: 0.1, // Sum = 1.1
        };
        weights.normalize();
        let sum = weights.sum();
        assert!(
            (sum - 1.0).abs() < WEIGHT_SUM_TOLERANCE,
            "Normalized weights sum to {} but should be 1.0",
            sum
        );
    }

    #[test]
    fn test_normalized_returns_copy() {
        let weights = Weights {
            medication: 0.6,
            dosage: 0.15,
            quantity: 0.05,
            price: 0.05,
            recency: 0.05,
            expiry: 0.05,
            supplier: 0.05,
            ai_logic: 0.1, // Sum = 1.1
        };
        let normalized = weights.normalized();
        // Original should be unchanged
        assert!((weights.sum() - 1.1).abs() < 0.001);
        // Normalized copy should sum to 1.0
        assert!((normalized.sum() - 1.0).abs() < WEIGHT_SUM_TOLERANCE);
    }

    #[test]
    fn test_new_default_weight_values() {
        let weights = Weights::default();
        assert!((weights.medication - 0.60).abs() < 0.001);
        assert!((weights.dosage - 0.15).abs() < 0.001);
        assert!((weights.quantity - 0.05).abs() < 0.001);
        assert!((weights.price - 0.05).abs() < 0.001);
        assert!((weights.recency - 0.05).abs() < 0.001);
        assert!((weights.expiry - 0.05).abs() < 0.001);
        assert!((weights.supplier - 0.05).abs() < 0.001);
        assert!((weights.ai_logic - 0.0).abs() < 0.001);
    }
}
