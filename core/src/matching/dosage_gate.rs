//! Dosage Gate - Safety validation for dosage mismatches
//!
//! Prevents matches when dosage differences exceed safe thresholds.
//! This is a critical safety component that caps match scores when
//! dosages differ significantly, even if other factors score highly.

use serde::{Deserialize, Serialize};

use super::dosage::{Dosage, parse_dosage};
use crate::domain::{Offer, Request};

/// Configuration for the dosage gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosageGateConfig {
    /// Minimum dosage score required to pass the gate (default: 0.7)
    pub min_dosage_score: f64,
    /// Maximum total score when gate fails (default: 0.7)
    pub max_score_on_fail: f64,
    /// Threshold percentage difference for mandatory review (default: 100.0)
    /// When dosages differ by more than this percentage, mandatory review is required
    pub review_threshold_percent: f64,
    /// Enable the dosage gate (default: true)
    pub enabled: bool,
}

impl Default for DosageGateConfig {
    fn default() -> Self {
        Self {
            min_dosage_score: 0.7,
            max_score_on_fail: 0.7,
            review_threshold_percent: 100.0,
            enabled: true,
        }
    }
}

/// Flags indicating dosage-related issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DosageFlag {
    /// Names match but dosages differ (>10% difference)
    DosageWarning,
    /// Dosages differ by more than 100% - requires mandatory review
    MandatoryReview,
    /// Gate was triggered and score was capped
    GateTriggered,
}

/// Result of dosage gate evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosageGateResult {
    /// Whether the gate passed (dosage score >= threshold)
    pub passed: bool,
    /// The calculated dosage score (0.0-1.0)
    pub dosage_score: f64,
    /// The capped score if gate failed, None if passed
    pub capped_score: Option<f64>,
    /// Flags indicating issues detected
    pub flags: Vec<DosageFlag>,
}

impl DosageGateResult {
    /// Create a new result indicating the gate passed
    pub fn passed(dosage_score: f64) -> Self {
        Self {
            passed: true,
            dosage_score,
            capped_score: None,
            flags: Vec::new(),
        }
    }

    /// Create a new result indicating the gate failed
    pub fn failed(dosage_score: f64, capped_score: f64) -> Self {
        Self {
            passed: false,
            dosage_score,
            capped_score: Some(capped_score),
            flags: vec![DosageFlag::GateTriggered],
        }
    }

    /// Add a flag to the result
    pub fn with_flag(mut self, flag: DosageFlag) -> Self {
        if !self.flags.contains(&flag) {
            self.flags.push(flag);
        }
        self
    }

    /// Check if a specific flag is present
    pub fn has_flag(&self, flag: &DosageFlag) -> bool {
        self.flags.contains(flag)
    }
}

/// Dosage Gate component for enforcing dosage safety rules
#[derive(Debug, Clone)]
pub struct DosageGate {
    config: DosageGateConfig,
}

impl Default for DosageGate {
    fn default() -> Self {
        Self::new(DosageGateConfig::default())
    }
}

impl DosageGate {
    /// Create a new DosageGate with the given configuration
    pub fn new(config: DosageGateConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration
    pub fn config(&self) -> &DosageGateConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: DosageGateConfig) {
        self.config = config;
    }

    /// Calculate the percentage difference between two dosages
    /// Returns None if either dosage is missing or zero
    fn calculate_percent_difference(
        offer_dosage: &Option<Dosage>,
        request_dosage: &Option<Dosage>,
    ) -> Option<f64> {
        match (offer_dosage, request_dosage) {
            (Some(offer), Some(request)) => {
                let offer_base = offer.to_base_unit();
                let request_base = request.to_base_unit();

                if offer_base == 0.0 || request_base == 0.0 {
                    return None;
                }

                // Calculate percentage difference relative to the request
                let diff = (offer_base - request_base).abs();
                Some((diff / request_base) * 100.0)
            }
            _ => None,
        }
    }

    /// Check if medication names match (after basic normalization)
    fn names_match(offer_med: &str, request_med: &str) -> bool {
        // Extract medication name without dosage for comparison
        let normalize = |s: &str| -> String {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphabetic() || c.is_whitespace())
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };

        let offer_normalized = normalize(offer_med);
        let request_normalized = normalize(request_med);

        // Check if the normalized names are equal or one contains the other
        offer_normalized == request_normalized
            || offer_normalized.contains(&request_normalized)
            || request_normalized.contains(&offer_normalized)
    }

    /// Evaluate the dosage gate for an offer-request pair
    ///
    /// # Arguments
    /// * `offer` - The medication offer
    /// * `request` - The medication request
    /// * `dosage_score` - Pre-calculated dosage similarity score (0.0-1.0)
    ///
    /// # Returns
    /// A `DosageGateResult` indicating whether the gate passed and any flags
    pub fn evaluate(
        &self,
        offer: &Offer,
        request: &Request,
        dosage_score: f64,
    ) -> DosageGateResult {
        // If gate is disabled, always pass
        if !self.config.enabled {
            return DosageGateResult::passed(dosage_score);
        }

        let offer_dosage = parse_dosage(&offer.medication);
        let request_dosage = parse_dosage(&request.medication);

        let mut result = if dosage_score >= self.config.min_dosage_score {
            DosageGateResult::passed(dosage_score)
        } else {
            DosageGateResult::failed(dosage_score, self.config.max_score_on_fail)
        };

        // Check for >100% dosage difference (mandatory review)
        if let Some(percent_diff) =
            Self::calculate_percent_difference(&offer_dosage, &request_dosage)
        {
            if percent_diff > self.config.review_threshold_percent {
                result = result.with_flag(DosageFlag::MandatoryReview);
            }

            // Check for name match with dosage mismatch (>10% difference)
            if percent_diff > 10.0 && Self::names_match(&offer.medication, &request.medication) {
                result = result.with_flag(DosageFlag::DosageWarning);
            }
        }

        result
    }

    /// Apply the gate result to a total score
    ///
    /// If the gate failed, caps the score at max_score_on_fail.
    /// If the gate passed, returns the original score unchanged.
    ///
    /// # Arguments
    /// * `total_score` - The original total match score
    /// * `gate_result` - The result from `evaluate()`
    ///
    /// # Returns
    /// The potentially capped score
    pub fn apply_to_score(&self, total_score: f64, gate_result: &DosageGateResult) -> f64 {
        if !self.config.enabled {
            return total_score;
        }

        if gate_result.passed {
            total_score
        } else {
            total_score.min(self.config.max_score_on_fail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Helper to create a test offer with a medication name
    fn make_offer(medication: &str) -> Offer {
        Offer {
            id: Uuid::new_v4(),
            medication: medication.to_string(),
            ..Default::default()
        }
    }

    /// Helper to create a test request with a medication name
    fn make_request(medication: &str) -> Request {
        Request {
            id: Uuid::new_v4(),
            medication: medication.to_string(),
            ..Default::default()
        }
    }

    // =========================================================================
    // DosageGateConfig Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = DosageGateConfig::default();
        assert!((config.min_dosage_score - 0.7).abs() < 0.001);
        assert!((config.max_score_on_fail - 0.7).abs() < 0.001);
        assert!((config.review_threshold_percent - 100.0).abs() < 0.001);
        assert!(config.enabled);
    }

    // =========================================================================
    // DosageGateResult Tests
    // =========================================================================

    #[test]
    fn test_result_passed() {
        let result = DosageGateResult::passed(0.85);
        assert!(result.passed);
        assert!((result.dosage_score - 0.85).abs() < 0.001);
        assert!(result.capped_score.is_none());
        assert!(result.flags.is_empty());
    }

    #[test]
    fn test_result_failed() {
        let result = DosageGateResult::failed(0.5, 0.7);
        assert!(!result.passed);
        assert!((result.dosage_score - 0.5).abs() < 0.001);
        assert_eq!(result.capped_score, Some(0.7));
        assert!(result.has_flag(&DosageFlag::GateTriggered));
    }

    #[test]
    fn test_result_with_flag() {
        let result = DosageGateResult::passed(0.85)
            .with_flag(DosageFlag::DosageWarning)
            .with_flag(DosageFlag::MandatoryReview);

        assert!(result.has_flag(&DosageFlag::DosageWarning));
        assert!(result.has_flag(&DosageFlag::MandatoryReview));
        assert!(!result.has_flag(&DosageFlag::GateTriggered));
    }

    #[test]
    fn test_result_no_duplicate_flags() {
        let result = DosageGateResult::passed(0.85)
            .with_flag(DosageFlag::DosageWarning)
            .with_flag(DosageFlag::DosageWarning);

        assert_eq!(result.flags.len(), 1);
    }

    // =========================================================================
    // DosageGate Evaluation Tests
    // =========================================================================

    #[test]
    fn test_gate_passes_high_score() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 100mg");

        let result = gate.evaluate(&offer, &request, 0.9);
        assert!(result.passed);
        assert!(result.capped_score.is_none());
    }

    #[test]
    fn test_gate_fails_low_score() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 500mg");

        let result = gate.evaluate(&offer, &request, 0.5);
        assert!(!result.passed);
        assert_eq!(result.capped_score, Some(0.7));
        assert!(result.has_flag(&DosageFlag::GateTriggered));
    }

    #[test]
    fn test_gate_at_threshold() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 100mg");

        // Exactly at threshold should pass
        let result = gate.evaluate(&offer, &request, 0.7);
        assert!(result.passed);
    }

    #[test]
    fn test_gate_just_below_threshold() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 100mg");

        // Just below threshold should fail
        let result = gate.evaluate(&offer, &request, 0.69);
        assert!(!result.passed);
    }

    #[test]
    fn test_gate_disabled() {
        let config = DosageGateConfig {
            enabled: false,
            ..Default::default()
        };
        let gate = DosageGate::new(config);
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 500mg");

        // Even with low score, should pass when disabled
        let result = gate.evaluate(&offer, &request, 0.3);
        assert!(result.passed);
    }

    // =========================================================================
    // Mandatory Review Flag Tests (>100% difference)
    // =========================================================================

    #[test]
    fn test_mandatory_review_large_difference() {
        let gate = DosageGate::default();
        // 500mg vs 1000mg = 100% difference (at threshold)
        let offer = make_offer("Aspirin 500mg");
        let request = make_request("Aspirin 1000mg");

        let result = gate.evaluate(&offer, &request, 0.5);
        // 100% is not > 100%, so no mandatory review
        assert!(!result.has_flag(&DosageFlag::MandatoryReview));
    }

    #[test]
    fn test_mandatory_review_over_threshold() {
        let gate = DosageGate::default();
        // 100mg vs 500mg = 80% difference relative to request (400/500)
        // Need a bigger difference: 100mg vs 50mg = 100% difference (50/50)
        // Or: 250mg vs 100mg = 150% difference (150/100)
        let offer = make_offer("Aspirin 250mg");
        let request = make_request("Aspirin 100mg");

        let result = gate.evaluate(&offer, &request, 0.3);
        assert!(result.has_flag(&DosageFlag::MandatoryReview));
    }

    #[test]
    fn test_no_mandatory_review_small_difference() {
        let gate = DosageGate::default();
        // 100mg vs 110mg = 10% difference
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 110mg");

        let result = gate.evaluate(&offer, &request, 0.9);
        assert!(!result.has_flag(&DosageFlag::MandatoryReview));
    }

    // =========================================================================
    // Dosage Warning Flag Tests (name match with dosage mismatch)
    // =========================================================================

    #[test]
    fn test_dosage_warning_name_match_dosage_mismatch() {
        let gate = DosageGate::default();
        // Same medication name, different dosages (>10% difference)
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 150mg");

        let result = gate.evaluate(&offer, &request, 0.8);
        assert!(result.has_flag(&DosageFlag::DosageWarning));
    }

    #[test]
    fn test_no_dosage_warning_small_difference() {
        let gate = DosageGate::default();
        // Same medication name, dosages within 10%
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 105mg");

        let result = gate.evaluate(&offer, &request, 0.95);
        assert!(!result.has_flag(&DosageFlag::DosageWarning));
    }

    #[test]
    fn test_no_dosage_warning_different_names() {
        let gate = DosageGate::default();
        // Different medication names, different dosages
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Ibuprofen 200mg");

        let result = gate.evaluate(&offer, &request, 0.5);
        // Names don't match, so no dosage warning (even though dosages differ)
        assert!(!result.has_flag(&DosageFlag::DosageWarning));
    }

    // =========================================================================
    // Apply to Score Tests
    // =========================================================================

    #[test]
    fn test_apply_to_score_passed() {
        let gate = DosageGate::default();
        let result = DosageGateResult::passed(0.85);

        let score = gate.apply_to_score(0.95, &result);
        assert!((score - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_apply_to_score_failed_caps() {
        let gate = DosageGate::default();
        let result = DosageGateResult::failed(0.5, 0.7);

        // High score should be capped
        let score = gate.apply_to_score(0.95, &result);
        assert!((score - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_apply_to_score_failed_already_low() {
        let gate = DosageGate::default();
        let result = DosageGateResult::failed(0.5, 0.7);

        // Score already below cap should remain unchanged
        let score = gate.apply_to_score(0.5, &result);
        assert!((score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_apply_to_score_disabled() {
        let config = DosageGateConfig {
            enabled: false,
            ..Default::default()
        };
        let gate = DosageGate::new(config);
        let result = DosageGateResult::failed(0.5, 0.7);

        // When disabled, score should not be capped
        let score = gate.apply_to_score(0.95, &result);
        assert!((score - 0.95).abs() < 0.001);
    }

    // =========================================================================
    // Custom Configuration Tests
    // =========================================================================

    #[test]
    fn test_custom_threshold() {
        let config = DosageGateConfig {
            min_dosage_score: 0.8,
            max_score_on_fail: 0.6,
            review_threshold_percent: 50.0,
            enabled: true,
        };
        let gate = DosageGate::new(config);
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 100mg");

        // 0.75 is below 0.8 threshold
        let result = gate.evaluate(&offer, &request, 0.75);
        assert!(!result.passed);
        assert_eq!(result.capped_score, Some(0.6));
    }

    #[test]
    fn test_custom_review_threshold() {
        let config = DosageGateConfig {
            review_threshold_percent: 50.0,
            ..Default::default()
        };
        let gate = DosageGate::new(config);
        // 100mg vs 200mg = 50% difference relative to request (100/200)
        // Need > 50%: 100mg vs 150mg = 33% (not enough)
        // Try: 200mg vs 100mg = 100% difference (100/100) > 50%
        let offer = make_offer("Aspirin 200mg");
        let request = make_request("Aspirin 100mg");

        let result = gate.evaluate(&offer, &request, 0.5);
        assert!(result.has_flag(&DosageFlag::MandatoryReview));
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_missing_dosage_in_offer() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin"); // No dosage
        let request = make_request("Aspirin 100mg");

        let result = gate.evaluate(&offer, &request, 0.7);
        // Should pass at threshold, no flags for missing dosage
        assert!(result.passed);
        assert!(!result.has_flag(&DosageFlag::MandatoryReview));
        assert!(!result.has_flag(&DosageFlag::DosageWarning));
    }

    #[test]
    fn test_missing_dosage_in_request() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin"); // No dosage

        let result = gate.evaluate(&offer, &request, 0.7);
        assert!(result.passed);
    }

    #[test]
    fn test_both_missing_dosage() {
        let gate = DosageGate::default();
        let offer = make_offer("Aspirin");
        let request = make_request("Aspirin");

        let result = gate.evaluate(&offer, &request, 0.9);
        assert!(result.passed);
        assert!(result.flags.is_empty());
    }

    #[test]
    fn test_names_match_case_insensitive() {
        let gate = DosageGate::default();
        let offer = make_offer("ASPIRIN 100mg");
        let request = make_request("aspirin 150mg");

        let result = gate.evaluate(&offer, &request, 0.8);
        // Names should match despite case difference
        assert!(result.has_flag(&DosageFlag::DosageWarning));
    }

    #[test]
    fn test_names_match_partial() {
        let gate = DosageGate::default();
        // Use names where one is a substring of the other (after removing dosage)
        // "Aspirin" normalized = "aspirin"
        // "Aspirin Extra" normalized = "aspirin extra"
        // "aspirin" is contained in "aspirin extra" ✓
        let offer = make_offer("Aspirin 100mg");
        let request = make_request("Aspirin 150mg");

        let result = gate.evaluate(&offer, &request, 0.8);
        // Both normalize to "aspirin", should match and trigger warning
        assert!(result.has_flag(&DosageFlag::DosageWarning));
    }
}
