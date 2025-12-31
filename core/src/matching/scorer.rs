//! Multi-field scorer
//!
//! Ported from legacy/matching/scorer.go

use chrono::{DateTime, Utc};
use std::sync::RwLock;

use super::{DecayType, Thresholds, Weights, compare_dosages, parse_dosage};
use crate::domain::{ConfidenceBand, Offer, Request};

/// Match score breakdown
/// Ported from Go: MatchScore struct (scorer.go:15-24)
#[derive(Debug, Clone)]
pub struct MatchScore {
    pub medication_score: f64,
    pub dosage_score: f64,
    pub quantity_score: f64,
    pub price_score: f64,
    pub recency_score: f64,
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
}

impl Default for Scorer {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Scorer {
    /// Create a new scorer with optional custom weights and thresholds
    pub fn new(weights: Option<Weights>, thresholds: Option<Thresholds>) -> Self {
        Self {
            weights: RwLock::new(weights.unwrap_or_default()),
            thresholds: RwLock::new(thresholds.unwrap_or_default()),
            recency_half_life: RwLock::new(24.0), // 24 hours default
            decay_type: RwLock::new(DecayType::Exponential),
            min_medication_score: RwLock::new(0.7), // Raised from 0.5 to reduce false positives
            medication_gate_enabled: RwLock::new(true),
        }
    }

    /// Calculate quantity score
    /// Ported from Go: Scorer.QuantityScore (scorer.go:65-95)
    pub fn quantity_score(&self, offer_qty: f64, request_qty: f64) -> f64 {
        // Handle edge cases
        if request_qty <= 0.0 {
            return 1.0; // Any quantity satisfies no/negative request
        }
        if offer_qty <= 0.0 {
            return 0.0; // No offer quantity = no match
        }

        let ratio = offer_qty / request_qty;

        // Perfect score within ±10% tolerance or over-supply
        if ratio >= 0.9 {
            return 1.0;
        }

        // Partial fulfillment - return ratio
        ratio
    }

    /// Calculate price score
    /// Ported from Go: Scorer.PriceScore (scorer.go:97-139)
    pub fn price_score(&self, offer_price: f64, max_price: f64) -> f64 {
        // No max price set
        if max_price <= 0.0 {
            return if offer_price <= 0.0 { 1.0 } else { 0.95 };
        }

        // No offer price
        if offer_price <= 0.0 {
            return 0.85; // Unknown price with budget
        }

        let ratio = offer_price / max_price;
        let tolerance = 0.05; // 5% tolerance

        // Within budget including tolerance
        if ratio <= 1.0 + tolerance {
            return 1.0;
        }

        // Over budget - linear decay
        let over_ratio = (ratio - (1.0 + tolerance)) / 1.0; // Decay over 100% above tolerance
        let score = 1.0 - over_ratio;

        score.max(0.0)
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
    /// Ported from Go: Scorer.ScoreMatch (scorer.go:223-272)
    pub fn score_match(
        &self,
        offer: &Offer,
        request: &Request,
        medication_score: f64,
    ) -> MatchScore {
        // Medication gate check
        let gate_enabled = *self.medication_gate_enabled.read().unwrap();
        let min_med = *self.min_medication_score.read().unwrap();

        if gate_enabled && medication_score < min_med {
            return MatchScore {
                medication_score,
                dosage_score: 0.0,
                quantity_score: 0.0,
                price_score: 0.0,
                recency_score: 0.0,
                total: 0.0,
                confidence: ConfidenceBand::None,
                breakdown: format!(
                    "Medication mismatch ({:.0}% < {:.0}% required)",
                    medication_score * 100.0,
                    min_med * 100.0
                ),
            };
        }

        let weights = self.weights.read().unwrap();

        let qty_score = self.quantity_score(offer.quantity_f64(), request.quantity_f64());
        let price_score = self.price_score(offer.price_f64(), request.max_price_f64());
        let recency_score = self.recency_score(offer.created_at);

        // Real dosage comparison - ported from Go: Scorer.DosageScore (scorer.go:189-207)
        let offer_dosage = parse_dosage(&offer.medication);
        let request_dosage = parse_dosage(&request.medication);
        let dosage_score = match (&offer_dosage, &request_dosage) {
            (None, None) => 0.9,                      // Both missing - slight penalty
            (None, Some(_)) | (Some(_), None) => 0.7, // One missing - partial penalty
            _ => compare_dosages(&offer_dosage, &request_dosage),
        };

        let total = medication_score * weights.medication
            + dosage_score * weights.dosage
            + qty_score * weights.quantity
            + price_score * weights.price
            + recency_score * weights.recency;

        let total = total.clamp(0.0, 1.0);
        let confidence = self.get_confidence_band(total);

        let breakdown = format!(
            "Med:{:.0}% Dos:{:.0}% Qty:{:.0}% Price:{:.0}% Rec:{:.0}%",
            medication_score * 100.0,
            dosage_score * 100.0,
            qty_score * 100.0,
            price_score * 100.0,
            recency_score * 100.0
        );

        MatchScore {
            medication_score,
            dosage_score,
            quantity_score: qty_score,
            price_score,
            recency_score,
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
    #[rstest]
    #[case(10.0, 10.0, 1.0)] // exact match
    #[case(20.0, 10.0, 1.0)] // offer exceeds request
    #[case(100.0, 10.0, 1.0)] // large surplus
    #[case(9.0, 10.0, 1.0)] // 90% (within tolerance)
    #[case(9.5, 10.0, 1.0)] // 95% (within tolerance)
    #[case(8.9, 10.0, 0.89)] // 89% (below tolerance)
    #[case(8.0, 10.0, 0.8)] // 80% partial
    #[case(5.0, 10.0, 0.5)] // 50% partial
    #[case(2.5, 10.0, 0.25)] // 25% partial
    #[case(10.0, 0.0, 1.0)] // zero request
    #[case(0.0, 10.0, 0.0)] // zero offer
    #[case(0.0, 0.0, 1.0)] // both zero
    fn test_quantity_score(#[case] offer: f64, #[case] request: f64, #[case] expected: f64) {
        let scorer = Scorer::default();
        let result = scorer.quantity_score(offer, request);
        assert!(
            (result - expected).abs() < 0.01,
            "got {} expected {}",
            result,
            expected
        );
    }

    // Ported from Go: TestPriceScore (scorer_test.go:56-100)
    #[rstest]
    #[case(50.0, 100.0, 1.0)] // well within budget
    #[case(100.0, 100.0, 1.0)] // at exact budget
    #[case(95.0, 100.0, 1.0)] // 95% of budget
    #[case(105.0, 100.0, 1.0)] // 105% (within tolerance)
    #[case(110.0, 100.0, 0.95)] // 110% over
    #[case(0.0, 0.0, 1.0)] // no prices
    #[case(500.0, 0.0, 0.95)] // no max, offer has price
    #[case(0.0, 100.0, 0.85)] // has max, no offer price
    fn test_price_score(#[case] offer: f64, #[case] max: f64, #[case] expected: f64) {
        let scorer = Scorer::default();
        let result = scorer.price_score(offer, max);
        assert!(
            (result - expected).abs() < 0.02,
            "got {} expected {}",
            result,
            expected
        );
    }

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
            medication: 0.6,
            dosage: 0.1,
            quantity: 0.2,
            price: 0.05,
            recency: 0.05,
        };

        scorer.update_weights(new_weights.clone());
        let got = scorer.get_weights();

        assert!((got.medication - 0.6).abs() < 0.001);
    }
}
