//! Uncertainty Estimator
//!
//! Estimates prediction uncertainty using Monte Carlo weight perturbation.
//! This helps identify matches where the model is uncertain, even if the
//! score is high.
//!
//! Features:
//! - Weight perturbation sampling
//! - Mean score and standard deviation calculation
//! - Confidence intervals
//! - Uncertainty-based filtering

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::domain::{Offer, Request};
use crate::matching::fuzzy::medication_similarity;
use crate::matching::{Scorer, Weights};

/// Type alias for match with uncertainty result tuple
pub type MatchWithUncertainty = (Offer, Request, f64, UncertaintyResult);

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for uncertainty estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncertaintyConfig {
    /// Number of Monte Carlo samples
    pub num_samples: usize,
    /// Standard deviation for weight perturbation (as fraction of weight)
    pub perturbation_std: f64,
    /// Confidence level for intervals (e.g., 0.95 for 95% CI)
    pub confidence_level: f64,
    /// Maximum allowed uncertainty (std dev) for auto-approval
    pub max_uncertainty_threshold: f64,
    /// Whether to use symmetric perturbation (both + and -)
    pub symmetric_perturbation: bool,
}

impl Default for UncertaintyConfig {
    fn default() -> Self {
        Self {
            num_samples: 20,
            perturbation_std: 0.1, // 10% perturbation
            confidence_level: 0.95,
            max_uncertainty_threshold: 0.15,
            symmetric_perturbation: true,
        }
    }
}

impl UncertaintyConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            num_samples: std::env::var("UNCERTAINTY_NUM_SAMPLES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            perturbation_std: std::env::var("UNCERTAINTY_PERTURBATION_STD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.1),
            confidence_level: std::env::var("UNCERTAINTY_CONFIDENCE_LEVEL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.95),
            max_uncertainty_threshold: std::env::var("UNCERTAINTY_MAX_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.15),
            symmetric_perturbation: std::env::var("UNCERTAINTY_SYMMETRIC")
                .map(|v| v != "false")
                .unwrap_or(true),
        }
    }

    /// Create a fast config with fewer samples
    pub fn fast() -> Self {
        Self {
            num_samples: 5,
            ..Default::default()
        }
    }

    /// Create a thorough config with more samples
    pub fn thorough() -> Self {
        Self {
            num_samples: 50,
            perturbation_std: 0.15,
            ..Default::default()
        }
    }
}

// =============================================================================
// Uncertainty Result
// =============================================================================

/// Result of uncertainty estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncertaintyResult {
    /// Mean score across all samples
    pub mean_score: f64,
    /// Standard deviation of scores
    pub std_dev: f64,
    /// Coefficient of variation (std_dev / mean)
    pub coefficient_of_variation: f64,
    /// Lower bound of confidence interval
    pub ci_lower: f64,
    /// Upper bound of confidence interval
    pub ci_upper: f64,
    /// Number of samples used
    pub num_samples: usize,
    /// All sampled scores (for debugging)
    pub samples: Vec<f64>,
    /// Whether uncertainty is within acceptable threshold
    pub is_certain: bool,
    /// Original score without perturbation
    pub original_score: f64,
}

impl UncertaintyResult {
    /// Get the uncertainty level as a descriptive string
    pub fn uncertainty_level(&self) -> &'static str {
        if self.std_dev < 0.05 {
            "very_low"
        } else if self.std_dev < 0.10 {
            "low"
        } else if self.std_dev < 0.15 {
            "moderate"
        } else if self.std_dev < 0.20 {
            "high"
        } else {
            "very_high"
        }
    }

    /// Check if the score is robust (mean close to original)
    pub fn is_robust(&self, tolerance: f64) -> bool {
        (self.mean_score - self.original_score).abs() < tolerance
    }

    /// Get the worst-case score (lower CI bound)
    pub fn worst_case_score(&self) -> f64 {
        self.ci_lower
    }

    /// Get the best-case score (upper CI bound)
    pub fn best_case_score(&self) -> f64 {
        self.ci_upper
    }
}

// =============================================================================
// Uncertainty Estimator
// =============================================================================

/// Estimates prediction uncertainty using Monte Carlo weight perturbation
pub struct UncertaintyEstimator {
    config: UncertaintyConfig,
    base_weights: Weights,
}

impl UncertaintyEstimator {
    /// Create a new uncertainty estimator
    pub fn new(config: UncertaintyConfig, base_weights: Weights) -> Self {
        Self {
            config,
            base_weights,
        }
    }

    /// Create with default config
    pub fn with_weights(base_weights: Weights) -> Self {
        Self::new(UncertaintyConfig::default(), base_weights)
    }

    /// Estimate uncertainty for a match
    pub fn estimate(&self, offer: &Offer, request: &Request) -> UncertaintyResult {
        let mut rng = rand::rng();
        let mut samples = Vec::with_capacity(self.config.num_samples);

        // Calculate medication similarity once (it doesn't depend on weights)
        let med_score = medication_similarity(&offer.medication, &request.medication);

        // Create scorer with base weights and get original score
        let base_scorer = Scorer::new(Some(self.base_weights.clone()), None);
        let original_match = base_scorer.score_match(offer, request, med_score, None);
        let original_score = original_match.total;

        // Generate perturbed samples
        for _ in 0..self.config.num_samples {
            let perturbed_weights = self.perturb_weights(&mut rng);
            let scorer = Scorer::new(Some(perturbed_weights), None);
            let match_result = scorer.score_match(offer, request, med_score, None);
            samples.push(match_result.total);
        }

        // If no valid samples, return with original score
        if samples.is_empty() {
            return UncertaintyResult {
                mean_score: original_score,
                std_dev: 0.0,
                coefficient_of_variation: 0.0,
                ci_lower: original_score,
                ci_upper: original_score,
                num_samples: 0,
                samples: vec![],
                is_certain: true,
                original_score,
            };
        }

        // Calculate statistics
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance =
            samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let std_dev = variance.sqrt();
        let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };

        // Calculate confidence interval
        let (ci_lower, ci_upper) = self.calculate_confidence_interval(&samples, mean, std_dev);

        UncertaintyResult {
            mean_score: mean,
            std_dev,
            coefficient_of_variation: cv,
            ci_lower,
            ci_upper,
            num_samples: samples.len(),
            samples,
            is_certain: std_dev <= self.config.max_uncertainty_threshold,
            original_score,
        }
    }

    /// Perturb weights using Gaussian noise
    fn perturb_weights<R: Rng>(&self, rng: &mut R) -> Weights {
        let perturbation = |w: f64, rng: &mut R| -> f64 {
            let noise = if self.config.symmetric_perturbation {
                // Box-Muller transform for Gaussian
                let u1: f64 = rng.random();
                let u2: f64 = rng.random();
                (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
            } else {
                // Uniform perturbation
                rng.random::<f64>() * 2.0 - 1.0
            };

            let perturbed = w + noise * self.config.perturbation_std * w;
            perturbed.max(0.001) // Ensure positive
        };

        // Perturb each weight
        let medication = perturbation(self.base_weights.medication, rng);
        let recency = perturbation(self.base_weights.recency, rng);
        let expiry = perturbation(self.base_weights.expiry, rng);

        // Normalize to sum to 1.0
        let sum = medication + recency + expiry;
        Weights {
            medication: medication / sum,
            pharmaceutical: self.base_weights.pharmaceutical,
            recency: recency / sum,
            expiry: expiry / sum,
            supplier: self.base_weights.supplier,
            ai_logic: self.base_weights.ai_logic,
        }
    }

    /// Calculate confidence interval using percentile method
    fn calculate_confidence_interval(
        &self,
        samples: &[f64],
        mean: f64,
        std_dev: f64,
    ) -> (f64, f64) {
        if samples.len() < 2 {
            return (mean, mean);
        }

        // Use percentile method for small samples
        if samples.len() < 30 {
            let mut sorted = samples.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let alpha = 1.0 - self.config.confidence_level;
            let lower_idx = ((alpha / 2.0) * samples.len() as f64).floor() as usize;
            let upper_idx = ((1.0 - alpha / 2.0) * samples.len() as f64).ceil() as usize - 1;

            let lower = sorted.get(lower_idx).copied().unwrap_or(mean - std_dev);
            let upper = sorted.get(upper_idx).copied().unwrap_or(mean + std_dev);

            return (lower, upper);
        }

        // Use normal approximation for larger samples
        let z = match self.config.confidence_level {
            l if l >= 0.99 => 2.576,
            l if l >= 0.95 => 1.96,
            l if l >= 0.90 => 1.645,
            _ => 1.96,
        };

        let margin = z * std_dev / (samples.len() as f64).sqrt();
        (mean - margin, mean + margin)
    }

    /// Batch estimate uncertainty for multiple matches
    pub fn estimate_batch(&self, matches: &[(Offer, Request)]) -> Vec<UncertaintyResult> {
        matches
            .iter()
            .map(|(offer, request)| self.estimate(offer, request))
            .collect()
    }

    /// Filter matches by uncertainty threshold
    pub fn filter_certain(
        &self,
        matches: Vec<(Offer, Request, f64)>,
    ) -> (Vec<MatchWithUncertainty>, Vec<MatchWithUncertainty>) {
        let mut certain = Vec::new();
        let mut uncertain = Vec::new();

        for (offer, request, score) in matches {
            let result = self.estimate(&offer, &request);
            if result.is_certain {
                certain.push((offer, request, score, result));
            } else {
                uncertain.push((offer, request, score, result));
            }
        }

        (certain, uncertain)
    }

    /// Get config
    pub fn config(&self) -> &UncertaintyConfig {
        &self.config
    }

    /// Estimate with custom scorer (for integration with existing scoring)
    pub fn estimate_with_scorer(
        &self,
        offer: &Offer,
        request: &Request,
        scorer: &Scorer,
    ) -> UncertaintyResult {
        let mut rng = rand::rng();
        let mut samples = Vec::with_capacity(self.config.num_samples);

        // Calculate medication similarity once
        let med_score = medication_similarity(&offer.medication, &request.medication);

        // Get original score
        let original_match = scorer.score_match(offer, request, med_score, None);
        let original_score = original_match.total;

        // Generate perturbed samples
        for _ in 0..self.config.num_samples {
            let perturbed_weights = self.perturb_weights(&mut rng);
            let perturbed_scorer = Scorer::new(Some(perturbed_weights), None);
            let match_result = perturbed_scorer.score_match(offer, request, med_score, None);
            samples.push(match_result.total);
        }

        self.build_result(samples, original_score)
    }

    /// Build result from samples
    fn build_result(&self, samples: Vec<f64>, original_score: f64) -> UncertaintyResult {
        if samples.is_empty() {
            return UncertaintyResult {
                mean_score: original_score,
                std_dev: 0.0,
                coefficient_of_variation: 0.0,
                ci_lower: original_score,
                ci_upper: original_score,
                num_samples: 0,
                samples: vec![],
                is_certain: true,
                original_score,
            };
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance =
            samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let std_dev = variance.sqrt();
        let cv = if mean > 0.0 { std_dev / mean } else { 0.0 };
        let (ci_lower, ci_upper) = self.calculate_confidence_interval(&samples, mean, std_dev);

        UncertaintyResult {
            mean_score: mean,
            std_dev,
            coefficient_of_variation: cv,
            ci_lower,
            ci_upper,
            num_samples: samples.len(),
            samples,
            is_certain: std_dev <= self.config.max_uncertainty_threshold,
            original_score,
        }
    }
}

// =============================================================================
// Ensemble Uncertainty (using multiple scorers)
// =============================================================================

/// Ensemble-based uncertainty using variance across different scoring strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleUncertaintyResult {
    /// Mean score across ensemble
    pub mean_score: f64,
    /// Standard deviation across ensemble
    pub std_dev: f64,
    /// Individual scores from each strategy
    pub strategy_scores: Vec<(String, f64)>,
    /// Agreement level (1.0 = all agree, 0.0 = max disagreement)
    pub agreement: f64,
}

/// Calculates ensemble uncertainty from multiple scoring strategies
pub struct EnsembleUncertainty;

impl EnsembleUncertainty {
    /// Calculate uncertainty from multiple strategy scores
    pub fn from_scores(strategy_scores: Vec<(String, f64)>) -> EnsembleUncertaintyResult {
        if strategy_scores.is_empty() {
            return EnsembleUncertaintyResult {
                mean_score: 0.0,
                std_dev: 0.0,
                strategy_scores: vec![],
                agreement: 1.0,
            };
        }

        let scores: Vec<f64> = strategy_scores.iter().map(|(_, s)| *s).collect();
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        let std_dev = variance.sqrt();

        // Agreement: 1 - normalized std dev (assuming scores are 0-1)
        let agreement = (1.0 - std_dev.min(0.5) * 2.0).max(0.0);

        EnsembleUncertaintyResult {
            mean_score: mean,
            std_dev,
            strategy_scores,
            agreement,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_offer() -> Offer {
        Offer {
            id: uuid::Uuid::new_v4(),
            medication: "Aspirin 100mg".to_string(),
            ..Default::default()
        }
    }

    fn create_test_request() -> Request {
        Request {
            id: uuid::Uuid::new_v4(),
            medication: "Aspirin 100mg".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_uncertainty_estimation() {
        let weights = Weights::default();
        let estimator = UncertaintyEstimator::new(UncertaintyConfig::fast(), weights);

        let offer = create_test_offer();
        let request = create_test_request();

        let result = estimator.estimate(&offer, &request);

        assert!(result.num_samples > 0);
        assert!(result.mean_score >= 0.0 && result.mean_score <= 1.0);
        assert!(result.std_dev >= 0.0);
        assert!(result.ci_lower <= result.mean_score);
        assert!(result.ci_upper >= result.mean_score);
    }

    #[test]
    fn test_uncertainty_levels() {
        let result = UncertaintyResult {
            mean_score: 0.8,
            std_dev: 0.03,
            coefficient_of_variation: 0.0375,
            ci_lower: 0.75,
            ci_upper: 0.85,
            num_samples: 20,
            samples: vec![],
            is_certain: true,
            original_score: 0.8,
        };

        assert_eq!(result.uncertainty_level(), "very_low");

        let high_uncertainty = UncertaintyResult {
            std_dev: 0.18,
            ..result.clone()
        };
        assert_eq!(high_uncertainty.uncertainty_level(), "high");
    }

    #[test]
    fn test_ensemble_uncertainty() {
        let scores = vec![
            ("exact".to_string(), 0.95),
            ("fuzzy".to_string(), 0.88),
            ("embedding".to_string(), 0.92),
        ];

        let result = EnsembleUncertainty::from_scores(scores);

        assert!((result.mean_score - 0.9167).abs() < 0.01);
        assert!(result.std_dev > 0.0);
        assert!(result.agreement > 0.5);
    }

    #[test]
    fn test_config_from_env() {
        // Just verify it doesn't panic
        let config = UncertaintyConfig::from_env();
        assert!(config.num_samples > 0);
        assert!(config.perturbation_std > 0.0);
    }

    #[test]
    fn test_robustness_check() {
        let result = UncertaintyResult {
            mean_score: 0.82,
            std_dev: 0.05,
            coefficient_of_variation: 0.061,
            ci_lower: 0.75,
            ci_upper: 0.89,
            num_samples: 20,
            samples: vec![],
            is_certain: true,
            original_score: 0.80,
        };

        assert!(result.is_robust(0.05)); // Within 5% tolerance
        assert!(!result.is_robust(0.01)); // Not within 1% tolerance
    }

    #[test]
    fn test_perturbed_weights_normalized() {
        let weights = Weights::default();
        let estimator = UncertaintyEstimator::new(UncertaintyConfig::default(), weights);
        let mut rng = rand::rng();

        for _ in 0..10 {
            let perturbed = estimator.perturb_weights(&mut rng);
            let sum = perturbed.medication + perturbed.recency + perturbed.expiry; // dosage removed
            assert!((sum - 1.0).abs() < 0.001, "Weights should sum to 1.0");
        }
    }
}
