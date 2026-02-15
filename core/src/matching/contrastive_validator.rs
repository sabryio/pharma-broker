//! Contrastive Validation for False Positive Detection
//!
//! Detects false positives by ensuring the match score is significantly
//! higher than scores against random negative samples. If a match scores
//! similarly to random alternatives, it's likely a false positive.
//!
//! This implements a contrastive learning approach where we compare:
//! - Positive pair: (offer, matched_request)
//! - Negative pairs: (offer, random_requests)
//!
//! A valid match should have a score significantly higher than negatives.
//!
//! Enhanced with hard negative mining to select challenging negative samples
//! from the same therapeutic class or with similar spelling.

use std::sync::atomic::{AtomicU64, Ordering};

use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::matching::fuzzy::medication_similarity_with_raw;
use crate::matching::hard_negative::HardNegativeMiner;
use crate::repository::{OfferModel, RequestModel};

/// Configuration for contrastive validation
#[derive(Debug, Clone)]
pub struct ContrastiveConfig {
    /// Number of negative samples to compare against
    pub num_negatives: usize,
    /// Minimum margin between positive and average negative score
    pub min_margin: f64,
    /// Minimum margin between positive and max negative score
    pub min_margin_vs_max: f64,
    /// Whether contrastive validation is enabled
    pub enabled: bool,
    /// Minimum score to even consider validation (skip very low scores)
    pub min_score_threshold: f64,
    /// Whether to use hard negative mining when available
    pub use_hard_negatives: bool,
}

impl Default for ContrastiveConfig {
    fn default() -> Self {
        Self {
            num_negatives: 3,
            min_margin: 0.15,
            min_margin_vs_max: 0.10,
            enabled: true,
            min_score_threshold: 0.50,
            use_hard_negatives: true,
        }
    }
}

impl ContrastiveConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            num_negatives: std::env::var("CONTRASTIVE_NUM_NEGATIVES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            min_margin: std::env::var("CONTRASTIVE_MIN_MARGIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.15),
            min_margin_vs_max: std::env::var("CONTRASTIVE_MIN_MARGIN_VS_MAX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.10),
            enabled: std::env::var("CONTRASTIVE_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            min_score_threshold: std::env::var("CONTRASTIVE_MIN_SCORE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.50),
            use_hard_negatives: std::env::var("CONTRASTIVE_USE_HARD_NEGATIVES")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
        }
    }

    /// Strict configuration for high-precision matching
    pub fn strict() -> Self {
        Self {
            num_negatives: 5,
            min_margin: 0.20,
            min_margin_vs_max: 0.15,
            enabled: true,
            min_score_threshold: 0.50,
            use_hard_negatives: true,
        }
    }

    /// Relaxed configuration for higher recall
    pub fn relaxed() -> Self {
        Self {
            num_negatives: 2,
            min_margin: 0.10,
            min_margin_vs_max: 0.05,
            enabled: true,
            min_score_threshold: 0.40,
            use_hard_negatives: true,
        }
    }
}

/// Result of contrastive validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastiveResult {
    /// Whether the match passed validation
    pub valid: bool,
    /// The positive (match) score
    pub positive_score: f64,
    /// Average score against negative samples
    pub avg_negative_score: f64,
    /// Maximum score against negative samples
    pub max_negative_score: f64,
    /// Margin between positive and average negative
    pub margin_vs_avg: f64,
    /// Margin between positive and max negative
    pub margin_vs_max: f64,
    /// Number of negative samples used
    pub num_negatives: usize,
    /// Reason for validation result
    pub reason: String,
    /// IDs of negative samples used (for debugging)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub negative_ids: Vec<Uuid>,
}

impl ContrastiveResult {
    /// Create a result indicating validation was skipped
    pub fn skipped(reason: &str) -> Self {
        Self {
            valid: true, // Skipped = pass through
            positive_score: 0.0,
            avg_negative_score: 0.0,
            max_negative_score: 0.0,
            margin_vs_avg: 0.0,
            margin_vs_max: 0.0,
            num_negatives: 0,
            reason: reason.to_string(),
            negative_ids: vec![],
        }
    }
}

/// Statistics for contrastive validation
#[derive(Debug, Default)]
pub struct ContrastiveStats {
    pub total_validations: AtomicU64,
    pub passed: AtomicU64,
    pub failed: AtomicU64,
    pub skipped: AtomicU64,
    pub false_positives_detected: AtomicU64,
}

impl ContrastiveStats {
    pub fn snapshot(&self) -> ContrastiveStatsSnapshot {
        ContrastiveStatsSnapshot {
            total_validations: self.total_validations.load(Ordering::Relaxed),
            passed: self.passed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            skipped: self.skipped.load(Ordering::Relaxed),
            false_positives_detected: self.false_positives_detected.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.total_validations.store(0, Ordering::Relaxed);
        self.passed.store(0, Ordering::Relaxed);
        self.failed.store(0, Ordering::Relaxed);
        self.skipped.store(0, Ordering::Relaxed);
        self.false_positives_detected.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of contrastive validation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastiveStatsSnapshot {
    pub total_validations: u64,
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub false_positives_detected: u64,
}

impl ContrastiveStatsSnapshot {
    /// Calculate the false positive detection rate
    pub fn detection_rate(&self) -> f64 {
        let total = self.passed + self.failed;
        if total == 0 {
            0.0
        } else {
            self.failed as f64 / total as f64
        }
    }
}

/// Contrastive validator for detecting false positive matches
pub struct ContrastiveValidator {
    config: ContrastiveConfig,
    stats: ContrastiveStats,
    /// Optional hard negative miner for strategic sample selection
    hard_negative_miner: Option<HardNegativeMiner>,
}

impl Default for ContrastiveValidator {
    fn default() -> Self {
        Self::new(ContrastiveConfig::default())
    }
}

impl ContrastiveValidator {
    /// Create a new contrastive validator
    pub fn new(config: ContrastiveConfig) -> Self {
        Self {
            config,
            stats: ContrastiveStats::default(),
            hard_negative_miner: None,
        }
    }

    /// Create a new contrastive validator with a hard negative miner
    pub fn with_hard_negative_miner(config: ContrastiveConfig, miner: HardNegativeMiner) -> Self {
        Self {
            config,
            stats: ContrastiveStats::default(),
            hard_negative_miner: Some(miner),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(ContrastiveConfig::from_env())
    }

    /// Set the hard negative miner
    pub fn set_hard_negative_miner(&mut self, miner: HardNegativeMiner) {
        self.hard_negative_miner = Some(miner);
    }

    /// Get the hard negative miner if available
    pub fn hard_negative_miner(&self) -> Option<&HardNegativeMiner> {
        self.hard_negative_miner.as_ref()
    }

    /// Check if hard negative mining is available and ready
    pub fn has_hard_negatives(&self) -> bool {
        self.hard_negative_miner
            .as_ref()
            .is_some_and(|m| m.is_ready())
    }

    /// Validate a match using contrastive comparison
    ///
    /// Compares the match score against scores with random negative samples.
    /// If the positive score isn't significantly higher than negatives,
    /// the match is flagged as a potential false positive.
    pub fn validate(
        &self,
        offer: &OfferModel,
        matched_request: &RequestModel,
        positive_score: f64,
        negative_pool: &[RequestModel],
    ) -> ContrastiveResult {
        self.stats.total_validations.fetch_add(1, Ordering::Relaxed);

        // Check if validation is enabled
        if !self.config.enabled {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("Contrastive validation disabled");
        }

        // Skip if score is below threshold
        if positive_score < self.config.min_score_threshold {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped(&format!(
                "Score {:.2} below threshold {:.2}",
                positive_score, self.config.min_score_threshold
            ));
        }

        // Need enough negatives to sample from
        if negative_pool.len() < self.config.num_negatives {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped(&format!(
                "Insufficient negative pool ({} < {})",
                negative_pool.len(),
                self.config.num_negatives
            ));
        }

        // Sample random negatives (excluding the matched request)
        let negatives = self.sample_negatives(negative_pool, matched_request.id);

        if negatives.is_empty() {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("No valid negatives after filtering");
        }

        // Calculate scores against negatives
        let negative_scores: Vec<f64> = negatives
            .iter()
            .map(|neg| self.calculate_score(offer, neg))
            .collect();

        let negative_ids: Vec<Uuid> = negatives.iter().map(|n| n.id).collect();

        let avg_negative = negative_scores.iter().sum::<f64>() / negative_scores.len() as f64;
        let max_negative = negative_scores
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let margin_vs_avg = positive_score - avg_negative;
        let margin_vs_max = positive_score - max_negative;

        // Check if margins meet thresholds
        let valid = margin_vs_avg >= self.config.min_margin
            && margin_vs_max >= self.config.min_margin_vs_max;

        let reason = if valid {
            format!(
                "Valid: margin vs avg {:.2} >= {:.2}, margin vs max {:.2} >= {:.2}",
                margin_vs_avg, self.config.min_margin, margin_vs_max, self.config.min_margin_vs_max
            )
        } else if margin_vs_avg < self.config.min_margin {
            format!(
                "Failed: margin vs avg {:.2} < {:.2} (positive {:.2}, avg negative {:.2})",
                margin_vs_avg, self.config.min_margin, positive_score, avg_negative
            )
        } else {
            format!(
                "Failed: margin vs max {:.2} < {:.2} (positive {:.2}, max negative {:.2})",
                margin_vs_max, self.config.min_margin_vs_max, positive_score, max_negative
            )
        };

        if valid {
            self.stats.passed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.failed.fetch_add(1, Ordering::Relaxed);
            self.stats
                .false_positives_detected
                .fetch_add(1, Ordering::Relaxed);
        }

        ContrastiveResult {
            valid,
            positive_score,
            avg_negative_score: avg_negative,
            max_negative_score: max_negative,
            margin_vs_avg,
            margin_vs_max,
            num_negatives: negatives.len(),
            reason,
            negative_ids,
        }
    }

    /// Validate using embeddings for more accurate comparison
    pub fn validate_with_embeddings(
        &self,
        offer: &OfferModel,
        matched_request: &RequestModel,
        positive_score: f64,
        negative_pool: &[RequestModel],
    ) -> ContrastiveResult {
        self.stats.total_validations.fetch_add(1, Ordering::Relaxed);

        if !self.config.enabled {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("Contrastive validation disabled");
        }

        if positive_score < self.config.min_score_threshold {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped(&format!(
                "Score {:.2} below threshold {:.2}",
                positive_score, self.config.min_score_threshold
            ));
        }

        // Check if offer has embedding
        let offer_embedding = match &offer.content_embedding {
            Some(e) => e,
            None => {
                // Fall back to text-based validation
                return self.validate(offer, matched_request, positive_score, negative_pool);
            }
        };

        // Filter negatives that have embeddings
        let negatives_with_embeddings: Vec<&RequestModel> = negative_pool
            .iter()
            .filter(|r| r.id != matched_request.id && r.content_embedding.is_some())
            .collect();

        if negatives_with_embeddings.len() < self.config.num_negatives {
            // Fall back to text-based validation
            return self.validate(offer, matched_request, positive_score, negative_pool);
        }

        // Sample negatives
        let mut rng = rand::rng();
        let sampled: Vec<&RequestModel> = negatives_with_embeddings
            .choose_multiple(&mut rng, self.config.num_negatives)
            .cloned()
            .collect();

        // Calculate embedding similarities
        let negative_scores: Vec<f64> = sampled
            .iter()
            .filter_map(|neg| {
                neg.content_embedding.as_ref().map(|neg_emb| {
                    crate::matching::cosine_similarity(
                        offer_embedding.as_slice(),
                        neg_emb.as_slice(),
                    )
                    .unwrap_or(0.0)
                })
            })
            .collect();

        let negative_ids: Vec<Uuid> = sampled.iter().map(|n| n.id).collect();

        if negative_scores.is_empty() {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("No valid embedding comparisons");
        }

        let avg_negative = negative_scores.iter().sum::<f64>() / negative_scores.len() as f64;
        let max_negative = negative_scores
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let margin_vs_avg = positive_score - avg_negative;
        let margin_vs_max = positive_score - max_negative;

        let valid = margin_vs_avg >= self.config.min_margin
            && margin_vs_max >= self.config.min_margin_vs_max;

        let reason = if valid {
            format!(
                "Valid (embedding): margin vs avg {:.2} >= {:.2}",
                margin_vs_avg, self.config.min_margin
            )
        } else {
            format!(
                "Failed (embedding): margin {:.2} < {:.2}",
                margin_vs_avg.min(margin_vs_max),
                self.config.min_margin.min(self.config.min_margin_vs_max)
            )
        };

        if valid {
            self.stats.passed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.failed.fetch_add(1, Ordering::Relaxed);
            self.stats
                .false_positives_detected
                .fetch_add(1, Ordering::Relaxed);
        }

        ContrastiveResult {
            valid,
            positive_score,
            avg_negative_score: avg_negative,
            max_negative_score: max_negative,
            margin_vs_avg,
            margin_vs_max,
            num_negatives: sampled.len(),
            reason,
            negative_ids,
        }
    }

    /// Validate using hard negative mining for strategic sample selection
    ///
    /// This method uses the hard negative miner to select challenging negative samples
    /// from the same therapeutic class or with similar spelling. If hard negatives
    /// are not available, it falls back to random sampling with a warning.
    ///
    /// # Arguments
    /// * `offer` - The offer being matched
    /// * `matched_request` - The request that was matched
    /// * `positive_score` - The score of the positive match
    /// * `negative_pool` - Pool of requests to sample negatives from
    ///
    /// # Returns
    /// A `ContrastiveResult` indicating whether the match passed validation
    pub fn validate_with_hard_negatives(
        &self,
        offer: &OfferModel,
        matched_request: &RequestModel,
        positive_score: f64,
        negative_pool: &[RequestModel],
    ) -> ContrastiveResult {
        self.stats.total_validations.fetch_add(1, Ordering::Relaxed);

        // Check if validation is enabled
        if !self.config.enabled {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("Contrastive validation disabled");
        }

        // Skip if score is below threshold
        if positive_score < self.config.min_score_threshold {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped(&format!(
                "Score {:.2} below threshold {:.2}",
                positive_score, self.config.min_score_threshold
            ));
        }

        // Check if we should use hard negatives and if they're available
        let use_hard_negatives = self.config.use_hard_negatives && self.has_hard_negatives();

        // Get hard negative medication names if available
        let hard_negative_names: Vec<String> = if use_hard_negatives {
            self.hard_negative_miner
                .as_ref()
                .map(|miner| miner.get_hard_negatives(&offer.medication, self.config.num_negatives))
                .unwrap_or_default()
        } else {
            vec![]
        };

        // If no hard negatives available, fall back to random sampling
        if hard_negative_names.is_empty() {
            if self.config.use_hard_negatives {
                tracing::warn!(
                    medication = %offer.medication,
                    "No hard negatives available, falling back to random sampling"
                );
            }
            return self.validate(offer, matched_request, positive_score, negative_pool);
        }

        // Find requests in the pool that match the hard negative medication names
        let hard_negatives: Vec<&RequestModel> = negative_pool
            .iter()
            .filter(|r| {
                r.id != matched_request.id
                    && hard_negative_names
                        .iter()
                        .any(|name| r.medication.to_lowercase().contains(name))
            })
            .take(self.config.num_negatives)
            .collect();

        // If we couldn't find enough hard negatives in the pool, supplement with random
        let negatives: Vec<&RequestModel> = if hard_negatives.len() < self.config.num_negatives {
            let remaining = self.config.num_negatives - hard_negatives.len();
            let hard_negative_ids: Vec<Uuid> = hard_negatives.iter().map(|r| r.id).collect();

            // Get random negatives excluding already selected hard negatives
            let random_negatives: Vec<&RequestModel> = negative_pool
                .iter()
                .filter(|r| r.id != matched_request.id && !hard_negative_ids.contains(&r.id))
                .collect();

            let mut rng = rand::rng();
            let additional: Vec<&RequestModel> = random_negatives
                .choose_multiple(&mut rng, remaining)
                .cloned()
                .collect();

            let mut combined = hard_negatives;
            combined.extend(additional);
            combined
        } else {
            hard_negatives
        };

        if negatives.is_empty() {
            self.stats.skipped.fetch_add(1, Ordering::Relaxed);
            return ContrastiveResult::skipped("No valid negatives after hard negative selection");
        }

        // Calculate scores against negatives
        let negative_scores: Vec<f64> = negatives
            .iter()
            .map(|neg| self.calculate_score(offer, neg))
            .collect();

        let negative_ids: Vec<Uuid> = negatives.iter().map(|n| n.id).collect();

        let avg_negative = negative_scores.iter().sum::<f64>() / negative_scores.len() as f64;
        let max_negative = negative_scores
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let margin_vs_avg = positive_score - avg_negative;
        let margin_vs_max = positive_score - max_negative;

        // Check if margins meet thresholds
        let valid = margin_vs_avg >= self.config.min_margin
            && margin_vs_max >= self.config.min_margin_vs_max;

        let reason = if valid {
            format!(
                "Valid (hard negatives): margin vs avg {:.2} >= {:.2}, margin vs max {:.2} >= {:.2}",
                margin_vs_avg, self.config.min_margin, margin_vs_max, self.config.min_margin_vs_max
            )
        } else if margin_vs_avg < self.config.min_margin {
            format!(
                "Failed (hard negatives): margin vs avg {:.2} < {:.2} (positive {:.2}, avg negative {:.2})",
                margin_vs_avg, self.config.min_margin, positive_score, avg_negative
            )
        } else {
            format!(
                "Failed (hard negatives): margin vs max {:.2} < {:.2} (positive {:.2}, max negative {:.2})",
                margin_vs_max, self.config.min_margin_vs_max, positive_score, max_negative
            )
        };

        if valid {
            self.stats.passed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.failed.fetch_add(1, Ordering::Relaxed);
            self.stats
                .false_positives_detected
                .fetch_add(1, Ordering::Relaxed);
        }

        ContrastiveResult {
            valid,
            positive_score,
            avg_negative_score: avg_negative,
            max_negative_score: max_negative,
            margin_vs_avg,
            margin_vs_max,
            num_negatives: negatives.len(),
            reason,
            negative_ids,
        }
    }

    /// Sample random negatives from the pool
    fn sample_negatives<'a>(
        &self,
        pool: &'a [RequestModel],
        exclude_id: Uuid,
    ) -> Vec<&'a RequestModel> {
        let filtered: Vec<&RequestModel> = pool.iter().filter(|r| r.id != exclude_id).collect();

        if filtered.len() <= self.config.num_negatives {
            return filtered;
        }

        let mut rng = rand::rng();
        filtered
            .choose_multiple(&mut rng, self.config.num_negatives)
            .cloned()
            .collect()
    }

    /// Calculate similarity score between offer and request
    fn calculate_score(&self, offer: &OfferModel, request: &RequestModel) -> f64 {
        medication_similarity_with_raw(
            &offer.medication,
            &request.medication,
            Some(&offer.medication),
            Some(&request.medication),
        )
    }

    /// Get current statistics
    pub fn stats(&self) -> ContrastiveStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// Get current configuration
    pub fn config(&self) -> &ContrastiveConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ContrastiveConfig) {
        self.config = config;
    }

    /// Enable or disable validation
    pub fn enable(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if validation is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ContrastiveConfig::default();
        assert_eq!(config.num_negatives, 3);
        assert!((config.min_margin - 0.15).abs() < 0.001);
        assert!((config.min_margin_vs_max - 0.10).abs() < 0.001);
        assert!(config.enabled);
    }

    #[test]
    fn test_strict_config() {
        let config = ContrastiveConfig::strict();
        assert_eq!(config.num_negatives, 5);
        assert!((config.min_margin - 0.20).abs() < 0.001);
    }

    #[test]
    fn test_relaxed_config() {
        let config = ContrastiveConfig::relaxed();
        assert_eq!(config.num_negatives, 2);
        assert!((config.min_margin - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_stats_snapshot() {
        let stats = ContrastiveStats::default();
        stats.total_validations.store(100, Ordering::Relaxed);
        stats.passed.store(80, Ordering::Relaxed);
        stats.failed.store(20, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_validations, 100);
        assert_eq!(snapshot.passed, 80);
        assert_eq!(snapshot.failed, 20);
    }

    #[test]
    fn test_detection_rate() {
        let snapshot = ContrastiveStatsSnapshot {
            total_validations: 100,
            passed: 80,
            failed: 20,
            skipped: 0,
            false_positives_detected: 20,
        };

        assert!((snapshot.detection_rate() - 0.20).abs() < 0.001);
    }

    #[test]
    fn test_detection_rate_zero() {
        let snapshot = ContrastiveStatsSnapshot {
            total_validations: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            false_positives_detected: 0,
        };

        assert!((snapshot.detection_rate() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_validator_disabled() {
        let config = ContrastiveConfig {
            enabled: false,
            ..Default::default()
        };
        let validator = ContrastiveValidator::new(config);

        assert!(!validator.is_enabled());
    }

    #[test]
    fn test_result_skipped() {
        let result = ContrastiveResult::skipped("Test reason");
        assert!(result.valid);
        assert_eq!(result.num_negatives, 0);
        assert_eq!(result.reason, "Test reason");
    }

    #[test]
    fn test_stats_reset() {
        let stats = ContrastiveStats::default();
        stats.total_validations.store(100, Ordering::Relaxed);
        stats.passed.store(80, Ordering::Relaxed);

        stats.reset();

        assert_eq!(stats.total_validations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.passed.load(Ordering::Relaxed), 0);
    }

    // =========================================================================
    // Hard Negative Integration Tests
    // =========================================================================

    #[test]
    fn test_config_use_hard_negatives_default() {
        let config = ContrastiveConfig::default();
        assert!(config.use_hard_negatives);
    }

    #[test]
    fn test_validator_without_hard_negative_miner() {
        let validator = ContrastiveValidator::default();
        assert!(!validator.has_hard_negatives());
        assert!(validator.hard_negative_miner().is_none());
    }

    #[test]
    fn test_validator_with_hard_negative_miner() {
        use crate::matching::hard_negative::{
            HardNegativeConfig, HardNegativeMiner, MedicationInfo,
        };

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Glipizide").with_class("Antidiabetic"),
        ];
        miner.build_index(&medications).unwrap();

        let validator =
            ContrastiveValidator::with_hard_negative_miner(ContrastiveConfig::default(), miner);

        assert!(validator.has_hard_negatives());
        assert!(validator.hard_negative_miner().is_some());
    }

    #[test]
    fn test_validator_set_hard_negative_miner() {
        use crate::matching::hard_negative::{
            HardNegativeConfig, HardNegativeMiner, MedicationInfo,
        };

        let mut validator = ContrastiveValidator::default();
        assert!(!validator.has_hard_negatives());

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        let medications = vec![MedicationInfo::new("Metformin").with_class("Antidiabetic")];
        miner.build_index(&medications).unwrap();

        validator.set_hard_negative_miner(miner);
        assert!(validator.has_hard_negatives());
    }

    #[test]
    fn test_has_hard_negatives_not_built() {
        use crate::matching::hard_negative::{HardNegativeConfig, HardNegativeMiner};

        // Miner exists but index not built
        let miner = HardNegativeMiner::new(HardNegativeConfig::default());
        let validator =
            ContrastiveValidator::with_hard_negative_miner(ContrastiveConfig::default(), miner);

        assert!(!validator.has_hard_negatives());
    }
}

