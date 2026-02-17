//! Pharmaceutical validator for matching
//!
//! Orchestrates concentration and form validation for pharmaceutical matching.
//! Provides a unified interface for validating pharmaceutical compatibility.

use std::sync::RwLock;

use crate::domain::{Offer, Request};

use super::{ConcentrationParser, ConcentrationValue, FormValidator};

/// Configuration for pharmaceutical validation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PharmaceuticalValidatorConfig {
    /// Concentration tolerance percentage (default: 20.0)
    /// Differences below this are considered acceptable
    pub concentration_tolerance_percent: f64,

    /// Concentration reject threshold percentage (default: 50.0)
    /// Differences above this result in rejection
    pub concentration_reject_threshold_percent: f64,

    /// Penalty when one side is missing concentration (default: 0.15)
    pub missing_concentration_penalty: f64,

    /// Penalty when one side is missing form (default: 0.20)
    pub missing_form_penalty: f64,

    /// Enable concentration validation (default: true)
    pub enable_concentration_check: bool,

    /// Enable form validation (default: true)
    pub enable_form_check: bool,
}

impl Default for PharmaceuticalValidatorConfig {
    fn default() -> Self {
        Self {
            concentration_tolerance_percent: 20.0,
            concentration_reject_threshold_percent: 50.0,
            missing_concentration_penalty: 0.15,
            missing_form_penalty: 0.20,
            enable_concentration_check: true,
            enable_form_check: true,
        }
    }
}

/// Result of concentration validation check
#[derive(Debug, Clone)]
pub struct ConcentrationCheckResult {
    pub offer_value: Option<ConcentrationValue>,
    pub request_value: Option<ConcentrationValue>,
    pub difference_percent: Option<f64>,
    pub penalty: f64,
    pub compatible: bool,
}

/// Result of form validation check
#[derive(Debug, Clone)]
pub struct FormCheckResult {
    pub offer_form: Option<String>,
    pub request_form: Option<String>,
    pub compatible: bool,
    pub penalty: f64,
}

/// Complete pharmaceutical validation result
#[derive(Debug, Clone)]
pub struct PharmaceuticalValidationResult {
    /// Whether validation passed (no rejection)
    pub passed: bool,

    /// Combined compatibility score (0.0 to 1.0)
    pub score: f64,

    /// Concentration check result (if enabled)
    pub concentration_check: Option<ConcentrationCheckResult>,

    /// Form check result (if enabled)
    pub form_check: Option<FormCheckResult>,

    /// Rejection reason (if rejected)
    pub rejection_reason: Option<String>,
}

/// Statistics for pharmaceutical validation
#[derive(Debug, Clone, Default)]
pub struct PharmaceuticalValidationStats {
    pub total_validations: u64,
    pub concentration_rejections: u64,
    pub form_rejections: u64,
    pub passed_validations: u64,
}

/// Snapshot of pharmaceutical validation statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct PharmaceuticalValidationStatsSnapshot {
    pub total_validations: u64,
    pub concentration_rejections: u64,
    pub form_rejections: u64,
    pub passed_validations: u64,
    pub rejection_rate: f64,
}

impl PharmaceuticalValidationStats {
    pub fn snapshot(&self) -> PharmaceuticalValidationStatsSnapshot {
        let rejection_rate = if self.total_validations > 0 {
            ((self.concentration_rejections + self.form_rejections) as f64
                / self.total_validations as f64)
                * 100.0
        } else {
            0.0
        };

        PharmaceuticalValidationStatsSnapshot {
            total_validations: self.total_validations,
            concentration_rejections: self.concentration_rejections,
            form_rejections: self.form_rejections,
            passed_validations: self.passed_validations,
            rejection_rate,
        }
    }
}

/// Pharmaceutical validator that orchestrates all validation checks
pub struct PharmaceuticalValidator {
    config: RwLock<PharmaceuticalValidatorConfig>,
    concentration_parser: ConcentrationParser,
    form_validator: FormValidator,
    stats: RwLock<PharmaceuticalValidationStats>,
}

impl Default for PharmaceuticalValidator {
    fn default() -> Self {
        Self::new(PharmaceuticalValidatorConfig::default())
    }
}

impl PharmaceuticalValidator {
    /// Create a new pharmaceutical validator with the given configuration
    pub fn new(config: PharmaceuticalValidatorConfig) -> Self {
        Self {
            config: RwLock::new(config),
            concentration_parser: ConcentrationParser::new(),
            form_validator: FormValidator::new(),
            stats: RwLock::new(PharmaceuticalValidationStats::default()),
        }
    }

    /// Validate pharmaceutical compatibility between offer and request
    pub fn validate(&self, offer: &Offer, request: &Request) -> PharmaceuticalValidationResult {
        let config = self.config.read().unwrap();
        let mut stats = self.stats.write().unwrap();
        stats.total_validations += 1;

        let mut score = 1.0;
        let mut rejection_reason = None;

        // 1. Concentration validation
        let concentration_check = if config.enable_concentration_check {
            let offer_conc = offer
                .concentration
                .as_ref()
                .and_then(|c| self.concentration_parser.parse(c));
            let request_conc = request
                .concentration
                .as_ref()
                .and_then(|c| self.concentration_parser.parse(c));

            let result = match (offer_conc.as_ref(), request_conc.as_ref()) {
                (Some(oc), Some(rc)) => {
                    let diff = self.concentration_parser.difference_percent(oc, rc);
                    let penalty = if diff > config.concentration_reject_threshold_percent {
                        stats.concentration_rejections += 1;
                        rejection_reason = Some(format!(
                            "Concentration difference {:.1}% exceeds threshold {:.1}%",
                            diff, config.concentration_reject_threshold_percent
                        ));
                        1.0 // Full penalty = rejection
                    } else if diff > config.concentration_tolerance_percent {
                        // Graduated penalty: tolerance to reject threshold
                        let range = config.concentration_reject_threshold_percent
                            - config.concentration_tolerance_percent;
                        let excess = diff - config.concentration_tolerance_percent;
                        0.3 + (excess / range) * 0.7
                    } else {
                        0.0 // No penalty
                    };

                    score *= 1.0 - penalty;

                    ConcentrationCheckResult {
                        offer_value: Some(oc.clone()),
                        request_value: Some(rc.clone()),
                        difference_percent: Some(diff),
                        penalty,
                        compatible: penalty < 1.0,
                    }
                }
                (None, Some(_)) | (Some(_), None) => {
                    // One side missing - moderate penalty
                    score *= 1.0 - config.missing_concentration_penalty;
                    ConcentrationCheckResult {
                        offer_value: offer_conc,
                        request_value: request_conc,
                        difference_percent: None,
                        penalty: config.missing_concentration_penalty,
                        compatible: true,
                    }
                }
                (None, None) => {
                    // Both missing - no penalty
                    ConcentrationCheckResult {
                        offer_value: None,
                        request_value: None,
                        difference_percent: None,
                        penalty: 0.0,
                        compatible: true,
                    }
                }
            };

            Some(result)
        } else {
            None
        };

        // 2. Form validation
        let form_check = if config.enable_form_check {
            let result = match (&offer.form, &request.form) {
                (Some(of), Some(rf)) => {
                    let compat = self.form_validator.are_compatible(of, rf);
                    score *= 1.0 - compat.penalty;

                    if !compat.compatible {
                        stats.form_rejections += 1;
                        rejection_reason = rejection_reason.or_else(|| Some(compat.reason.clone()));
                    }

                    FormCheckResult {
                        offer_form: Some(of.clone()),
                        request_form: Some(rf.clone()),
                        compatible: compat.compatible,
                        penalty: compat.penalty,
                    }
                }
                (None, Some(_)) | (Some(_), None) => {
                    score *= 1.0 - config.missing_form_penalty;
                    FormCheckResult {
                        offer_form: offer.form.clone(),
                        request_form: request.form.clone(),
                        compatible: true,
                        penalty: config.missing_form_penalty,
                    }
                }
                (None, None) => FormCheckResult {
                    offer_form: None,
                    request_form: None,
                    compatible: true,
                    penalty: 0.0,
                },
            };

            Some(result)
        } else {
            None
        };

        if rejection_reason.is_none() {
            stats.passed_validations += 1;
        }

        PharmaceuticalValidationResult {
            passed: rejection_reason.is_none(),
            score: score.max(0.0),
            concentration_check,
            form_check,
            rejection_reason,
        }
    }

    /// Calculate pharmaceutical compatibility score (0.0 to 1.0)
    pub fn calculate_score(&self, offer: &Offer, request: &Request) -> f64 {
        self.validate(offer, request).score
    }

    /// Check if match should be rejected based on validation
    ///
    /// Returns (should_reject, rejection_reason)
    pub fn should_reject(&self, offer: &Offer, request: &Request) -> (bool, Option<String>) {
        let result = self.validate(offer, request);
        (!result.passed, result.rejection_reason)
    }

    /// Get current configuration
    pub fn get_config(&self) -> PharmaceuticalValidatorConfig {
        self.config.read().unwrap().clone()
    }

    /// Set new configuration
    pub fn set_config(&self, config: PharmaceuticalValidatorConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get validation statistics snapshot
    pub fn get_stats(&self) -> PharmaceuticalValidationStatsSnapshot {
        self.stats.read().unwrap().snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.stats.write().unwrap() = PharmaceuticalValidationStats::default();
    }
}
