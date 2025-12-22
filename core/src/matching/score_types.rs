//! Score and weight newtypes for type-safe matching calculations
//!
//! These newtypes provide compile-time safety and validation for score
//! and weight values, preventing common errors like using raw f64 values
//! outside valid ranges.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Mul;

// =============================================================================
// ConfidenceScore
// =============================================================================

/// A confidence score between 0.0 and 1.0
///
/// Represents the confidence level of a match or prediction.
/// Values are guaranteed to be in the valid range.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfidenceScore(f64);

impl ConfidenceScore {
    /// Minimum valid score
    pub const MIN: f64 = 0.0;
    /// Maximum valid score
    pub const MAX: f64 = 1.0;

    /// Create a new confidence score, returning None if out of range
    pub fn new(value: f64) -> Option<Self> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Create a new confidence score, clamping to valid range
    pub fn new_clamped(value: f64) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    /// Create a zero confidence score
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Create a perfect confidence score
    pub fn perfect() -> Self {
        Self(1.0)
    }

    /// Get the raw f64 value
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Convert to percentage (0-100)
    pub fn as_percentage(&self) -> f64 {
        self.0 * 100.0
    }

    /// Check if this score meets a threshold
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.0 >= threshold
    }

    /// Check if this is a high confidence score (>= 0.9)
    pub fn is_high(&self) -> bool {
        self.0 >= 0.9
    }

    /// Check if this is a medium confidence score (0.7 - 0.9)
    pub fn is_medium(&self) -> bool {
        self.0 >= 0.7 && self.0 < 0.9
    }

    /// Check if this is a low confidence score (< 0.7)
    pub fn is_low(&self) -> bool {
        self.0 < 0.7
    }
}

impl Default for ConfidenceScore {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Display for ConfidenceScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percentage())
    }
}

impl From<ConfidenceScore> for f64 {
    fn from(score: ConfidenceScore) -> Self {
        score.0
    }
}

// =============================================================================
// Weight
// =============================================================================

/// A weight value between 0.0 and 1.0
///
/// Represents a weighting factor for scoring calculations.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Weight(f64);

impl Weight {
    /// Minimum valid weight
    pub const MIN: f64 = 0.0;
    /// Maximum valid weight
    pub const MAX: f64 = 1.0;

    /// Create a new weight, returning None if out of range
    pub fn new(value: f64) -> Option<Self> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Create a new weight, clamping to valid range
    pub fn new_clamped(value: f64) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    /// Create a zero weight
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Create a full weight (1.0)
    pub fn full() -> Self {
        Self(1.0)
    }

    /// Get the raw f64 value
    pub fn value(&self) -> f64 {
        self.0
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

impl From<Weight> for f64 {
    fn from(weight: Weight) -> Self {
        weight.0
    }
}

impl Mul<f64> for Weight {
    type Output = f64;

    fn mul(self, rhs: f64) -> Self::Output {
        self.0 * rhs
    }
}

impl Mul<Weight> for f64 {
    type Output = f64;

    fn mul(self, rhs: Weight) -> Self::Output {
        self * rhs.0
    }
}

// =============================================================================
// NormalizedWeights
// =============================================================================

/// A set of weights that are guaranteed to sum to 1.0
///
/// Used for weighted scoring where all weights must be normalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedWeights {
    pub medication: Weight,
    pub dosage: Weight,
    pub quantity: Weight,
    pub price: Weight,
    pub recency: Weight,
}

impl NormalizedWeights {
    /// Tolerance for sum validation
    const SUM_TOLERANCE: f64 = 0.001;

    /// Create new normalized weights, validating they sum to 1.0
    pub fn new(
        medication: f64,
        dosage: f64,
        quantity: f64,
        price: f64,
        recency: f64,
    ) -> Result<Self, WeightError> {
        let sum = medication + dosage + quantity + price + recency;
        if (sum - 1.0).abs() > Self::SUM_TOLERANCE {
            return Err(WeightError::InvalidSum { actual: sum });
        }

        Ok(Self {
            medication: Weight::new(medication).ok_or(WeightError::OutOfRange {
                field: "medication",
                value: medication,
            })?,
            dosage: Weight::new(dosage).ok_or(WeightError::OutOfRange {
                field: "dosage",
                value: dosage,
            })?,
            quantity: Weight::new(quantity).ok_or(WeightError::OutOfRange {
                field: "quantity",
                value: quantity,
            })?,
            price: Weight::new(price).ok_or(WeightError::OutOfRange {
                field: "price",
                value: price,
            })?,
            recency: Weight::new(recency).ok_or(WeightError::OutOfRange {
                field: "recency",
                value: recency,
            })?,
        })
    }

    /// Create normalized weights by normalizing the input values
    pub fn from_unnormalized(
        medication: f64,
        dosage: f64,
        quantity: f64,
        price: f64,
        recency: f64,
    ) -> Result<Self, WeightError> {
        let sum = medication + dosage + quantity + price + recency;
        if sum == 0.0 {
            return Err(WeightError::ZeroSum);
        }

        Self::new(
            medication / sum,
            dosage / sum,
            quantity / sum,
            price / sum,
            recency / sum,
        )
    }

    /// Calculate weighted score from component scores
    pub fn calculate_score(
        &self,
        medication_score: f64,
        dosage_score: f64,
        quantity_score: f64,
        price_score: f64,
        recency_score: f64,
    ) -> ConfidenceScore {
        let total = self.medication * medication_score
            + self.dosage * dosage_score
            + self.quantity * quantity_score
            + self.price * price_score
            + self.recency * recency_score;

        ConfidenceScore::new_clamped(total)
    }
}

impl Default for NormalizedWeights {
    /// Default weights matching the legacy configuration
    fn default() -> Self {
        Self {
            medication: Weight::new(0.75).unwrap(),
            dosage: Weight::new(0.05).unwrap(),
            quantity: Weight::new(0.05).unwrap(),
            price: Weight::new(0.05).unwrap(),
            recency: Weight::new(0.10).unwrap(),
        }
    }
}

// =============================================================================
// WeightError
// =============================================================================

/// Errors that can occur when creating weights
#[derive(Debug, Clone, thiserror::Error)]
pub enum WeightError {
    #[error("Weight '{field}' value {value} is out of range [0.0, 1.0]")]
    OutOfRange { field: &'static str, value: f64 },

    #[error("Weights must sum to 1.0, got {actual}")]
    InvalidSum { actual: f64 },

    #[error("Cannot normalize weights that sum to zero")]
    ZeroSum,
}

// =============================================================================
// ScoreBreakdown
// =============================================================================

/// A breakdown of individual scores with their weights
///
/// Provides a detailed view of how a total score was calculated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub medication: ComponentScore,
    pub dosage: ComponentScore,
    pub quantity: ComponentScore,
    pub price: ComponentScore,
    pub recency: ComponentScore,
    pub total: ConfidenceScore,
}

/// A single component score with its weight contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentScore {
    /// The raw score for this component (0.0 - 1.0)
    pub score: f64,
    /// The weight applied to this component
    pub weight: Weight,
    /// The weighted contribution (score * weight)
    pub contribution: f64,
}

impl ComponentScore {
    pub fn new(score: f64, weight: Weight) -> Self {
        Self {
            score,
            weight,
            contribution: score * weight.value(),
        }
    }
}

impl ScoreBreakdown {
    /// Create a new score breakdown
    pub fn new(
        weights: &NormalizedWeights,
        medication_score: f64,
        dosage_score: f64,
        quantity_score: f64,
        price_score: f64,
        recency_score: f64,
    ) -> Self {
        let medication = ComponentScore::new(medication_score, weights.medication);
        let dosage = ComponentScore::new(dosage_score, weights.dosage);
        let quantity = ComponentScore::new(quantity_score, weights.quantity);
        let price = ComponentScore::new(price_score, weights.price);
        let recency = ComponentScore::new(recency_score, weights.recency);

        let total_value = medication.contribution
            + dosage.contribution
            + quantity.contribution
            + price.contribution
            + recency.contribution;

        Self {
            medication,
            dosage,
            quantity,
            price,
            recency,
            total: ConfidenceScore::new_clamped(total_value),
        }
    }

    /// Format as a human-readable breakdown string
    pub fn format_breakdown(&self) -> String {
        format!(
            "Med:{:.0}% Dos:{:.0}% Qty:{:.0}% Price:{:.0}% Rec:{:.0}%",
            self.medication.score * 100.0,
            self.dosage.score * 100.0,
            self.quantity.score * 100.0,
            self.price.score * 100.0,
            self.recency.score * 100.0
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_score_new() {
        assert!(ConfidenceScore::new(0.5).is_some());
        assert!(ConfidenceScore::new(0.0).is_some());
        assert!(ConfidenceScore::new(1.0).is_some());
        assert!(ConfidenceScore::new(-0.1).is_none());
        assert!(ConfidenceScore::new(1.1).is_none());
    }

    #[test]
    fn test_confidence_score_clamped() {
        let score = ConfidenceScore::new_clamped(1.5);
        assert!((score.value() - 1.0).abs() < 0.001);

        let score = ConfidenceScore::new_clamped(-0.5);
        assert!((score.value() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_confidence_score_display() {
        let score = ConfidenceScore::new(0.85).unwrap();
        assert_eq!(format!("{}", score), "85.0%");
    }

    #[test]
    fn test_confidence_score_levels() {
        assert!(ConfidenceScore::new(0.95).unwrap().is_high());
        assert!(ConfidenceScore::new(0.80).unwrap().is_medium());
        assert!(ConfidenceScore::new(0.50).unwrap().is_low());
    }

    #[test]
    fn test_weight_new() {
        assert!(Weight::new(0.5).is_some());
        assert!(Weight::new(-0.1).is_none());
        assert!(Weight::new(1.1).is_none());
    }

    #[test]
    fn test_weight_multiplication() {
        let weight = Weight::new(0.5).unwrap();
        let result = weight * 0.8;
        assert!((result - 0.4).abs() < 0.001);

        let result2 = 0.8 * weight;
        assert!((result2 - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_normalized_weights_valid() {
        let weights = NormalizedWeights::new(0.75, 0.05, 0.05, 0.05, 0.10);
        assert!(weights.is_ok());
    }

    #[test]
    fn test_normalized_weights_invalid_sum() {
        let weights = NormalizedWeights::new(0.5, 0.5, 0.5, 0.5, 0.5);
        assert!(matches!(weights, Err(WeightError::InvalidSum { .. })));
    }

    #[test]
    fn test_normalized_weights_from_unnormalized() {
        let weights = NormalizedWeights::from_unnormalized(3.0, 1.0, 1.0, 1.0, 2.0).unwrap();
        let sum = weights.medication.value()
            + weights.dosage.value()
            + weights.quantity.value()
            + weights.price.value()
            + weights.recency.value();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_normalized_weights_calculate_score() {
        let weights = NormalizedWeights::default();
        let score = weights.calculate_score(0.9, 0.8, 0.7, 0.6, 0.5);
        assert!(score.value() > 0.0);
        assert!(score.value() <= 1.0);
    }

    #[test]
    fn test_score_breakdown() {
        let weights = NormalizedWeights::default();
        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);

        assert!((breakdown.medication.score - 0.9).abs() < 0.001);
        assert!(breakdown.total.value() > 0.0);

        let formatted = breakdown.format_breakdown();
        assert!(formatted.contains("Med:90%"));
    }
}
