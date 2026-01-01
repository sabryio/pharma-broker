//! A/B testing module for weight optimization
//!
//! Ported from legacy/matching/abtest.go

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::RwLock;
use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Utc};

use super::Weights;

// =============================================================================
// A/B Test Configuration
// =============================================================================

/// A/B test experiment configuration
/// Ported from Go: ABTestConfig (abtest.go:17-28)
#[derive(Debug, Clone)]
pub struct ABTestConfig {
    /// Unique identifier for the test
    pub test_id: String,
    /// Human-readable name
    pub name: String,
    /// What this test is evaluating
    pub description: String,
    /// Percentage of users in control group (0.0-1.0)
    pub control_pct: f64,
    /// Weights for test group
    pub test_weights: Weights,
    /// When the test starts
    pub start_time: DateTime<Utc>,
    /// When the test ends
    pub end_time: DateTime<Utc>,
    /// Minimum samples before results are valid
    pub min_samples: usize,
    /// Whether the test is currently active
    pub active: bool,
}

/// A/B test result with statistical analysis
/// Ported from Go: ABTestResult (abtest.go:30-46)
#[derive(Debug, Clone)]
pub struct ABTestResult {
    pub test_id: String,
    pub control_samples: i64,
    pub control_confirmed: i64,
    pub control_rejected: i64,
    pub control_avg_score: f64,
    pub test_samples: i64,
    pub test_confirmed: i64,
    pub test_rejected: i64,
    pub test_avg_score: f64,
    pub start_time: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub statistically_significant: bool,
    pub p_value: f64,
    /// Percentage improvement of test over control
    pub uplift: f64,
}

/// Atomic statistics for an A/B test
/// Ported from Go: ABTestStats (abtest.go:48-56)
pub struct ABTestStats {
    pub control_samples: AtomicI64,
    pub control_confirmed: AtomicI64,
    /// Stored as int64 (score * 10000)
    pub control_score_sum: AtomicI64,
    pub test_samples: AtomicI64,
    pub test_confirmed: AtomicI64,
    /// Stored as int64 (score * 10000)
    pub test_score_sum: AtomicI64,
}

impl Default for ABTestStats {
    fn default() -> Self {
        Self {
            control_samples: AtomicI64::new(0),
            control_confirmed: AtomicI64::new(0),
            control_score_sum: AtomicI64::new(0),
            test_samples: AtomicI64::new(0),
            test_confirmed: AtomicI64::new(0),
            test_score_sum: AtomicI64::new(0),
        }
    }
}

// =============================================================================
// A/B Test Manager
// =============================================================================

/// A/B test manager for weight optimization experiments
/// Ported from Go: ABTestManager (abtest.go:62-69)
pub struct ABTestManager {
    tests: RwLock<HashMap<String, ABTestConfig>>,
    stats: RwLock<HashMap<String, ABTestStats>>,
    base_weights: RwLock<Weights>,
}

impl Default for ABTestManager {
    fn default() -> Self {
        Self::new(Weights::default())
    }
}

impl ABTestManager {
    /// Create a new A/B test manager
    pub fn new(base_weights: Weights) -> Self {
        Self {
            tests: RwLock::new(HashMap::new()),
            stats: RwLock::new(HashMap::new()),
            base_weights: RwLock::new(base_weights),
        }
    }

    /// Create a new A/B test
    /// Ported from Go: ABTestManager.CreateTest (abtest.go:81-106)
    pub fn create_test(&self, mut cfg: ABTestConfig) -> Result<(), String> {
        // Validate and set defaults
        if cfg.control_pct <= 0.0 || cfg.control_pct >= 1.0 {
            cfg.control_pct = 0.5; // Default 50/50 split
        }
        if cfg.min_samples == 0 {
            cfg.min_samples = 100;
        }

        cfg.active = true;

        let test_id = cfg.test_id.clone();

        {
            let mut tests = self.tests.write().unwrap();
            tests.insert(test_id.clone(), cfg.clone());
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.insert(test_id.clone(), ABTestStats::default());
        }

        tracing::info!(
            test_id = test_id,
            name = cfg.name,
            control_pct = cfg.control_pct,
            "🧪 Created A/B test"
        );

        Ok(())
    }

    /// Get user bucket (deterministic 0.0-1.0)
    /// Ported from Go: ABTestManager.getUserBucket (abtest.go:138-143)
    fn get_user_bucket(&self, user_id: &str, test_id: &str) -> f64 {
        let mut hasher = DefaultHasher::new();
        format!("{}:{}", user_id, test_id).hash(&mut hasher);
        let hash = hasher.finish();
        hash as f64 / u64::MAX as f64
    }

    /// Get weights for a user based on active tests
    /// Ported from Go: ABTestManager.GetWeightsForUser (abtest.go:108-136)
    pub fn get_weights_for_user(&self, user_id: &str) -> (Weights, Option<String>) {
        let tests = self.tests.read().unwrap();
        let base_weights = self.base_weights.read().unwrap().clone();
        let now = Utc::now();

        for (test_id, test) in tests.iter() {
            if !test.active {
                continue;
            }
            if now < test.start_time || now > test.end_time {
                continue;
            }

            // Deterministic assignment based on user ID + test ID
            let bucket = self.get_user_bucket(user_id, test_id);

            if bucket >= test.control_pct {
                // Test group
                return (test.test_weights.clone(), Some(format!("{}:test", test_id)));
            }
            // Control group
            return (base_weights, Some(format!("{}:control", test_id)));
        }

        // No active test, use base weights
        (base_weights, None)
    }

    /// Record feedback for A/B test
    /// Ported from Go: ABTestManager.RecordFeedback (abtest.go:145-180)
    pub fn record_feedback(&self, user_id: &str, confirmed: bool, score: f64) {
        let tests = self.tests.read().unwrap();
        let stats = self.stats.read().unwrap();
        let now = Utc::now();

        for (test_id, test) in tests.iter() {
            if !test.active {
                continue;
            }
            if now < test.start_time || now > test.end_time {
                continue;
            }

            if let Some(test_stats) = stats.get(test_id) {
                let bucket = self.get_user_bucket(user_id, test_id);
                let score_int = (score * 10000.0) as i64;

                if bucket >= test.control_pct {
                    // Test group
                    test_stats.test_samples.fetch_add(1, Ordering::Relaxed);
                    test_stats
                        .test_score_sum
                        .fetch_add(score_int, Ordering::Relaxed);
                    if confirmed {
                        test_stats.test_confirmed.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    // Control group
                    test_stats.control_samples.fetch_add(1, Ordering::Relaxed);
                    test_stats
                        .control_score_sum
                        .fetch_add(score_int, Ordering::Relaxed);
                    if confirmed {
                        test_stats.control_confirmed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    /// Get test result with statistical analysis
    /// Ported from Go: ABTestManager.GetTestResult (abtest.go:182-229)
    pub fn get_test_result(&self, test_id: &str) -> Option<ABTestResult> {
        let tests = self.tests.read().unwrap();
        let stats = self.stats.read().unwrap();

        let test = tests.get(test_id)?;
        let test_stats = stats.get(test_id)?;

        let control_samples = test_stats.control_samples.load(Ordering::Relaxed);
        let test_samples = test_stats.test_samples.load(Ordering::Relaxed);
        let control_confirmed = test_stats.control_confirmed.load(Ordering::Relaxed);
        let test_confirmed = test_stats.test_confirmed.load(Ordering::Relaxed);

        let mut result = ABTestResult {
            test_id: test_id.to_string(),
            control_samples,
            control_confirmed,
            control_rejected: control_samples - control_confirmed,
            test_samples,
            test_confirmed,
            test_rejected: test_samples - test_confirmed,
            start_time: test.start_time,
            last_updated: Utc::now(),
            statistically_significant: false,
            p_value: 1.0,
            uplift: 0.0,
            control_avg_score: 0.0,
            test_avg_score: 0.0,
        };

        // Calculate average scores
        if control_samples > 0 {
            result.control_avg_score = test_stats.control_score_sum.load(Ordering::Relaxed) as f64
                / control_samples as f64
                / 10000.0;
        }
        if test_samples > 0 {
            result.test_avg_score = test_stats.test_score_sum.load(Ordering::Relaxed) as f64
                / test_samples as f64
                / 10000.0;
        }

        // Calculate uplift
        if result.control_avg_score > 0.0 {
            result.uplift = (result.test_avg_score - result.control_avg_score)
                / result.control_avg_score
                * 100.0;
        }

        // Calculate statistical significance
        let (significant, p_value) = self.calculate_significance(&result, test.min_samples);
        result.statistically_significant = significant;
        result.p_value = p_value;

        Some(result)
    }

    /// Calculate statistical significance (simplified chi-square test)
    /// Ported from Go: ABTestManager.calculateSignificance (abtest.go:231-260)
    fn calculate_significance(&self, result: &ABTestResult, min_samples: usize) -> (bool, f64) {
        if result.control_samples < min_samples as i64 || result.test_samples < min_samples as i64 {
            return (false, 1.0); // Not enough samples
        }

        // Calculate confirmation rates
        let control_rate = result.control_confirmed as f64 / result.control_samples as f64;
        let test_rate = result.test_confirmed as f64 / result.test_samples as f64;

        // Pooled rate
        let total_confirmed = result.control_confirmed + result.test_confirmed;
        let total_samples = result.control_samples + result.test_samples;
        let pooled_rate = total_confirmed as f64 / total_samples as f64;

        // Standard error
        let se = (pooled_rate
            * (1.0 - pooled_rate)
            * (1.0 / result.control_samples as f64 + 1.0 / result.test_samples as f64))
            .sqrt();

        if se == 0.0 {
            return (false, 1.0);
        }

        // Z-score
        let z = (test_rate - control_rate).abs() / se;

        // Approximate p-value (two-tailed)
        let p_value = 2.0 * (1.0 - normal_cdf(z));

        (p_value < 0.05, p_value)
    }

    /// End a test and get final results
    /// Ported from Go: ABTestManager.EndTest (abtest.go:268-294)
    pub fn end_test(&self, test_id: &str) -> Option<ABTestResult> {
        {
            let mut tests = self.tests.write().unwrap();
            if let Some(test) = tests.get_mut(test_id) {
                test.active = false;
            } else {
                return None;
            }
        }

        let result = self.get_test_result(test_id)?;

        tracing::info!(
            test_id = test_id,
            control_samples = result.control_samples,
            test_samples = result.test_samples,
            uplift = format!("{:.2}%", result.uplift),
            significant = result.statistically_significant,
            "🏁 A/B test ended"
        );

        Some(result)
    }

    /// Get all active tests
    /// Ported from Go: ABTestManager.GetActiveTests (abtest.go:296-311)
    pub fn get_active_tests(&self) -> Vec<ABTestConfig> {
        let tests = self.tests.read().unwrap();
        let now = Utc::now();

        tests
            .values()
            .filter(|t| t.active && now > t.start_time && now < t.end_time)
            .cloned()
            .collect()
    }

    /// Delete a test
    /// Ported from Go: ABTestManager.DeleteTest (abtest.go:313-320)
    pub fn delete_test(&self, test_id: &str) {
        self.tests.write().unwrap().remove(test_id);
        self.stats.write().unwrap().remove(test_id);
    }

    /// Set base weights (control group weights)
    /// Ported from Go: ABTestManager.SetBaseWeights (abtest.go:322-327)
    pub fn set_base_weights(&self, weights: Weights) {
        *self.base_weights.write().unwrap() = weights;
    }
}

/// Approximate normal CDF using error function
/// Ported from Go: normalCDF (abtest.go:262-266)
fn normal_cdf(z: f64) -> f64 {
    use std::f64::consts::SQRT_2;
    0.5 * (1.0 + erf(z / SQRT_2))
}

/// Approximate error function
fn erf(x: f64) -> f64 {
    // Approximation using Horner's method
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn create_test_config(test_id: &str) -> ABTestConfig {
        ABTestConfig {
            test_id: test_id.to_string(),
            name: "Test Experiment".to_string(),
            description: "Testing new weights".to_string(),
            control_pct: 0.5,
            test_weights: Weights {
                medication: 0.50,
                dosage: 0.20,
                quantity: 0.10,
                price: 0.10,
                recency: 0.10,
                ai_logic: 0.0,
            },
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now() + Duration::hours(1),
            min_samples: 10,
            active: true,
        }
    }

    #[test]
    fn test_create_test() {
        let manager = ABTestManager::default();
        let config = create_test_config("test-1");

        let result = manager.create_test(config);
        assert!(result.is_ok());

        let active = manager.get_active_tests();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].test_id, "test-1");
    }

    #[test]
    fn test_user_assignment_deterministic() {
        let manager = ABTestManager::default();
        let config = create_test_config("test-1");
        manager.create_test(config).unwrap();

        // Same user should always get same assignment
        let (w1, g1) = manager.get_weights_for_user("user-123");
        let (w2, g2) = manager.get_weights_for_user("user-123");

        assert_eq!(g1, g2);
        assert!((w1.medication - w2.medication).abs() < 0.001);
    }

    #[test]
    fn test_user_bucket_distribution() {
        let manager = ABTestManager::default();

        // Test that bucket values are in range [0, 1]
        for i in 0..100 {
            let bucket = manager.get_user_bucket(&format!("user-{}", i), "test-1");
            assert!(bucket >= 0.0);
            assert!(bucket <= 1.0);
        }
    }

    #[test]
    fn test_record_feedback() {
        let manager = ABTestManager::default();
        let config = create_test_config("test-1");
        manager.create_test(config).unwrap();

        // Record some feedback
        for i in 0..20 {
            let user_id = format!("user-{}", i);
            manager.record_feedback(&user_id, i % 2 == 0, 0.75);
        }

        let result = manager.get_test_result("test-1").unwrap();
        assert!(result.control_samples + result.test_samples == 20);
    }

    #[test]
    fn test_get_result_with_significance() {
        let manager = ABTestManager::default();
        let mut config = create_test_config("test-1");
        config.min_samples = 5;
        manager.create_test(config).unwrap();

        // Record enough samples
        for i in 0..50 {
            let user_id = format!("user-{}", i);
            manager.record_feedback(&user_id, i % 2 == 0, 0.7 + (i as f64 * 0.001));
        }

        let result = manager.get_test_result("test-1").unwrap();
        assert!(result.control_samples > 0);
        assert!(result.test_samples > 0);
        assert!(result.p_value >= 0.0 && result.p_value <= 1.0);
    }

    #[test]
    fn test_end_test() {
        let manager = ABTestManager::default();
        let config = create_test_config("test-1");
        manager.create_test(config).unwrap();

        let result = manager.end_test("test-1");
        assert!(result.is_some());

        // Test should no longer be active
        let active = manager.get_active_tests();
        assert!(active.is_empty());
    }

    #[test]
    fn test_delete_test() {
        let manager = ABTestManager::default();
        let config = create_test_config("test-1");
        manager.create_test(config).unwrap();

        manager.delete_test("test-1");

        assert!(manager.get_test_result("test-1").is_none());
    }

    #[test]
    fn test_no_active_test_returns_base_weights() {
        let base = Weights {
            medication: 0.40,
            dosage: 0.20,
            quantity: 0.15,
            price: 0.15,
            recency: 0.10,
            ai_logic: 0.0,
        };
        let manager = ABTestManager::new(base.clone());

        let (weights, group) = manager.get_weights_for_user("any-user");

        assert!(group.is_none());
        assert!((weights.medication - 0.40).abs() < 0.001);
    }

    #[test]
    fn test_normal_cdf() {
        // z=0 should give 0.5
        let cdf_0 = normal_cdf(0.0);
        assert!((cdf_0 - 0.5).abs() < 0.001);

        // z=1.96 should give ~0.975 (95% confidence)
        let cdf_196 = normal_cdf(1.96);
        assert!(cdf_196 > 0.97);
        assert!(cdf_196 < 0.98);
    }
}
