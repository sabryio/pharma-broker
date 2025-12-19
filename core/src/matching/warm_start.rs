//! Warm start and outlier detection module
//!
//! Ported from legacy/matching/warm_start.go

use chrono::{DateTime, Duration, Utc};
use std::sync::RwLock;

use super::Weights;

// =============================================================================
// Warm Start Configuration
// =============================================================================

/// Warm start configuration for cold start scenarios
/// Ported from Go: WarmStartConfig (warm_start.go:19-34)
#[derive(Debug, Clone)]
pub struct WarmStartConfig {
    /// Prior weights to use when insufficient data
    pub prior_weights: Weights,
    /// Equivalent sample count for prior (higher = stronger prior influence)
    pub prior_strength: usize,
    /// Days until prior influence halves (decay)
    pub decay_half_life: i64,
    /// Minimum samples before learning kicks in
    pub min_samples_for_learning: usize,
    /// Enable warm start
    pub enabled: bool,
}

impl Default for WarmStartConfig {
    /// Sensible defaults
    /// Ported from Go: DefaultWarmStartConfig (warm_start.go:36-51)
    fn default() -> Self {
        Self {
            prior_weights: Weights {
                medication: 0.35, // Medication match is most important
                dosage: 0.25,
                quantity: 0.15,
                price: 0.15,
                recency: 0.10,
            },
            prior_strength: 50,           // Equivalent to 50 samples
            decay_half_life: 14,          // Prior halves every 2 weeks
            min_samples_for_learning: 20, // Start blending at 20 samples
            enabled: true,
        }
    }
}

// =============================================================================
// Warm Start Manager
// =============================================================================

/// Warm start manager for cold start scenarios with prior knowledge
/// Ported from Go: WarmStartManager (warm_start.go:57-64)
pub struct WarmStartManager {
    config: RwLock<WarmStartConfig>,
    start_time: RwLock<DateTime<Utc>>,
}

impl Default for WarmStartManager {
    fn default() -> Self {
        Self::new(WarmStartConfig::default())
    }
}

impl WarmStartManager {
    /// Create a new warm start manager
    pub fn new(config: WarmStartConfig) -> Self {
        Self {
            config: RwLock::new(config),
            start_time: RwLock::new(Utc::now()),
        }
    }

    /// Get effective weights blended with priors based on sample count
    /// Ported from Go: WarmStartManager.GetEffectiveWeights (warm_start.go:75-119)
    pub fn get_effective_weights(&self, learned_weights: &Weights, sample_count: usize) -> Weights {
        let config = self.config.read().unwrap();

        if !config.enabled {
            return learned_weights.clone();
        }

        // Calculate effective prior strength (decays over time)
        let start_time = *self.start_time.read().unwrap();
        let days_since_start = (Utc::now() - start_time).num_hours() as f64 / 24.0;
        let decay_factor = 0.5_f64.powf(days_since_start / config.decay_half_life as f64);
        let effective_prior_strength = config.prior_strength as f64 * decay_factor;

        // If not enough samples, use pure prior
        if sample_count < config.min_samples_for_learning {
            tracing::debug!(
                samples = sample_count,
                min_required = config.min_samples_for_learning,
                "Using prior weights (insufficient samples)"
            );
            return config.prior_weights.clone();
        }

        // Calculate blend ratio
        let total_weight = sample_count as f64 + effective_prior_strength;
        let prior_weight = effective_prior_strength / total_weight;
        let data_weight = 1.0 - prior_weight;

        // Blend weights
        let blended = Weights {
            medication: data_weight * learned_weights.medication
                + prior_weight * config.prior_weights.medication,
            dosage: data_weight * learned_weights.dosage
                + prior_weight * config.prior_weights.dosage,
            quantity: data_weight * learned_weights.quantity
                + prior_weight * config.prior_weights.quantity,
            price: data_weight * learned_weights.price + prior_weight * config.prior_weights.price,
            recency: data_weight * learned_weights.recency
                + prior_weight * config.prior_weights.recency,
        };

        tracing::debug!(
            samples = sample_count,
            prior_weight = format!("{:.2}", prior_weight),
            data_weight = format!("{:.2}", data_weight),
            "Blended weights with prior"
        );

        blended
    }

    /// Get current prior influence percentage
    /// Ported from Go: WarmStartManager.GetPriorInfluence (warm_start.go:121-136)
    pub fn get_prior_influence(&self, sample_count: usize) -> f64 {
        let config = self.config.read().unwrap();

        if !config.enabled || sample_count < config.min_samples_for_learning {
            return 100.0;
        }

        let start_time = *self.start_time.read().unwrap();
        let days_since_start = (Utc::now() - start_time).num_hours() as f64 / 24.0;
        let decay_factor = 0.5_f64.powf(days_since_start / config.decay_half_life as f64);
        let effective_prior_strength = config.prior_strength as f64 * decay_factor;

        let total_weight = sample_count as f64 + effective_prior_strength;
        (effective_prior_strength / total_weight) * 100.0
    }

    /// Set configuration
    pub fn set_config(&self, config: WarmStartConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get configuration
    pub fn get_config(&self) -> WarmStartConfig {
        self.config.read().unwrap().clone()
    }

    /// Enable or disable warm start
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Reset start time
    pub fn reset(&self) {
        *self.start_time.write().unwrap() = Utc::now();
        tracing::info!("Warm start timer reset");
    }
}

// =============================================================================
// Outlier Detection
// =============================================================================

/// Outlier detector configuration
/// Ported from Go: OutlierDetectorConfig (warm_start.go:171-184)
#[derive(Debug, Clone)]
pub struct OutlierDetectorConfig {
    /// Window size for calculating statistics
    pub window_size: usize,
    /// Z-score threshold for outlier detection (default: 2.5)
    pub z_score_threshold: f64,
    /// Minimum samples before detection activates
    pub min_samples: usize,
    /// Enable outlier detection
    pub enabled: bool,
}

impl Default for OutlierDetectorConfig {
    fn default() -> Self {
        Self {
            window_size: 100,
            z_score_threshold: 2.5,
            min_samples: 20,
            enabled: true,
        }
    }
}

/// Outlier detector for filtering anomalous feedback
/// Ported from Go: OutlierDetector (warm_start.go:196-203)
pub struct OutlierDetector {
    config: RwLock<OutlierDetectorConfig>,
    recent_scores: RwLock<Vec<f64>>,
    idx: RwLock<usize>,
}

impl Default for OutlierDetector {
    fn default() -> Self {
        Self::new(OutlierDetectorConfig::default())
    }
}

impl OutlierDetector {
    /// Create a new outlier detector
    pub fn new(config: OutlierDetectorConfig) -> Self {
        let window_size = config.window_size;
        Self {
            config: RwLock::new(config),
            recent_scores: RwLock::new(Vec::with_capacity(window_size)),
            idx: RwLock::new(0),
        }
    }

    /// Add a score to the window
    /// Ported from Go: OutlierDetector.AddScore (warm_start.go:214-225)
    pub fn add_score(&self, score: f64) {
        let config = self.config.read().unwrap();
        let window_size = config.window_size;
        drop(config);

        let mut scores = self.recent_scores.write().unwrap();
        let mut idx = self.idx.write().unwrap();

        if scores.len() < window_size {
            scores.push(score);
        } else {
            scores[*idx] = score;
            *idx = (*idx + 1) % window_size;
        }
    }

    /// Check if a score is an outlier
    /// Ported from Go: OutlierDetector.IsOutlier (warm_start.go:227-258)
    pub fn is_outlier(&self, score: f64) -> bool {
        let config = self.config.read().unwrap();

        if !config.enabled {
            return false;
        }

        let scores = self.recent_scores.read().unwrap();

        if scores.len() < config.min_samples {
            return false; // Not enough data
        }

        let (mean, std_dev) = Self::calculate_stats_inner(&scores);

        if std_dev == 0.0 {
            return false;
        }

        let z_score = (score - mean).abs() / std_dev;
        let is_outlier = z_score > config.z_score_threshold;

        if is_outlier {
            tracing::debug!(
                score = score,
                mean = mean,
                std_dev = std_dev,
                z_score = z_score,
                "🚨 Outlier detected"
            );
        }

        is_outlier
    }

    /// Calculate mean and standard deviation
    fn calculate_stats_inner(scores: &[f64]) -> (f64, f64) {
        if scores.is_empty() {
            return (0.0, 0.0);
        }

        // Calculate mean
        let sum: f64 = scores.iter().sum();
        let mean = sum / scores.len() as f64;

        // Calculate standard deviation
        let sum_sq: f64 = scores.iter().map(|s| (s - mean).powi(2)).sum();
        let variance = sum_sq / scores.len() as f64;
        let std_dev = variance.sqrt();

        (mean, std_dev)
    }

    /// Get current statistics
    pub fn get_stats(&self) -> (f64, f64, usize) {
        let scores = self.recent_scores.read().unwrap();
        let (mean, std_dev) = Self::calculate_stats_inner(&scores);
        (mean, std_dev, scores.len())
    }

    /// Set configuration
    pub fn set_config(&self, config: OutlierDetectorConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Get configuration
    pub fn get_config(&self) -> OutlierDetectorConfig {
        self.config.read().unwrap().clone()
    }

    /// Enable or disable outlier detection
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Reset the score window
    pub fn reset(&self) {
        let config = self.config.read().unwrap();
        let window_size = config.window_size;
        drop(config);

        *self.recent_scores.write().unwrap() = Vec::with_capacity(window_size);
        *self.idx.write().unwrap() = 0;
        tracing::info!("Outlier detector reset");
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_warm_start_config() {
        let config = WarmStartConfig::default();

        assert!((config.prior_weights.medication - 0.35).abs() < 0.001);
        assert_eq!(config.prior_strength, 50);
        assert_eq!(config.decay_half_life, 14);
        assert_eq!(config.min_samples_for_learning, 20);
        assert!(config.enabled);
    }

    #[test]
    fn test_warm_start_insufficient_samples() {
        let manager = WarmStartManager::default();

        let learned = Weights {
            medication: 0.50,
            dosage: 0.20,
            quantity: 0.10,
            price: 0.10,
            recency: 0.10,
        };

        // With only 10 samples (< 20 min), should use pure prior
        let effective = manager.get_effective_weights(&learned, 10);

        // Should return prior weights
        assert!((effective.medication - 0.35).abs() < 0.001);
    }

    #[test]
    fn test_warm_start_blending() {
        let manager = WarmStartManager::default();

        let learned = Weights {
            medication: 0.50,
            dosage: 0.20,
            quantity: 0.10,
            price: 0.10,
            recency: 0.10,
        };

        // With 100 samples, should blend with prior
        let effective = manager.get_effective_weights(&learned, 100);

        // Should be between prior (0.35) and learned (0.50)
        assert!(effective.medication > 0.35);
        assert!(effective.medication < 0.50);
    }

    #[test]
    fn test_warm_start_disabled() {
        let mut config = WarmStartConfig::default();
        config.enabled = false;
        let manager = WarmStartManager::new(config);

        let learned = Weights {
            medication: 0.50,
            dosage: 0.20,
            quantity: 0.10,
            price: 0.10,
            recency: 0.10,
        };

        // When disabled, should return learned weights
        let effective = manager.get_effective_weights(&learned, 10);
        assert!((effective.medication - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_prior_influence() {
        let manager = WarmStartManager::default();

        // With insufficient samples, influence is 100%
        assert!((manager.get_prior_influence(10) - 100.0).abs() < 0.001);

        // With more samples, influence decreases
        let influence_100 = manager.get_prior_influence(100);
        assert!(influence_100 < 100.0);
        assert!(influence_100 > 0.0);
    }

    #[test]
    fn test_outlier_detector_add_scores() {
        let detector = OutlierDetector::default();

        for i in 0..50 {
            detector.add_score(0.5 + (i as f64 * 0.01));
        }

        let (mean, std_dev, count) = detector.get_stats();
        assert_eq!(count, 50);
        assert!(mean > 0.5);
        assert!(std_dev > 0.0);
    }

    #[test]
    fn test_outlier_detection() {
        let config = OutlierDetectorConfig {
            window_size: 100,
            z_score_threshold: 2.5,
            min_samples: 10,
            enabled: true,
        };
        let detector = OutlierDetector::new(config);

        // Add normal scores around 0.7
        for _ in 0..50 {
            detector.add_score(0.7);
        }

        // 0.7 is not an outlier
        assert!(!detector.is_outlier(0.7));

        // Very different score is outlier (if std_dev > 0)
        // Since all scores are 0.7, std_dev is 0, so nothing is outlier
        // Let's add some variance
    }

    #[test]
    fn test_outlier_detection_with_variance() {
        let config = OutlierDetectorConfig {
            window_size: 100,
            z_score_threshold: 2.0,
            min_samples: 10,
            enabled: true,
        };
        let detector = OutlierDetector::new(config);

        // Add normal scores between 0.6 and 0.8
        for i in 0..50 {
            detector.add_score(0.6 + (i as f64 % 20.0) * 0.01);
        }

        // Score far outside normal range should be outlier
        assert!(detector.is_outlier(0.1));
        assert!(detector.is_outlier(1.5));
    }

    #[test]
    fn test_outlier_detector_disabled() {
        let mut config = OutlierDetectorConfig::default();
        config.enabled = false;
        let detector = OutlierDetector::new(config);

        for _ in 0..30 {
            detector.add_score(0.7);
        }

        // When disabled, nothing is outlier
        assert!(!detector.is_outlier(0.1));
    }

    #[test]
    fn test_outlier_detector_insufficient_samples() {
        let detector = OutlierDetector::default();

        // Add only 5 samples (< 20 min)
        for _ in 0..5 {
            detector.add_score(0.7);
        }

        // Not enough samples, so no outlier detection
        assert!(!detector.is_outlier(0.1));
    }
}
