//! Adaptive weight learning module
//!
//! Ported from legacy/matching/learner.go

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::FeedbackStats;

use super::{Scorer, Weights};

/// Learning algorithm configuration
/// Ported from Go: LearningConfig struct (learner.go:13-20)
#[derive(Debug, Clone)]
pub struct LearningConfig {
    /// Alpha - how quickly weights adjust (default: 0.1)
    pub learning_rate: f64,
    /// Minimum allowed weight (default: 0.05)
    pub min_weight: f64,
    /// Maximum allowed weight (default: 0.70)
    pub max_weight: f64,
    /// Ignore changes smaller than this (default: 0.02)
    pub min_change: f64,
    /// Minimum feedback samples required (default: 100)
    pub min_samples: usize,
    /// Days of feedback to analyze (default: 30)
    pub analysis_window: i64,
}

impl Default for LearningConfig {
    /// Conservative default configuration
    /// Ported from Go: DefaultLearningConfig (learner.go:22-32)
    fn default() -> Self {
        Self {
            learning_rate: 0.1,  // Conservative: slow but stable learning
            min_weight: 0.05,    // Keep all factors relevant
            max_weight: 0.70,    // Prevent single factor dominance
            min_change: 0.02,    // Ignore noise
            min_samples: 100,    // Require sufficient data
            analysis_window: 30, // Last 30 days
        }
    }
}

/// Performance metrics for weight evaluation
/// Ported from Go: entity.PerformanceMetrics
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub confirmation_rate: f64,
    pub avg_score_confirmed: f64,
    pub avg_score_rejected: f64,
    pub sample_size: i64,
    pub evaluated_at: DateTime<Utc>,
}

/// Weight source indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightSource {
    Default,
    Manual,
    AutoLearned,
}

/// Weight history entry
#[derive(Debug, Clone)]
pub struct WeightHistory {
    pub id: String,
    pub medication_weight: f64,
    pub dosage_weight: f64,
    pub quantity_weight: f64,
    pub price_weight: f64,
    pub recency_weight: f64,
    pub ai_logic_weight: f64,
    pub source: WeightSource,
    pub performance_metrics: Option<PerformanceMetrics>,
    pub notes: Option<String>,
    pub applied_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Feedback record repository interface
/// Ported from Go: FeedbackRecordRepository (learner.go:42-46)
#[async_trait]
pub trait FeedbackRecordRepository: Send + Sync {
    async fn get_feedback_stats(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<FeedbackStats, LearnerError>;
}

/// Weight history repository interface
/// Ported from Go: WeightHistoryRepository (learner.go:48-58)
#[async_trait]
pub trait WeightHistoryRepository: Send + Sync {
    async fn save(&self, history: &WeightHistory) -> Result<(), LearnerError>;
    async fn get_current(&self) -> Result<Option<WeightHistory>, LearnerError>;
    async fn get_history(&self, limit: usize) -> Result<Vec<WeightHistory>, LearnerError>;
    async fn save_with_metrics(
        &self,
        weights: &Weights,
        source: WeightSource,
        metrics: Option<&PerformanceMetrics>,
        notes: Option<&str>,
    ) -> Result<(), LearnerError>;
}

/// Learner errors
#[derive(Debug, Error)]
pub enum LearnerError {
    #[error("Insufficient feedback data for learning (got {got}, need {need})")]
    InsufficientData { got: usize, need: usize },

    #[error("No previous weights available for rollback")]
    NoRollbackAvailable,

    #[error("Repository error: {0}")]
    Repository(String),
}

/// Adaptive weight learner based on operator feedback
/// Ported from Go: WeightLearner struct (learner.go:34-40)
pub struct WeightLearner {
    config: RwLock<LearningConfig>,
    scorer: Option<Scorer>,
}

impl Default for WeightLearner {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightLearner {
    /// Create a new weight learner with default config
    pub fn new() -> Self {
        Self {
            config: RwLock::new(LearningConfig::default()),
            scorer: Some(Scorer::default()),
        }
    }

    /// Create weight learner with custom config
    pub fn with_config(config: LearningConfig) -> Self {
        Self {
            config: RwLock::new(config),
            scorer: Some(Scorer::default()),
        }
    }

    /// Create weight learner with scorer reference
    pub fn with_scorer(scorer: Scorer) -> Self {
        Self {
            config: RwLock::new(LearningConfig::default()),
            scorer: Some(scorer),
        }
    }

    /// Calculate correlations for each factor
    /// Ported from Go: WeightLearner.calculateCorrelations (learner.go:123-156)
    pub fn calculate_correlations(&self, stats: &FeedbackStats) -> HashMap<String, f64> {
        let mut correlations = HashMap::new();

        // Formula: diff / max(confirmed_avg, rejected_avg)
        let calc_corr = |diff: f64, confirmed: f64, rejected: f64| -> f64 {
            let max_val = confirmed.max(rejected);
            if max_val > 0.0 { diff / max_val } else { 0.0 }
        };

        correlations.insert(
            "medication".to_string(),
            calc_corr(
                stats.medication_diff,
                stats.confirmed_avg_medication,
                stats.rejected_avg_medication,
            ),
        );

        correlations.insert(
            "dosage".to_string(),
            calc_corr(
                stats.dosage_diff,
                stats.confirmed_avg_dosage,
                stats.rejected_avg_dosage,
            ),
        );

        correlations.insert(
            "quantity".to_string(),
            calc_corr(
                stats.quantity_diff,
                stats.confirmed_avg_quantity,
                stats.rejected_avg_quantity,
            ),
        );

        correlations.insert(
            "price".to_string(),
            calc_corr(
                stats.price_diff,
                stats.confirmed_avg_price,
                stats.rejected_avg_price,
            ),
        );

        correlations.insert(
            "recency".to_string(),
            calc_corr(
                stats.recency_diff,
                stats.confirmed_avg_recency,
                stats.rejected_avg_recency,
            ),
        );
        correlations.insert(
            "ai_logic".to_string(),
            calc_corr(
                stats.ai_logic_diff,
                stats.confirmed_avg_ai_logic,
                stats.rejected_avg_ai_logic,
            ),
        );

        correlations
    }

    /// Adjust weights based on correlations
    /// Ported from Go: WeightLearner.adjustWeights (learner.go:158-170)
    pub fn adjust_weights(
        &self,
        current: &Weights,
        correlations: &HashMap<String, f64>,
    ) -> Weights {
        let config = self.config.read().unwrap();
        let lr = config.learning_rate;

        // Formula: new_weight = current_weight * (1 + alpha * correlation)
        Weights {
            medication: current.medication
                * (1.0 + lr * correlations.get("medication").unwrap_or(&0.0)),
            dosage: current.dosage * (1.0 + lr * correlations.get("dosage").unwrap_or(&0.0)),
            quantity: current.quantity * (1.0 + lr * correlations.get("quantity").unwrap_or(&0.0)),
            price: current.price * (1.0 + lr * correlations.get("price").unwrap_or(&0.0)),
            recency: current.recency * (1.0 + lr * correlations.get("recency").unwrap_or(&0.0)),
            expiry: current.expiry * (1.0 + lr * correlations.get("expiry").unwrap_or(&0.0)),
            supplier: current.supplier * (1.0 + lr * correlations.get("supplier").unwrap_or(&0.0)),
            ai_logic: current.ai_logic * (1.0 + lr * correlations.get("ai_logic").unwrap_or(&0.0)),
        }
    }

    /// Apply safety constraints on weights
    /// Ported from Go: WeightLearner.applyConstraints (learner.go:172-201)
    pub fn apply_constraints(&self, current: &Weights, adjusted: &Weights) -> Weights {
        let config = self.config.read().unwrap();

        let constrain = |current_val: f64, adjusted_val: f64| -> f64 {
            // Ignore small changes
            if (adjusted_val - current_val).abs() < config.min_change {
                return current_val;
            }

            // Clamp to bounds
            adjusted_val.clamp(config.min_weight, config.max_weight)
        };

        Weights {
            medication: constrain(current.medication, adjusted.medication),
            dosage: constrain(current.dosage, adjusted.dosage),
            quantity: constrain(current.quantity, adjusted.quantity),
            price: constrain(current.price, adjusted.price),
            recency: constrain(current.recency, adjusted.recency),
            expiry: constrain(current.expiry, adjusted.expiry),
            supplier: constrain(current.supplier, adjusted.supplier),
            ai_logic: constrain(current.ai_logic, adjusted.ai_logic),
        }
    }

    /// Normalize weights to sum to 1.0
    /// Ported from Go: WeightLearner.normalizeWeights (learner.go:203-225)
    pub fn normalize_weights(&self, weights: &Weights) -> Weights {
        let sum = weights.medication
            + weights.dosage
            + weights.quantity
            + weights.price
            + weights.recency
            + weights.expiry
            + weights.supplier
            + weights.ai_logic;

        if sum == 0.0 {
            // Fallback to equal weights (8 weights now)
            return Weights {
                medication: 1.0 / 8.0,
                dosage: 1.0 / 8.0,
                quantity: 1.0 / 8.0,
                price: 1.0 / 8.0,
                recency: 1.0 / 8.0,
                expiry: 1.0 / 8.0,
                supplier: 1.0 / 8.0,
                ai_logic: 1.0 / 8.0,
            };
        }

        Weights {
            medication: weights.medication / sum,
            dosage: weights.dosage / sum,
            quantity: weights.quantity / sum,
            price: weights.price / sum,
            recency: weights.recency / sum,
            expiry: weights.expiry / sum,
            supplier: weights.supplier / sum,
            ai_logic: weights.ai_logic / sum,
        }
    }

    /// Calculate performance metrics from feedback stats
    /// Ported from Go: WeightLearner.calculateMetrics (learner.go:227-249)
    pub fn calculate_metrics(&self, stats: &FeedbackStats) -> PerformanceMetrics {
        let precision = stats.confirmation_rate;
        let recall = stats.confirmation_rate; // Simplified

        let f1_score = if precision + recall > 0.0 {
            2.0 * (precision * recall) / (precision + recall)
        } else {
            0.0
        };

        PerformanceMetrics {
            precision,
            recall,
            f1_score,
            confirmation_rate: stats.confirmation_rate,
            avg_score_confirmed: stats.confirmed_avg_total,
            avg_score_rejected: stats.rejected_avg_total,
            sample_size: stats.total_feedback,
            evaluated_at: Utc::now(),
        }
    }

    /// Calculate optimal weights from feedback
    /// Ported from Go: WeightLearner.CalculateOptimalWeights (learner.go:89-121)
    pub fn calculate_optimal_weights(
        &self,
        stats: &FeedbackStats,
        current_weights: &Weights,
    ) -> Result<(Weights, PerformanceMetrics), LearnerError> {
        let config = self.config.read().unwrap();

        // Check minimum sample size
        if (stats.total_feedback as usize) < config.min_samples {
            return Err(LearnerError::InsufficientData {
                got: stats.total_feedback as usize,
                need: config.min_samples,
            });
        }
        drop(config);

        // Calculate correlations
        let correlations = self.calculate_correlations(stats);

        // Adjust weights based on correlations
        let adjusted = self.adjust_weights(current_weights, &correlations);

        // Apply safety constraints
        let constrained = self.apply_constraints(current_weights, &adjusted);

        // Normalize to sum = 1.0
        let normalized = self.normalize_weights(&constrained);

        // Calculate performance metrics
        let metrics = self.calculate_metrics(stats);

        Ok((normalized, metrics))
    }

    /// Determine if new weights should be auto-applied
    /// Ported from Go: WeightLearner.ShouldApply (learner.go:274-288)
    pub fn should_apply(
        &self,
        old_metrics: &PerformanceMetrics,
        new_metrics: &PerformanceMetrics,
    ) -> bool {
        // Calculate separation (how well we distinguish confirmed from rejected)
        let old_separation = old_metrics.avg_score_confirmed - old_metrics.avg_score_rejected;
        let new_separation = new_metrics.avg_score_confirmed - new_metrics.avg_score_rejected;

        // Apply if:
        // 1. Separation improved (better discrimination)
        // 2. Confirmation rate didn't drop by more than 5%
        let separation_improved = new_separation > old_separation;
        let confirmation_rate_ok =
            new_metrics.confirmation_rate >= (old_metrics.confirmation_rate - 0.05);

        separation_improved && confirmation_rate_ok
    }

    /// Set learning configuration
    pub fn set_config(&self, config: LearningConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> LearningConfig {
        self.config.read().unwrap().clone()
    }

    /// Get scorer reference
    pub fn scorer(&self) -> Option<&Scorer> {
        self.scorer.as_ref()
    }
}

// ============================================================================
// Tests - Ported from learner_test.go
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_default_learning_config() {
        let config = LearningConfig::default();

        assert!((config.learning_rate - 0.1).abs() < 0.001);
        assert!((config.min_weight - 0.05).abs() < 0.001);
        assert!((config.max_weight - 0.70).abs() < 0.001);
        assert!((config.min_change - 0.02).abs() < 0.001);
        assert_eq!(config.min_samples, 100);
        assert_eq!(config.analysis_window, 30);
    }

    #[test]
    fn test_calculate_correlations() {
        let learner = WeightLearner::new();

        let stats = FeedbackStats {
            // Medication: strong positive correlation
            confirmed_avg_medication: 0.90,
            rejected_avg_medication: 0.60,
            medication_diff: 0.30,

            // Quantity: weak positive correlation
            confirmed_avg_quantity: 0.85,
            rejected_avg_quantity: 0.80,
            quantity_diff: 0.05,

            // Price: negative correlation
            confirmed_avg_price: 0.70,
            rejected_avg_price: 0.80,
            price_diff: -0.10,

            ..Default::default()
        };

        let correlations = learner.calculate_correlations(&stats);

        // Medication: 0.30 / 0.90 ≈ 0.333
        let expected_med = 0.30 / 0.90;
        assert!(
            (correlations["medication"] - expected_med).abs() < 0.001,
            "medication correlation: {} vs {}",
            correlations["medication"],
            expected_med
        );

        // Quantity: 0.05 / 0.85 ≈ 0.059
        let expected_qty = 0.05 / 0.85;
        assert!(
            (correlations["quantity"] - expected_qty).abs() < 0.001,
            "quantity correlation: {} vs {}",
            correlations["quantity"],
            expected_qty
        );

        // Price: -0.10 / 0.80 = -0.125 (negative!)
        let expected_price = -0.10 / 0.80;
        assert!(
            (correlations["price"] - expected_price).abs() < 0.001,
            "price correlation: {} vs {}",
            correlations["price"],
            expected_price
        );
    }

    #[test]
    fn test_adjust_weights_positive_correlation() {
        let learner = WeightLearner::new();

        let current = Weights {
            medication: 0.45,
            dosage: 0.10,
            quantity: 0.20,
            price: 0.15,
            recency: 0.05,
            expiry: 0.025,
            supplier: 0.025,
            ai_logic: 0.0,
        };

        let mut correlations = HashMap::new();
        correlations.insert("medication".to_string(), 0.30); // Strong positive
        correlations.insert("dosage".to_string(), 0.0);
        correlations.insert("quantity".to_string(), 0.0);
        correlations.insert("price".to_string(), -0.10); // Negative
        correlations.insert("recency".to_string(), 0.0);

        let adjusted = learner.adjust_weights(&current, &correlations);

        // Medication: 0.45 * (1 + 0.1 * 0.30) = 0.45 * 1.03 = 0.4635
        let expected_med = 0.45 * (1.0 + 0.1 * 0.30);
        assert!(
            (adjusted.medication - expected_med).abs() < 0.0001,
            "medication: {} vs {}",
            adjusted.medication,
            expected_med
        );

        // Price: 0.15 * (1 + 0.1 * -0.10) = 0.15 * 0.99 = 0.1485
        let expected_price = 0.15 * (1.0 + 0.1 * -0.10);
        assert!(
            (adjusted.price - expected_price).abs() < 0.0001,
            "price: {} vs {}",
            adjusted.price,
            expected_price
        );
    }

    #[test]
    fn test_apply_constraints_min_max_bounds() {
        let learner = WeightLearner::with_config(LearningConfig {
            min_weight: 0.05,
            max_weight: 0.70,
            min_change: 0.02,
            ..Default::default()
        });

        let current = Weights {
            medication: 0.50,
            dosage: 0.10,
            quantity: 0.20,
            price: 0.10,
            recency: 0.05,
            expiry: 0.025,
            supplier: 0.025,
            ai_logic: 0.0,
        };

        let adjusted = Weights {
            medication: 0.80, // Exceeds max
            dosage: 0.02,     // Below min
            quantity: 0.21,   // Small change (0.01 < 0.02)
            price: 0.15,      // Acceptable change
            recency: 0.05,    // No change
            expiry: 0.025,
            supplier: 0.025,
            ai_logic: 0.05, // Acceptable change
        };

        let constrained = learner.apply_constraints(&current, &adjusted);

        // Medication clamped to max
        assert_eq!(constrained.medication, 0.70);

        // Dosage raised to min
        assert_eq!(constrained.dosage, 0.05);

        // Quantity unchanged (change too small)
        assert_eq!(constrained.quantity, 0.20);

        // Price adjusted
        assert_eq!(constrained.price, 0.15);
    }

    #[test]
    fn test_normalize_weights() {
        let learner = WeightLearner::new();

        let weights = Weights {
            medication: 0.50,
            dosage: 0.10,
            quantity: 0.20,
            price: 0.15,
            recency: 0.05,
            expiry: 0.025,
            supplier: 0.025,
            ai_logic: 0.0,
        };
        // Sum = 1.05

        let normalized = learner.normalize_weights(&weights);

        let sum = normalized.medication
            + normalized.dosage
            + normalized.quantity
            + normalized.price
            + normalized.recency
            + normalized.expiry
            + normalized.supplier
            + normalized.ai_logic;

        assert!((sum - 1.0).abs() < 0.0001, "sum: {}", sum);

        // Proportions maintained
        let expected_med = 0.50 / 1.05;
        assert!(
            (normalized.medication - expected_med).abs() < 0.0001,
            "medication: {} vs {}",
            normalized.medication,
            expected_med
        );
    }

    #[test]
    fn test_normalize_weights_zero_sum() {
        let learner = WeightLearner::new();

        // Explicit zero weights (not default which has non-zero values)
        let weights = Weights {
            medication: 0.0,
            dosage: 0.0,
            quantity: 0.0,
            price: 0.0,
            recency: 0.0,
            expiry: 0.0,
            supplier: 0.0,
            ai_logic: 0.0,
        };

        let normalized = learner.normalize_weights(&weights);

        // Should return equal weights (1/8 for each of 8 weights)
        let expected = 1.0 / 8.0;
        assert!((normalized.medication - expected).abs() < 0.0001);
        assert!((normalized.dosage - expected).abs() < 0.0001);

        let sum = normalized.medication
            + normalized.dosage
            + normalized.quantity
            + normalized.price
            + normalized.recency
            + normalized.expiry
            + normalized.supplier
            + normalized.ai_logic;
        assert!((sum - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_optimal_weights_insufficient_data() {
        let learner = WeightLearner::new();

        let stats = FeedbackStats {
            total_feedback: 50, // Less than min (100)
            ..Default::default()
        };

        let current = Weights::default();
        let result = learner.calculate_optimal_weights(&stats, &current);

        assert!(result.is_err());
        match result.unwrap_err() {
            LearnerError::InsufficientData { got, need } => {
                assert_eq!(got, 50);
                assert_eq!(need, 100);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_calculate_optimal_weights_success() {
        let learner = WeightLearner::new();

        let stats = FeedbackStats {
            total_feedback: 200,
            confirmed_count: 150,
            rejected_count: 50,
            avg_confirmed_score: 0.88,
            avg_rejected_score: 0.75,
            confirmation_rate: 0.75,

            confirmed_avg_medication: 0.90,
            rejected_avg_medication: 0.60,
            medication_diff: 0.30,

            confirmed_avg_dosage: 0.85,
            rejected_avg_dosage: 0.82,
            dosage_diff: 0.03,

            confirmed_avg_quantity: 0.88,
            rejected_avg_quantity: 0.85,
            quantity_diff: 0.03,

            confirmed_avg_price: 0.80,
            rejected_avg_price: 0.78,
            price_diff: 0.02,

            confirmed_avg_recency: 0.92,
            rejected_avg_recency: 0.90,
            recency_diff: 0.02,

            confirmed_avg_total: 0.88,
            rejected_avg_total: 0.75,

            confirmed_avg_ai_logic: 0.85,
            rejected_avg_ai_logic: 0.80,
            ai_logic_diff: 0.05,
        };

        let current = Weights::default();
        let (weights, metrics) = learner
            .calculate_optimal_weights(&stats, &current)
            .expect("should calculate");

        // Sum should be 1.0 (all 8 weight fields)
        let sum = weights.medication
            + weights.dosage
            + weights.quantity
            + weights.price
            + weights.recency
            + weights.expiry
            + weights.supplier
            + weights.ai_logic;
        assert!((sum - 1.0).abs() < 0.0001, "sum: {}", sum);

        // Metrics should reflect stats
        assert_eq!(metrics.confirmation_rate, 0.75);
        assert_eq!(metrics.sample_size, 200);
    }

    #[rstest]
    #[case(
        PerformanceMetrics {
            avg_score_confirmed: 0.85,
            avg_score_rejected: 0.65,
            confirmation_rate: 0.75,
            ..Default::default()
        },
        PerformanceMetrics {
            avg_score_confirmed: 0.88,
            avg_score_rejected: 0.63,
            confirmation_rate: 0.76,
            ..Default::default()
        },
        true // Better separation, stable rate
    )]
    #[case(
        PerformanceMetrics {
            avg_score_confirmed: 0.85,
            avg_score_rejected: 0.65,
            confirmation_rate: 0.75,
            ..Default::default()
        },
        PerformanceMetrics {
            avg_score_confirmed: 0.82,
            avg_score_rejected: 0.68,
            confirmation_rate: 0.75,
            ..Default::default()
        },
        false // Worse separation
    )]
    #[case(
        PerformanceMetrics {
            avg_score_confirmed: 0.85,
            avg_score_rejected: 0.65,
            confirmation_rate: 0.75,
            ..Default::default()
        },
        PerformanceMetrics {
            avg_score_confirmed: 0.90,
            avg_score_rejected: 0.60,
            confirmation_rate: 0.65, // Dropped > 5%
            ..Default::default()
        },
        false // Rate dropped too much
    )]
    fn test_should_apply(
        #[case] old: PerformanceMetrics,
        #[case] new: PerformanceMetrics,
        #[case] expected: bool,
    ) {
        let learner = WeightLearner::new();
        assert_eq!(learner.should_apply(&old, &new), expected);
    }

    #[test]
    fn test_set_get_config() {
        let learner = WeightLearner::new();

        let custom = LearningConfig {
            learning_rate: 0.05,
            min_weight: 0.10,
            max_weight: 0.60,
            min_change: 0.03,
            min_samples: 200,
            analysis_window: 60,
        };

        learner.set_config(custom);
        let got = learner.get_config();

        assert!((got.learning_rate - 0.05).abs() < 0.001);
        assert_eq!(got.min_samples, 200);
    }

    #[test]
    fn test_calculate_metrics() {
        let learner = WeightLearner::new();

        let stats = FeedbackStats {
            total_feedback: 200,
            confirmed_count: 150,
            confirmation_rate: 0.75,
            confirmed_avg_total: 0.88,
            rejected_avg_total: 0.65,
            ..Default::default()
        };

        let metrics = learner.calculate_metrics(&stats);

        assert_eq!(metrics.confirmation_rate, 0.75);
        assert_eq!(metrics.avg_score_confirmed, 0.88);
        assert_eq!(metrics.avg_score_rejected, 0.65);
        assert_eq!(metrics.sample_size, 200);
        assert_eq!(metrics.precision, 0.75);

        // F1 Score
        let expected_f1 = 2.0 * (0.75 * 0.75) / (0.75 + 0.75);
        assert!((metrics.f1_score - expected_f1).abs() < 0.0001);
    }
}
