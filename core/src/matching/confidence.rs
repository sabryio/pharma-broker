//! Dynamic Confidence Threshold Management
//!
//! Ported from legacy/parsing/confidence.go
//!
//! Provides adaptive confidence thresholds that adjust based on acceptance rates.
//! This helps maintain optimal match quality as data patterns change over time.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for dynamic confidence thresholds
/// Ported from Go: ConfidenceConfig (confidence.go:14-27)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceConfig {
    /// Base strict threshold (default: 0.70)
    pub base_strict: f64,
    /// Base relaxed threshold (default: 0.40)
    pub base_relaxed: f64,
    /// Enable automatic threshold adjustment
    pub enable_adaptive: bool,
    /// How much to adjust per evaluation (default: 0.02)
    pub adjustment_step: f64,
    /// Minimum allowed threshold (default: 0.30)
    pub min_threshold: f64,
    /// Maximum allowed threshold (default: 0.95)
    pub max_threshold: f64,
    /// Number of results to evaluate before adjusting (default: 100)
    pub evaluation_window: usize,
    /// Target acceptance rate (default: 0.85)
    pub target_accept_rate: f64,
    /// Tolerance around target rate (default: 0.05)
    pub accept_rate_tolerance: f64,
}

impl Default for ConfidenceConfig {
    /// Default configuration - conservative with adaptive disabled
    /// Ported from Go: DefaultConfidenceConfig (confidence.go:30-43)
    fn default() -> Self {
        Self {
            base_strict: 0.70,
            base_relaxed: 0.40,
            enable_adaptive: false, // Disabled by default for stability
            adjustment_step: 0.02,
            min_threshold: 0.30,
            max_threshold: 0.95,
            evaluation_window: 100,
            target_accept_rate: 0.85,
            accept_rate_tolerance: 0.05,
        }
    }
}

impl ConfidenceConfig {
    /// Create an adaptive configuration
    pub fn adaptive() -> Self {
        Self {
            enable_adaptive: true,
            ..Default::default()
        }
    }

    /// Create a strict configuration
    pub fn strict() -> Self {
        Self {
            base_strict: 0.80,
            base_relaxed: 0.50,
            target_accept_rate: 0.75,
            ..Default::default()
        }
    }

    /// Create a permissive configuration
    pub fn permissive() -> Self {
        Self {
            base_strict: 0.60,
            base_relaxed: 0.35,
            target_accept_rate: 0.90,
            ..Default::default()
        }
    }
}

// =============================================================================
// Statistics (Lock-free atomics)
// =============================================================================

/// Atomic statistics for confidence tracking
/// Ported from Go: ConfidenceStats (confidence.go:46-52)
#[derive(Debug, Default)]
pub struct ConfidenceStats {
    /// Total items evaluated
    total_evaluations: AtomicU64,
    /// Items above threshold (accepted)
    accepted_items: AtomicU64,
    /// Items below threshold (rejected)
    rejected_items: AtomicU64,
    /// Number of threshold adjustments made
    threshold_adjustments: AtomicU64,
    /// Sum of confidence * 10000 (for averaging without floats)
    confidence_sum: AtomicU64,
}

impl ConfidenceStats {
    /// Get atomic values for building manager stats
    pub(super) fn get_values(&self) -> (u64, u64, u64, u64) {
        (
            self.total_evaluations.load(Ordering::Relaxed),
            self.accepted_items.load(Ordering::Relaxed),
            self.rejected_items.load(Ordering::Relaxed),
            self.threshold_adjustments.load(Ordering::Relaxed),
        )
    }

    /// Get current acceptance rate
    /// Ported from Go: ConfidenceStats.GetAcceptanceRate (confidence.go:65-71)
    pub fn acceptance_rate(&self) -> f64 {
        let total = self.total_evaluations.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0;
        }
        self.accepted_items.load(Ordering::Relaxed) as f64 / total as f64
    }

    /// Get average confidence score
    /// Ported from Go: ConfidenceStats.GetAverageConfidence (confidence.go:74-81)
    pub fn average_confidence(&self) -> f64 {
        let total = self.total_evaluations.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.confidence_sum.load(Ordering::Relaxed) as f64 / total as f64 / 10000.0
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_evaluations.store(0, Ordering::Relaxed);
        self.accepted_items.store(0, Ordering::Relaxed);
        self.rejected_items.store(0, Ordering::Relaxed);
        self.threshold_adjustments.store(0, Ordering::Relaxed);
        self.confidence_sum.store(0, Ordering::Relaxed);
    }
}

// =============================================================================
// Dynamic Confidence Manager
// =============================================================================

/// Window tracking for adaptive adjustment
#[derive(Debug, Default)]
struct AdaptiveWindow {
    accepted: usize,
    total: usize,
}

/// Dynamic confidence threshold manager
/// Ported from Go: ConfidenceManager (confidence.go:87-102)
pub struct ConfidenceManager {
    config: RwLock<ConfidenceConfig>,
    stats: ConfidenceStats,
    /// Current strict threshold (may differ from base if adaptive)
    current_strict: RwLock<f64>,
    /// Current relaxed threshold
    current_relaxed: RwLock<f64>,
    /// Window tracking for adaptive adjustment
    window: RwLock<AdaptiveWindow>,
}

impl Default for ConfidenceManager {
    fn default() -> Self {
        Self::new(ConfidenceConfig::default())
    }
}

impl ConfidenceManager {
    /// Create a new confidence manager
    /// Ported from Go: NewConfidenceManager (confidence.go:105-133)
    pub fn new(config: ConfidenceConfig) -> Self {
        let strict = config.base_strict;
        let relaxed = config.base_relaxed;

        Self {
            config: RwLock::new(config),
            stats: ConfidenceStats::default(),
            current_strict: RwLock::new(strict),
            current_relaxed: RwLock::new(relaxed),
            window: RwLock::new(AdaptiveWindow::default()),
        }
    }

    /// Create with adaptive mode enabled
    pub fn adaptive() -> Self {
        Self::new(ConfidenceConfig::adaptive())
    }

    // =========================================================================
    // Threshold Getters
    // =========================================================================

    /// Get current strict threshold
    /// Ported from Go: ConfidenceManager.GetStrictThreshold (confidence.go:136-138)
    pub fn strict_threshold(&self) -> f64 {
        *self.current_strict.read().unwrap()
    }

    /// Get current relaxed threshold
    /// Ported from Go: ConfidenceManager.GetRelaxedThreshold (confidence.go:141-143)
    pub fn relaxed_threshold(&self) -> f64 {
        *self.current_relaxed.read().unwrap()
    }

    // =========================================================================
    // Threshold Setters
    // =========================================================================

    /// Manually set strict threshold (clamped to bounds)
    /// Ported from Go: ConfidenceManager.SetStrictThreshold (confidence.go:146-157)
    pub fn set_strict_threshold(&self, threshold: f64) {
        let config = self.config.read().unwrap();
        let clamped = threshold.clamp(config.min_threshold, config.max_threshold);
        drop(config);

        *self.current_strict.write().unwrap() = clamped;
        tracing::info!(
            strict_threshold = clamped,
            "Strict confidence threshold updated"
        );
    }

    /// Manually set relaxed threshold (clamped to bounds)
    /// Ported from Go: ConfidenceManager.SetRelaxedThreshold (confidence.go:160-171)
    pub fn set_relaxed_threshold(&self, threshold: f64) {
        let config = self.config.read().unwrap();
        let clamped = threshold.clamp(config.min_threshold, config.max_threshold);
        drop(config);

        *self.current_relaxed.write().unwrap() = clamped;
        tracing::info!(
            relaxed_threshold = clamped,
            "Relaxed confidence threshold updated"
        );
    }

    // =========================================================================
    // Evaluation Methods
    // =========================================================================

    /// Evaluate confidence against strict threshold and track statistics
    /// Returns true if confidence meets the strict threshold
    /// Ported from Go: ConfidenceManager.EvaluateConfidence (confidence.go:174-193)
    pub fn evaluate(&self, confidence: f64) -> bool {
        // Update statistics (lock-free)
        self.stats.total_evaluations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .confidence_sum
            .fetch_add((confidence * 10000.0) as u64, Ordering::Relaxed);

        let strict = self.strict_threshold();
        let accepted = confidence >= strict;

        if accepted {
            self.stats.accepted_items.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.rejected_items.fetch_add(1, Ordering::Relaxed);
        }

        // Track for adaptive adjustment if enabled
        let config = self.config.read().unwrap();
        if config.enable_adaptive {
            drop(config);
            self.track_for_adaptive(accepted);
        }

        accepted
    }

    /// Evaluate against relaxed threshold (no tracking)
    /// Ported from Go: ConfidenceManager.EvaluateRelaxed (confidence.go:196-198)
    pub fn evaluate_relaxed(&self, confidence: f64) -> bool {
        confidence >= self.relaxed_threshold()
    }

    /// Track result for adaptive threshold adjustment
    /// Ported from Go: ConfidenceManager.trackForAdaptive (confidence.go:201-213)
    fn track_for_adaptive(&self, accepted: bool) {
        let mut window = self.window.write().unwrap();
        window.total += 1;
        if accepted {
            window.accepted += 1;
        }

        let config = self.config.read().unwrap();
        if window.total >= config.evaluation_window {
            let accepted = window.accepted;
            let total = window.total;
            drop(config);
            drop(window);
            self.adjust_thresholds(accepted, total);
        }
    }

    /// Adjust thresholds based on acceptance rate
    /// Ported from Go: ConfidenceManager.adjustThresholds (confidence.go:216-260)
    fn adjust_thresholds(&self, window_accepted: usize, window_total: usize) {
        if window_total == 0 {
            return;
        }

        let accept_rate = window_accepted as f64 / window_total as f64;
        let config = self.config.read().unwrap();

        let target_low = config.target_accept_rate - config.accept_rate_tolerance;
        let target_high = config.target_accept_rate + config.accept_rate_tolerance;

        let (adjustment, direction) = if accept_rate < target_low {
            // Too many rejections - lower threshold
            (-config.adjustment_step, "lowered")
        } else if accept_rate > target_high {
            // Too many acceptances - raise threshold
            (config.adjustment_step, "raised")
        } else {
            // Within tolerance - reset window and return
            drop(config);
            let mut window = self.window.write().unwrap();
            window.accepted = 0;
            window.total = 0;
            return;
        };

        // Apply adjustment to strict threshold
        let mut strict = self.current_strict.write().unwrap();
        let new_strict = (*strict + adjustment).clamp(config.min_threshold, config.max_threshold);

        if (new_strict - *strict).abs() > f64::EPSILON {
            *strict = new_strict;
            self.stats
                .threshold_adjustments
                .fetch_add(1, Ordering::Relaxed);

            tracing::info!(
                accept_rate = format!("{:.2}", accept_rate),
                target_rate = format!("{:.2}", config.target_accept_rate),
                new_strict_threshold = format!("{:.3}", new_strict),
                direction = direction,
                "🎚️ Adaptive threshold adjustment"
            );
        }

        drop(strict);
        drop(config);

        // Reset window
        let mut window = self.window.write().unwrap();
        window.accepted = 0;
        window.total = 0;
    }

    // =========================================================================
    // Statistics & Configuration
    // =========================================================================

    /// Get comprehensive statistics snapshot
    /// Ported from Go: ConfidenceManager.GetStats (confidence.go:263-277)
    pub fn get_stats(&self) -> ConfidenceManagerStats {
        let config = self.config.read().unwrap();
        let (total_evaluations, accepted_items, rejected_items, threshold_adjustments) =
            self.stats.get_values();

        ConfidenceManagerStats {
            total_evaluations,
            accepted_items,
            rejected_items,
            threshold_adjustments,
            acceptance_rate: self.stats.acceptance_rate(),
            average_confidence: self.stats.average_confidence(),
            current_strict: self.strict_threshold(),
            current_relaxed: self.relaxed_threshold(),
            adaptive_enabled: config.enable_adaptive,
        }
    }

    /// Get current configuration
    /// Ported from Go: ConfidenceManager.GetConfig (confidence.go:280-282)
    pub fn get_config(&self) -> ConfidenceConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    /// Ported from Go: ConfidenceManager.SetConfig (confidence.go:285-293)
    pub fn set_config(&self, config: ConfidenceConfig) {
        tracing::info!(
            base_strict = config.base_strict,
            base_relaxed = config.base_relaxed,
            adaptive = config.enable_adaptive,
            "Confidence configuration updated"
        );
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable adaptive adjustment
    /// Ported from Go: ConfidenceManager.EnableAdaptive (confidence.go:296-302)
    pub fn enable_adaptive(&self, enabled: bool) {
        self.config.write().unwrap().enable_adaptive = enabled;
        tracing::info!(enabled = enabled, "Adaptive confidence adjustment toggled");
    }

    /// Reset thresholds to base values
    /// Ported from Go: ConfidenceManager.ResetToBase (confidence.go:305-316)
    pub fn reset_to_base(&self) {
        let config = self.config.read().unwrap();
        let strict = config.base_strict;
        let relaxed = config.base_relaxed;
        drop(config);

        *self.current_strict.write().unwrap() = strict;
        *self.current_relaxed.write().unwrap() = relaxed;

        let mut window = self.window.write().unwrap();
        window.accepted = 0;
        window.total = 0;

        tracing::info!(
            strict = strict,
            relaxed = relaxed,
            "Confidence thresholds reset to base values"
        );
    }

    /// Reset all statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }
}

/// Comprehensive statistics from ConfidenceManager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceManagerStats {
    pub total_evaluations: u64,
    pub accepted_items: u64,
    pub rejected_items: u64,
    pub threshold_adjustments: u64,
    pub acceptance_rate: f64,
    pub average_confidence: f64,
    pub current_strict: f64,
    pub current_relaxed: f64,
    pub adaptive_enabled: bool,
}

// =============================================================================
// Tests - Ported from confidence_test.go patterns + rstest
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = ConfidenceConfig::default();

        assert!((config.base_strict - 0.70).abs() < 0.001);
        assert!((config.base_relaxed - 0.40).abs() < 0.001);
        assert!(!config.enable_adaptive);
        assert!((config.adjustment_step - 0.02).abs() < 0.001);
        assert!((config.min_threshold - 0.30).abs() < 0.001);
        assert!((config.max_threshold - 0.95).abs() < 0.001);
        assert_eq!(config.evaluation_window, 100);
        assert!((config.target_accept_rate - 0.85).abs() < 0.001);
        assert!((config.accept_rate_tolerance - 0.05).abs() < 0.001);
    }

    #[rstest]
    #[case(ConfidenceConfig::default(), 0.70, 0.40, false)]
    #[case(ConfidenceConfig::adaptive(), 0.70, 0.40, true)]
    #[case(ConfidenceConfig::strict(), 0.80, 0.50, false)]
    #[case(ConfidenceConfig::permissive(), 0.60, 0.35, false)]
    fn test_config_presets(
        #[case] config: ConfidenceConfig,
        #[case] expected_strict: f64,
        #[case] expected_relaxed: f64,
        #[case] expected_adaptive: bool,
    ) {
        assert!((config.base_strict - expected_strict).abs() < 0.001);
        assert!((config.base_relaxed - expected_relaxed).abs() < 0.001);
        assert_eq!(config.enable_adaptive, expected_adaptive);
    }

    // =========================================================================
    // Manager Creation Tests
    // =========================================================================

    #[test]
    fn test_new_manager_uses_base_thresholds() {
        let config = ConfidenceConfig {
            base_strict: 0.75,
            base_relaxed: 0.45,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        assert!((manager.strict_threshold() - 0.75).abs() < 0.001);
        assert!((manager.relaxed_threshold() - 0.45).abs() < 0.001);
    }

    #[test]
    fn test_default_manager() {
        let manager = ConfidenceManager::default();

        assert!((manager.strict_threshold() - 0.70).abs() < 0.001);
        assert!((manager.relaxed_threshold() - 0.40).abs() < 0.001);
    }

    // =========================================================================
    // Threshold Setter Tests
    // =========================================================================

    #[rstest]
    #[case(0.80, 0.80)] // Normal value
    #[case(0.20, 0.30)] // Below min, clamped to 0.30
    #[case(0.99, 0.95)] // Above max, clamped to 0.95
    #[case(0.50, 0.50)] // Exact middle
    fn test_set_strict_threshold_clamping(#[case] input: f64, #[case] expected: f64) {
        let manager = ConfidenceManager::default();
        manager.set_strict_threshold(input);
        assert!((manager.strict_threshold() - expected).abs() < 0.001);
    }

    #[rstest]
    #[case(0.50, 0.50)] // Normal value
    #[case(0.10, 0.30)] // Below min, clamped
    #[case(0.99, 0.95)] // Above max, clamped
    fn test_set_relaxed_threshold_clamping(#[case] input: f64, #[case] expected: f64) {
        let manager = ConfidenceManager::default();
        manager.set_relaxed_threshold(input);
        assert!((manager.relaxed_threshold() - expected).abs() < 0.001);
    }

    // =========================================================================
    // Evaluation Tests
    // =========================================================================

    #[rstest]
    #[case(0.80, true)] // Above strict (0.70)
    #[case(0.70, true)] // Exactly at strict
    #[case(0.69, false)] // Below strict
    #[case(0.50, false)] // Well below
    #[case(1.00, true)] // Maximum
    #[case(0.00, false)] // Minimum
    fn test_evaluate_strict(#[case] confidence: f64, #[case] expected: bool) {
        let manager = ConfidenceManager::default();
        assert_eq!(manager.evaluate(confidence), expected);
    }

    #[rstest]
    #[case(0.50, true)] // Above relaxed (0.40)
    #[case(0.40, true)] // Exactly at relaxed
    #[case(0.39, false)] // Below relaxed
    #[case(0.20, false)] // Well below
    fn test_evaluate_relaxed(#[case] confidence: f64, #[case] expected: bool) {
        let manager = ConfidenceManager::default();
        assert_eq!(manager.evaluate_relaxed(confidence), expected);
    }

    #[test]
    fn test_evaluate_tracks_statistics() {
        let manager = ConfidenceManager::default();

        // Evaluate some scores
        manager.evaluate(0.80); // Accepted
        manager.evaluate(0.75); // Accepted
        manager.evaluate(0.60); // Rejected
        manager.evaluate(0.50); // Rejected

        let stats = manager.get_stats();
        assert_eq!(stats.total_evaluations, 4);
        assert_eq!(stats.accepted_items, 2);
        assert_eq!(stats.rejected_items, 2);
        assert!((stats.acceptance_rate - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_average_confidence_calculation() {
        let manager = ConfidenceManager::default();

        manager.evaluate(0.80);
        manager.evaluate(0.60);
        manager.evaluate(0.40);
        manager.evaluate(0.20);

        // Average: (0.80 + 0.60 + 0.40 + 0.20) / 4 = 0.50
        let stats = manager.get_stats();
        assert!((stats.average_confidence - 0.50).abs() < 0.01);
    }

    // =========================================================================
    // Adaptive Threshold Tests
    // =========================================================================

    #[test]
    fn test_adaptive_lowers_threshold_on_low_acceptance() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: true,
            evaluation_window: 10, // Small window for testing
            target_accept_rate: 0.85,
            accept_rate_tolerance: 0.05,
            adjustment_step: 0.02,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate low acceptance rate (20% accepted)
        for i in 0..10 {
            let score = if i < 2 { 0.80 } else { 0.60 }; // 2 accepted, 8 rejected
            manager.evaluate(score);
        }

        // Threshold should have been lowered
        let new_strict = manager.strict_threshold();
        assert!(
            new_strict < 0.70,
            "Expected threshold < 0.70, got {}",
            new_strict
        );
        assert!((new_strict - 0.68).abs() < 0.001); // 0.70 - 0.02 = 0.68
    }

    #[test]
    fn test_adaptive_raises_threshold_on_high_acceptance() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: true,
            evaluation_window: 10,
            target_accept_rate: 0.50, // Low target for testing
            accept_rate_tolerance: 0.05,
            adjustment_step: 0.02,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate high acceptance rate (90% accepted)
        for i in 0..10 {
            let score = if i < 9 { 0.80 } else { 0.60 }; // 9 accepted, 1 rejected
            manager.evaluate(score);
        }

        // Threshold should have been raised
        let new_strict = manager.strict_threshold();
        assert!(
            new_strict > 0.70,
            "Expected threshold > 0.70, got {}",
            new_strict
        );
        assert!((new_strict - 0.72).abs() < 0.001); // 0.70 + 0.02 = 0.72
    }

    #[test]
    fn test_adaptive_no_change_within_tolerance() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: true,
            evaluation_window: 10,
            target_accept_rate: 0.80,
            accept_rate_tolerance: 0.10, // Wide tolerance
            adjustment_step: 0.02,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate 80% acceptance (within tolerance)
        for i in 0..10 {
            let score = if i < 8 { 0.80 } else { 0.60 };
            manager.evaluate(score);
        }

        // Threshold should remain unchanged
        assert!((manager.strict_threshold() - 0.70).abs() < 0.001);
    }

    #[test]
    fn test_adaptive_disabled_no_adjustment() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: false, // Disabled
            evaluation_window: 10,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate low acceptance
        for _ in 0..20 {
            manager.evaluate(0.60); // All rejected
        }

        // Threshold should remain unchanged
        assert!((manager.strict_threshold() - 0.70).abs() < 0.001);
    }

    #[test]
    fn test_adaptive_respects_min_threshold() {
        let config = ConfidenceConfig {
            base_strict: 0.35, // Close to min
            min_threshold: 0.30,
            enable_adaptive: true,
            evaluation_window: 10,
            target_accept_rate: 0.90,
            adjustment_step: 0.10, // Large step
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate very low acceptance
        for _ in 0..10 {
            manager.evaluate(0.20); // All rejected
        }

        // Should be clamped to min
        assert!((manager.strict_threshold() - 0.30).abs() < 0.001);
    }

    #[test]
    fn test_adaptive_respects_max_threshold() {
        let config = ConfidenceConfig {
            base_strict: 0.90, // Close to max
            max_threshold: 0.95,
            enable_adaptive: true,
            evaluation_window: 10,
            target_accept_rate: 0.10, // Very low target
            adjustment_step: 0.10,    // Large step
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Simulate very high acceptance
        for _ in 0..10 {
            manager.evaluate(0.95); // All accepted
        }

        // Should be clamped to max
        assert!((manager.strict_threshold() - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_threshold_adjustments_counted() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: true,
            evaluation_window: 5,
            target_accept_rate: 0.50,
            accept_rate_tolerance: 0.05,
            adjustment_step: 0.02,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // First window: high acceptance -> raise
        for _ in 0..5 {
            manager.evaluate(0.80);
        }

        // Second window: high acceptance -> raise again
        for _ in 0..5 {
            manager.evaluate(0.80);
        }

        let stats = manager.get_stats();
        assert_eq!(stats.threshold_adjustments, 2);
    }

    // =========================================================================
    // Configuration Management Tests
    // =========================================================================

    #[test]
    fn test_enable_adaptive_toggle() {
        let manager = ConfidenceManager::default();
        assert!(!manager.get_config().enable_adaptive);

        manager.enable_adaptive(true);
        assert!(manager.get_config().enable_adaptive);

        manager.enable_adaptive(false);
        assert!(!manager.get_config().enable_adaptive);
    }

    #[test]
    fn test_reset_to_base() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            base_relaxed: 0.40,
            enable_adaptive: true,
            evaluation_window: 5,
            target_accept_rate: 0.50,
            adjustment_step: 0.05,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Trigger adjustment
        for _ in 0..5 {
            manager.evaluate(0.80); // All accepted -> raise threshold
        }

        // Verify threshold changed
        assert!(manager.strict_threshold() > 0.70);

        // Reset
        manager.reset_to_base();

        // Verify back to base
        assert!((manager.strict_threshold() - 0.70).abs() < 0.001);
        assert!((manager.relaxed_threshold() - 0.40).abs() < 0.001);
    }

    #[test]
    fn test_set_config() {
        let manager = ConfidenceManager::default();

        let new_config = ConfidenceConfig {
            base_strict: 0.80,
            base_relaxed: 0.50,
            enable_adaptive: true,
            ..Default::default()
        };

        manager.set_config(new_config);

        let config = manager.get_config();
        assert!((config.base_strict - 0.80).abs() < 0.001);
        assert!((config.base_relaxed - 0.50).abs() < 0.001);
        assert!(config.enable_adaptive);
    }

    #[test]
    fn test_reset_stats() {
        let manager = ConfidenceManager::default();

        // Generate some stats
        for _ in 0..10 {
            manager.evaluate(0.75);
        }

        assert_eq!(manager.get_stats().total_evaluations, 10);

        // Reset
        manager.reset_stats();

        assert_eq!(manager.get_stats().total_evaluations, 0);
        assert_eq!(manager.get_stats().accepted_items, 0);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[rstest]
    #[case(100, 75, 0.75)]
    #[case(100, 100, 1.0)]
    #[case(100, 0, 0.0)]
    #[case(0, 0, 1.0)] // Edge case: no evaluations returns 1.0
    fn test_acceptance_rate(#[case] total: u64, #[case] accepted: u64, #[case] expected: f64) {
        let stats = ConfidenceStats::default();
        stats.total_evaluations.store(total, Ordering::Relaxed);
        stats.accepted_items.store(accepted, Ordering::Relaxed);

        assert!((stats.acceptance_rate() - expected).abs() < 0.001);
    }

    #[test]
    fn test_get_stats_comprehensive() {
        let config = ConfidenceConfig {
            base_strict: 0.70,
            enable_adaptive: true,
            ..Default::default()
        };
        let manager = ConfidenceManager::new(config);

        // Evaluate some scores
        manager.evaluate(0.80);
        manager.evaluate(0.60);

        let stats = manager.get_stats();

        assert_eq!(stats.total_evaluations, 2);
        assert_eq!(stats.accepted_items, 1);
        assert_eq!(stats.rejected_items, 1);
        assert!((stats.acceptance_rate - 0.50).abs() < 0.001);
        assert!((stats.current_strict - 0.70).abs() < 0.001);
        assert!((stats.current_relaxed - 0.40).abs() < 0.001);
        assert!(stats.adaptive_enabled);
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_concurrent_evaluations() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(ConfidenceManager::default());
        let mut handles = vec![];

        // Spawn multiple threads evaluating concurrently
        for _ in 0..4 {
            let m = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    m.evaluate(0.75);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All evaluations should be counted
        assert_eq!(manager.get_stats().total_evaluations, 400);
    }
}
