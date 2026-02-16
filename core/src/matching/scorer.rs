//! Multi-field scorer
//!
//! Ported from legacy/matching/scorer.go

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

use super::{DecayType, Thresholds, Weights};
use crate::domain::{ConfidenceBand, Offer, Request};

/// Medication category for recency decay configuration
/// Requirements: 6.4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MedicationCategory {
    /// Urgent medications - shorter half-life (faster decay)
    /// Examples: antibiotics, emergency medications
    Urgent,
    /// Stable medications - longer half-life (slower decay)
    /// Examples: chronic condition medications, vitamins
    #[default]
    Stable,
}

/// Match score breakdown
/// Ported from Go: MatchScore struct (scorer.go:15-24)
#[derive(Debug, Clone)]
pub struct MatchScore {
    pub medication_score: f64,
    pub dosage_score: f64,
    pub recency_score: f64,
    pub ai_logic_score: f64,
    pub total: f64,
    pub confidence: ConfidenceBand,
    pub breakdown: String,
}

/// Multi-field scorer for offer-request matching
/// Ported from Go: Scorer struct (scorer.go:27-36)
pub struct Scorer {
    weights: RwLock<Weights>,
    thresholds: RwLock<Thresholds>,
    recency_half_life: RwLock<f64>,
    decay_type: RwLock<DecayType>,
    min_medication_score: RwLock<f64>,
    medication_gate_enabled: RwLock<bool>,
    /// Category-specific half-life overrides
    /// Requirements: 6.4
    category_half_lives: RwLock<HashMap<MedicationCategory, f64>>,
}

impl Default for Scorer {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Scorer {
    /// Create a new scorer with optional custom weights and thresholds
    pub fn new(weights: Option<Weights>, thresholds: Option<Thresholds>) -> Self {
        // Initialize category-specific half-lives
        // Requirements 6.4: Urgent medications decay faster than stable
        let mut category_half_lives = HashMap::new();
        category_half_lives.insert(MedicationCategory::Urgent, 24.0); // 24 hours for urgent
        category_half_lives.insert(MedicationCategory::Stable, 72.0); // 72 hours for stable

        Self {
            weights: RwLock::new(weights.unwrap_or_default()),
            thresholds: RwLock::new(thresholds.unwrap_or_default()),
            recency_half_life: RwLock::new(72.0), // 72 hours default (Requirements 6.1)
            decay_type: RwLock::new(DecayType::Exponential),
            min_medication_score: RwLock::new(0.7), // Raised from 0.5 to reduce false positives
            medication_gate_enabled: RwLock::new(true),
            category_half_lives: RwLock::new(category_half_lives),
        }
    }

    /// Calculate recency score with exponential decay
    /// Ported from Go: Scorer.RecencyScore (scorer.go:141-145)
    pub fn recency_score(&self, created_at: DateTime<Utc>) -> f64 {
        let half_life = *self.recency_half_life.read().unwrap();
        self.recency_score_with_half_life(created_at, half_life)
    }

    /// Calculate recency score with custom half-life
    /// Ported from Go: Scorer.RecencyScoreWithParams (scorer.go:154-187)
    pub fn recency_score_with_half_life(
        &self,
        created_at: DateTime<Utc>,
        half_life_hours: f64,
    ) -> f64 {
        let now = Utc::now();
        let age_hours = (now - created_at).num_minutes() as f64 / 60.0;

        if age_hours <= 0.0 {
            return 1.0;
        }

        let decay_type = *self.decay_type.read().unwrap();

        match decay_type {
            DecayType::Linear => {
                // Linear decay: 1 - (age / maxAge), reaches 0 at 2x halfLife
                let max_age = half_life_hours * 2.0;
                if age_hours >= max_age {
                    0.0
                } else {
                    1.0 - (age_hours / max_age)
                }
            }
            DecayType::Logarithmic => {
                // Logarithmic decay: slower decay over time
                let max_age = half_life_hours * 4.0;
                if age_hours >= max_age {
                    0.1
                } else {
                    let ratio = age_hours / max_age;
                    (1.0 - ratio).sqrt().max(0.1)
                }
            }
            DecayType::Exponential => {
                // Exponential decay: score = 0.5^(age/half_life)
                0.5_f64.powf(age_hours / half_life_hours)
            }
        }
    }

    /// Calculate recency score for a specific medication category
    /// Requirements: 6.4 - Different decay rates per medication category
    pub fn recency_score_for_category(
        &self,
        created_at: DateTime<Utc>,
        category: MedicationCategory,
    ) -> f64 {
        let half_life = self.get_category_half_life(category);
        self.recency_score_with_half_life(created_at, half_life)
    }

    /// Get the half-life for a specific medication category
    /// Requirements: 6.4
    pub fn get_category_half_life(&self, category: MedicationCategory) -> f64 {
        let category_half_lives = self.category_half_lives.read().unwrap();
        category_half_lives
            .get(&category)
            .copied()
            .unwrap_or_else(|| *self.recency_half_life.read().unwrap())
    }

    /// Set the half-life for a specific medication category
    /// Requirements: 6.4
    pub fn set_category_half_life(&self, category: MedicationCategory, half_life_hours: f64) {
        let mut category_half_lives = self.category_half_lives.write().unwrap();
        category_half_lives.insert(category, half_life_hours.max(1.0)); // Minimum 1 hour
    }

    /// Get all category half-lives
    pub fn get_all_category_half_lives(&self) -> HashMap<MedicationCategory, f64> {
        self.category_half_lives.read().unwrap().clone()
    }

    /// Set decay type
    pub fn set_decay_type(&self, decay_type: DecayType) {
        *self.decay_type.write().unwrap() = decay_type;
    }

    /// Get current decay type
    pub fn get_decay_type(&self) -> DecayType {
        *self.decay_type.read().unwrap()
    }

    /// Get confidence band for a score
    pub fn get_confidence_band(&self, score: f64) -> ConfidenceBand {
        let thresholds = self.thresholds.read().unwrap();

        if score >= thresholds.auto {
            ConfidenceBand::Auto
        } else if score >= thresholds.suggest {
            ConfidenceBand::Suggest
        } else if score >= thresholds.review {
            ConfidenceBand::Review
        } else {
            ConfidenceBand::None
        }
    }

    /// Calculate full match score
    /// Uses medication (80%), recency (10%), and AI logic (10%)
    /// Dosage has been completely removed from the system
    pub fn score_match(
        &self,
        offer: &Offer,
        _request: &Request,
        medication_score: f64,
        ai_logic_score: Option<f64>,
    ) -> MatchScore {
        // Medication gate check
        let gate_enabled = *self.medication_gate_enabled.read().unwrap();
        let min_med = *self.min_medication_score.read().unwrap();

        if gate_enabled && medication_score < min_med {
            return MatchScore {
                medication_score,
                dosage_score: 0.0,
                recency_score: 0.0,
                ai_logic_score: 0.0,
                total: 0.0,
                confidence: ConfidenceBand::None,
                breakdown: format!(
                    "Medication mismatch ({:.0}% < {:.0}% required)",
                    medication_score * 100.0,
                    min_med * 100.0
                ),
            };
        }

        // Get weights
        let weights = self.weights.read().unwrap();

        // Calculate recency score
        let recency_score = self.recency_score(offer.created_at);

        // Get AI logic score (default to 0.0 if not provided)
        let ai_score = ai_logic_score.unwrap_or(0.0);

        // Calculate weighted total: medication (80%) + recency (10%) + AI (10%)
        let total = (medication_score * weights.medication)
            + (recency_score * weights.recency)
            + (ai_score * weights.ai_logic);

        let total = total.clamp(0.0, 1.0);
        let confidence = self.get_confidence_band(total);

        let breakdown = format!(
            "Med:{:.0}% Rec:{:.0}% AI:{:.0}%",
            medication_score * 100.0,
            recency_score * 100.0,
            ai_score * 100.0
        );

        MatchScore {
            medication_score,
            dosage_score: 0.0, // Dosage permanently removed
            recency_score,
            ai_logic_score: ai_score,
            total,
            confidence,
            breakdown,
        }
    }

    /// Update weights (thread-safe)
    pub fn update_weights(&self, weights: Weights) {
        *self.weights.write().unwrap() = weights;
    }

    /// Get current weights
    pub fn get_weights(&self) -> Weights {
        self.weights.read().unwrap().clone()
    }

    /// Update thresholds (thread-safe)
    pub fn update_thresholds(&self, thresholds: Thresholds) {
        *self.thresholds.write().unwrap() = thresholds;
    }

    /// Get current thresholds
    pub fn get_thresholds(&self) -> Thresholds {
        self.thresholds.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // Ported from Go: TestQuantityScore (scorer_test.go:11-53)
    // REMOVED: Quantity scoring no longer used

    // Ported from Go: TestGetConfidenceBand (scorer_test.go:158-187)
    #[rstest]
    #[case(1.0, ConfidenceBand::Auto)]
    #[case(0.95, ConfidenceBand::Auto)]
    #[case(0.90, ConfidenceBand::Auto)]
    #[case(0.89, ConfidenceBand::Suggest)]
    #[case(0.70, ConfidenceBand::Suggest)]
    #[case(0.69, ConfidenceBand::Review)]
    #[case(0.50, ConfidenceBand::Review)]
    #[case(0.49, ConfidenceBand::None)]
    #[case(0.0, ConfidenceBand::None)]
    fn test_get_confidence_band(#[case] score: f64, #[case] expected: ConfidenceBand) {
        let scorer = Scorer::default();
        assert_eq!(scorer.get_confidence_band(score), expected);
    }

    #[test]
    fn test_update_weights() {
        let scorer = Scorer::default();
        let new_weights = Weights {
            medication: 0.70,
            recency: 0.05,
            expiry: 0.05,
            supplier: 0.0,
            ai_logic: 0.0,
        };

        scorer.update_weights(new_weights.clone());
        let got = scorer.get_weights();

        assert!((got.medication - 0.70).abs() < 0.001);
    }

    #[test]
    fn test_category_half_life_defaults() {
        let scorer = Scorer::default();

        // Urgent should have shorter half-life than stable
        let urgent_half_life = scorer.get_category_half_life(MedicationCategory::Urgent);
        let stable_half_life = scorer.get_category_half_life(MedicationCategory::Stable);

        assert!(
            urgent_half_life < stable_half_life,
            "Urgent half-life ({}) should be less than stable half-life ({})",
            urgent_half_life,
            stable_half_life
        );

        // Check default values
        assert!(
            (urgent_half_life - 24.0).abs() < 0.001,
            "Urgent default should be 24 hours"
        );
        assert!(
            (stable_half_life - 72.0).abs() < 0.001,
            "Stable default should be 72 hours"
        );
    }

    #[test]
    fn test_category_half_life_configurable() {
        let scorer = Scorer::default();

        // Change urgent half-life
        scorer.set_category_half_life(MedicationCategory::Urgent, 12.0);
        assert!((scorer.get_category_half_life(MedicationCategory::Urgent) - 12.0).abs() < 0.001);

        // Change stable half-life
        scorer.set_category_half_life(MedicationCategory::Stable, 96.0);
        assert!((scorer.get_category_half_life(MedicationCategory::Stable) - 96.0).abs() < 0.001);

        // Minimum enforcement (1 hour)
        scorer.set_category_half_life(MedicationCategory::Urgent, 0.5);
        assert!((scorer.get_category_half_life(MedicationCategory::Urgent) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_recency_score_for_category() {
        use chrono::Duration;

        let scorer = Scorer::default();
        let now = Utc::now();

        // Create an offer that's 24 hours old
        let created_at = now - Duration::hours(24);

        // For urgent (24h half-life), score should be ~0.5
        let urgent_score =
            scorer.recency_score_for_category(created_at, MedicationCategory::Urgent);
        assert!(
            (urgent_score - 0.5).abs() < 0.05,
            "Urgent score at 24h should be ~0.5, got {}",
            urgent_score
        );

        // For stable (72h half-life), score should be ~0.79 (0.5^(24/72) = 0.5^0.333)
        let stable_score =
            scorer.recency_score_for_category(created_at, MedicationCategory::Stable);
        let expected_stable = 0.5_f64.powf(24.0 / 72.0);
        assert!(
            (stable_score - expected_stable).abs() < 0.05,
            "Stable score at 24h should be ~{}, got {}",
            expected_stable,
            stable_score
        );

        // Urgent should decay faster (lower score for same age)
        assert!(
            urgent_score < stable_score,
            "Urgent score ({}) should be less than stable score ({}) for same age",
            urgent_score,
            stable_score
        );
    }

    #[test]
    fn test_get_all_category_half_lives() {
        let scorer = Scorer::default();
        let half_lives = scorer.get_all_category_half_lives();

        assert!(half_lives.contains_key(&MedicationCategory::Urgent));
        assert!(half_lives.contains_key(&MedicationCategory::Stable));
        assert_eq!(half_lives.len(), 2);
    }
}
