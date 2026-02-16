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
            pharmaceutical: current.pharmaceutical
                * (1.0 + lr * correlations.get("pharmaceutical").unwrap_or(&0.0)),
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
            pharmaceutical: constrain(current.pharmaceutical, adjusted.pharmaceutical),
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
            + weights.pharmaceutical
            + weights.recency
            + weights.expiry
            + weights.supplier
            + weights.ai_logic;

        if sum == 0.0 {
            // Fallback to equal weights (6 weights now)
            return Weights {
                medication: 1.0 / 6.0,
                pharmaceutical: 1.0 / 6.0,
                recency: 1.0 / 6.0,
                expiry: 1.0 / 6.0,
                supplier: 1.0 / 6.0,
                ai_logic: 1.0 / 6.0,
            };
        }

        Weights {
            medication: weights.medication / sum,
            pharmaceutical: weights.pharmaceutical / sum,
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
