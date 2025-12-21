//! Confidence Calibration Module
//!
//! Ported from legacy/parsing/calibration.go
//!
//! Calibrates AI confidence scores based on actual outcomes using histogram binning.
//! This helps correct systematic over/under-confidence in predictions.
//!
//! Key concepts:
//! - ECE (Expected Calibration Error): Weighted average of bin calibration errors
//! - MCE (Maximum Calibration Error): Worst-case bin error
//! - Bins: Confidence ranges that track predicted vs actual outcome rates

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for confidence calibration
/// Ported from Go: CalibrationConfig (calibration.go:17-32)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationConfig {
    /// Enable calibration
    pub enabled: bool,
    /// Number of bins for calibration (default: 10)
    pub num_bins: usize,
    /// Minimum samples per bin before calibration applies (default: 20)
    pub min_samples_per_bin: usize,
    /// Smoothing factor for blending calibrated with raw (default: 1.0 = full calibration)
    pub smoothing_factor: f64,
    /// Window size for recent outcomes (default: 1000)
    pub window_size: usize,
}

impl Default for CalibrationConfig {
    /// Default configuration
    /// Ported from Go: DefaultCalibrationConfig (calibration.go:35-43)
    fn default() -> Self {
        Self {
            enabled: true,
            num_bins: 10,
            min_samples_per_bin: 20,
            smoothing_factor: 1.0,
            window_size: 1000,
        }
    }
}

impl CalibrationConfig {
    /// Create a disabled configuration
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a conservative configuration (requires more samples)
    pub fn conservative() -> Self {
        Self {
            min_samples_per_bin: 50,
            smoothing_factor: 0.7, // Blend 70% calibrated, 30% raw
            ..Default::default()
        }
    }

    /// Create an aggressive configuration (fewer samples needed)
    pub fn aggressive() -> Self {
        Self {
            min_samples_per_bin: 10,
            smoothing_factor: 1.0, // Full calibration
            ..Default::default()
        }
    }
}

// =============================================================================
// Statistics (Lock-free atomics)
// =============================================================================

/// Atomic statistics for calibration tracking
/// Ported from Go: CalibrationStats (calibration.go:49-55)
#[derive(Debug, Default)]
pub struct CalibrationStats {
    /// Total prediction-outcome pairs recorded
    total_samples: AtomicU64,
    /// Samples that were calibrated (had enough bin data)
    calibrated_samples: AtomicU64,
    /// Overconfident: predicted high (>=0.7), actual negative
    overconfident_hits: AtomicU64,
    /// Underconfident: predicted low (<0.5), actual positive
    underconfident_hits: AtomicU64,
}

impl CalibrationStats {
    /// Get a snapshot of current statistics
    /// Ported from Go: CalibrationStats.GetStats (calibration.go:58-65)
    pub fn snapshot(&self) -> CalibrationStatsSnapshot {
        CalibrationStatsSnapshot {
            total_samples: self.total_samples.load(Ordering::Relaxed),
            calibrated_samples: self.calibrated_samples.load(Ordering::Relaxed),
            overconfident_hits: self.overconfident_hits.load(Ordering::Relaxed),
            underconfident_hits: self.underconfident_hits.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_samples.store(0, Ordering::Relaxed);
        self.calibrated_samples.store(0, Ordering::Relaxed);
        self.overconfident_hits.store(0, Ordering::Relaxed);
        self.underconfident_hits.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of calibration statistics (for serialization)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationStatsSnapshot {
    pub total_samples: u64,
    pub calibrated_samples: u64,
    pub overconfident_hits: u64,
    pub underconfident_hits: u64,
}

// =============================================================================
// Calibration Bin
// =============================================================================

/// Statistics for a confidence range bin
/// Ported from Go: CalibrationBin (calibration.go:71-79)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationBin {
    /// Lower bound of bin (inclusive)
    pub lower_bound: f64,
    /// Upper bound of bin (exclusive)
    pub upper_bound: f64,
    /// Total predictions in this bin
    pub total_count: u64,
    /// Actual positive outcomes
    pub positive_count: u64,
    /// Running mean of predicted confidence
    pub mean_predicted: f64,
    /// Running mean of actual outcome rate
    pub mean_actual: f64,
}

impl CalibrationBin {
    /// Create a new bin with given bounds
    pub fn new(lower: f64, upper: f64) -> Self {
        Self {
            lower_bound: lower,
            upper_bound: upper,
            total_count: 0,
            positive_count: 0,
            mean_predicted: 0.0,
            mean_actual: 0.0,
        }
    }

    /// Get calibration error for this bin (|predicted - actual|)
    /// Ported from Go: CalibrationBin.GetCalibrationError (calibration.go:82-87)
    pub fn calibration_error(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        (self.mean_predicted - self.mean_actual).abs()
    }

    /// Check if bin has enough samples for calibration
    /// Ported from Go: CalibrationBin.IsCalibrated (calibration.go:90-92)
    pub fn is_calibrated(&self, min_samples: usize) -> bool {
        self.total_count >= min_samples as u64
    }

    /// Reset bin statistics
    pub fn reset(&mut self) {
        self.total_count = 0;
        self.positive_count = 0;
        self.mean_predicted = 0.0;
        self.mean_actual = 0.0;
    }
}

// =============================================================================
// Outcome Record (for windowed tracking)
// =============================================================================

/// Single prediction-outcome pair
/// Ported from Go: outcomeRecord (calibration.go:107-111)
#[derive(Debug, Clone)]
struct OutcomeRecord {
    predicted: f64,
    actual: bool,
    timestamp: DateTime<Utc>,
}

impl Default for OutcomeRecord {
    fn default() -> Self {
        Self {
            predicted: 0.0,
            actual: false,
            timestamp: Utc::now(),
        }
    }
}

// =============================================================================
// Confidence Calibrator
// =============================================================================

/// Calibrates AI confidence scores based on actual outcomes
/// Ported from Go: ConfidenceCalibrator (calibration.go:98-111)
pub struct ConfidenceCalibrator {
    config: RwLock<CalibrationConfig>,
    stats: CalibrationStats,
    bins: RwLock<Vec<CalibrationBin>>,
    /// Circular buffer for recent outcomes
    recent_outcomes: RwLock<Vec<OutcomeRecord>>,
    recent_idx: RwLock<usize>,
}

impl Default for ConfidenceCalibrator {
    fn default() -> Self {
        Self::new(CalibrationConfig::default())
    }
}

impl ConfidenceCalibrator {
    /// Create a new confidence calibrator
    /// Ported from Go: NewConfidenceCalibrator (calibration.go:114-136)
    pub fn new(config: CalibrationConfig) -> Self {
        let num_bins = if config.num_bins == 0 {
            10
        } else {
            config.num_bins
        };
        let window_size = if config.window_size == 0 {
            1000
        } else {
            config.window_size
        };

        // Initialize bins
        let bin_width = 1.0 / num_bins as f64;
        let bins: Vec<CalibrationBin> = (0..num_bins)
            .map(|i| CalibrationBin::new(i as f64 * bin_width, (i + 1) as f64 * bin_width))
            .collect();

        Self {
            config: RwLock::new(config),
            stats: CalibrationStats::default(),
            bins: RwLock::new(bins),
            recent_outcomes: RwLock::new(vec![OutcomeRecord::default(); window_size]),
            recent_idx: RwLock::new(0),
        }
    }

    /// Create a disabled calibrator
    pub fn disabled() -> Self {
        Self::new(CalibrationConfig::disabled())
    }

    // =========================================================================
    // Core Calibration Methods
    // =========================================================================

    /// Calibrate a raw confidence score based on historical data
    /// Ported from Go: ConfidenceCalibrator.Calibrate (calibration.go:139-159)
    pub fn calibrate(&self, raw_confidence: f64) -> f64 {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return raw_confidence;
        }

        let bins = self.bins.read().unwrap();
        let bin_idx = self.get_bin_index(raw_confidence, bins.len());
        let bin = &bins[bin_idx];

        // If not enough samples, return raw confidence
        if !bin.is_calibrated(config.min_samples_per_bin) {
            return raw_confidence;
        }

        // Apply calibration with smoothing
        let calibrated = self.interpolate_calibration(raw_confidence, bin, config.smoothing_factor);

        self.stats
            .calibrated_samples
            .fetch_add(1, Ordering::Relaxed);

        calibrated
    }

    /// Record a prediction-outcome pair for calibration learning
    /// Ported from Go: ConfidenceCalibrator.RecordOutcome (calibration.go:162-199)
    pub fn record_outcome(&self, predicted_confidence: f64, actual_positive: bool) {
        self.stats.total_samples.fetch_add(1, Ordering::Relaxed);

        // Update bin statistics
        let mut bins = self.bins.write().unwrap();
        let bin_idx = self.get_bin_index(predicted_confidence, bins.len());
        let bin = &mut bins[bin_idx];

        bin.total_count += 1;
        if actual_positive {
            bin.positive_count += 1;
        }

        // Update running means using incremental formula
        // new_mean = old_mean + (new_value - old_mean) / n
        let n = bin.total_count as f64;
        bin.mean_predicted += (predicted_confidence - bin.mean_predicted) / n;

        let actual_val = if actual_positive { 1.0 } else { 0.0 };
        bin.mean_actual += (actual_val - bin.mean_actual) / n;

        drop(bins);

        // Track over/under confidence
        if predicted_confidence >= 0.7 && !actual_positive {
            self.stats
                .overconfident_hits
                .fetch_add(1, Ordering::Relaxed);
        } else if predicted_confidence < 0.5 && actual_positive {
            self.stats
                .underconfident_hits
                .fetch_add(1, Ordering::Relaxed);
        }

        // Store in recent window (circular buffer)
        let config = self.config.read().unwrap();
        let window_size = config.window_size;
        drop(config);

        let mut outcomes = self.recent_outcomes.write().unwrap();
        let mut idx = self.recent_idx.write().unwrap();

        if !outcomes.is_empty() {
            outcomes[*idx] = OutcomeRecord {
                predicted: predicted_confidence,
                actual: actual_positive,
                timestamp: Utc::now(),
            };
            *idx = (*idx + 1) % window_size;
        }

        // Log periodically
        let total = self.stats.total_samples.load(Ordering::Relaxed);
        if total.is_multiple_of(100) {
            self.log_calibration_status();
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Get bin index for a confidence value
    /// Ported from Go: ConfidenceCalibrator.getBinIndex (calibration.go:202-211)
    fn get_bin_index(&self, confidence: f64, num_bins: usize) -> usize {
        let confidence = confidence.clamp(0.0, 1.0);
        if confidence >= 1.0 {
            return num_bins - 1;
        }
        let bin_width = 1.0 / num_bins as f64;
        (confidence / bin_width) as usize
    }

    /// Interpolate calibration with smoothing
    /// Ported from Go: ConfidenceCalibrator.interpolateCalibration (calibration.go:214-234)
    fn interpolate_calibration(&self, raw: f64, bin: &CalibrationBin, smoothing: f64) -> f64 {
        if bin.total_count == 0 {
            return raw;
        }

        // Map to actual outcome rate with smoothing
        // smoothing=1.0 means full calibration, 0.0 means raw
        let calibrated = bin.mean_actual;
        let smoothed = smoothing * calibrated + (1.0 - smoothing) * raw;

        smoothed.clamp(0.0, 1.0)
    }

    /// Log current calibration status
    fn log_calibration_status(&self) {
        let ece = self.expected_calibration_error();
        let stats = self.stats.snapshot();

        tracing::info!(
            ece = format!("{:.4}", ece),
            total_samples = stats.total_samples,
            overconfident = stats.overconfident_hits,
            underconfident = stats.underconfident_hits,
            "📊 Calibration status update"
        );
    }

    // =========================================================================
    // Calibration Metrics
    // =========================================================================

    /// Calculate Expected Calibration Error (ECE)
    /// ECE = Σ (|bin| / n) * |accuracy(bin) - confidence(bin)|
    /// Ported from Go: ConfidenceCalibrator.GetExpectedCalibrationError (calibration.go:243-259)
    pub fn expected_calibration_error(&self) -> f64 {
        let bins = self.bins.read().unwrap();

        let mut total_samples: u64 = 0;
        let mut weighted_error: f64 = 0.0;

        for bin in bins.iter() {
            if bin.total_count > 0 {
                total_samples += bin.total_count;
                weighted_error += bin.total_count as f64 * bin.calibration_error();
            }
        }

        if total_samples == 0 {
            return 0.0;
        }

        weighted_error / total_samples as f64
    }

    /// Calculate Maximum Calibration Error (MCE)
    /// MCE = max over all bins of |accuracy(bin) - confidence(bin)|
    /// Ported from Go: ConfidenceCalibrator.GetMaxCalibrationError (calibration.go:262-275)
    pub fn max_calibration_error(&self) -> f64 {
        let bins = self.bins.read().unwrap();

        bins.iter()
            .filter(|b| b.total_count > 0)
            .map(|b| b.calibration_error())
            .fold(0.0_f64, f64::max)
    }

    /// Get a detailed calibration report
    /// Ported from Go: ConfidenceCalibrator.GetCalibrationReport (calibration.go:278-303)
    pub fn get_report(&self) -> CalibrationReport {
        let bins = self.bins.read().unwrap();
        let config = self.config.read().unwrap();
        let stats = self.stats.snapshot();

        let bin_reports: Vec<BinReport> = bins
            .iter()
            .map(|bin| BinReport {
                range: (bin.lower_bound, bin.upper_bound),
                count: bin.total_count,
                mean_predicted: bin.mean_predicted,
                mean_actual: bin.mean_actual,
                error: bin.calibration_error(),
                is_calibrated: bin.is_calibrated(config.min_samples_per_bin),
            })
            .collect();

        let total = stats.total_samples;
        let overconfident_pct = if total > 0 {
            stats.overconfident_hits as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let underconfident_pct = if total > 0 {
            stats.underconfident_hits as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        CalibrationReport {
            ece: self.expected_calibration_error(),
            mce: self.max_calibration_error(),
            total_samples: total,
            calibrated_samples: stats.calibrated_samples,
            overconfident_pct,
            underconfident_pct,
            bin_reports,
            is_well_calibrated: self.expected_calibration_error() < 0.1,
        }
    }

    // =========================================================================
    // Configuration Methods
    // =========================================================================

    /// Get current configuration
    /// Ported from Go: ConfidenceCalibrator.GetConfig (calibration.go:330-334)
    pub fn get_config(&self) -> CalibrationConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    /// Ported from Go: ConfidenceCalibrator.SetConfig (calibration.go:337-346)
    pub fn set_config(&self, config: CalibrationConfig) {
        tracing::info!(
            enabled = config.enabled,
            num_bins = config.num_bins,
            smoothing = config.smoothing_factor,
            "Calibration configuration updated"
        );
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable calibration
    /// Ported from Go: ConfidenceCalibrator.Enable (calibration.go:349-356)
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
        tracing::info!(enabled = enabled, "Calibration toggled");
    }

    /// Set smoothing factor (0.0 = raw, 1.0 = full calibration)
    /// Ported from Go: ConfidenceCalibrator.SetSmoothingFactor (calibration.go:359-370)
    pub fn set_smoothing_factor(&self, factor: f64) {
        let factor = factor.clamp(0.0, 1.0);
        self.config.write().unwrap().smoothing_factor = factor;
        tracing::info!(smoothing_factor = factor, "Smoothing factor updated");
    }

    /// Get current statistics snapshot
    pub fn get_stats(&self) -> CalibrationStatsSnapshot {
        self.stats.snapshot()
    }

    /// Check if calibration is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Reset all calibration data
    /// Ported from Go: ConfidenceCalibrator.Reset (calibration.go:378-397)
    pub fn reset(&self) {
        // Reset bins
        let config = self.config.read().unwrap();
        let num_bins = config.num_bins;
        let window_size = config.window_size;
        drop(config);

        let bin_width = 1.0 / num_bins as f64;
        let new_bins: Vec<CalibrationBin> = (0..num_bins)
            .map(|i| CalibrationBin::new(i as f64 * bin_width, (i + 1) as f64 * bin_width))
            .collect();

        *self.bins.write().unwrap() = new_bins;

        // Reset stats
        self.stats.reset();

        // Reset recent outcomes
        *self.recent_outcomes.write().unwrap() = vec![OutcomeRecord::default(); window_size];
        *self.recent_idx.write().unwrap() = 0;

        tracing::info!("Calibration data reset");
    }
}

// =============================================================================
// Report Types
// =============================================================================

/// Detailed calibration report
/// Ported from Go: CalibrationReport (calibration.go:306-314)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    /// Expected Calibration Error
    pub ece: f64,
    /// Maximum Calibration Error
    pub mce: f64,
    /// Total samples recorded
    pub total_samples: u64,
    /// Samples that were calibrated
    pub calibrated_samples: u64,
    /// Percentage of overconfident predictions
    pub overconfident_pct: f64,
    /// Percentage of underconfident predictions
    pub underconfident_pct: f64,
    /// Per-bin statistics
    pub bin_reports: Vec<BinReport>,
    /// Whether ECE < 0.1 (well calibrated)
    pub is_well_calibrated: bool,
}

/// Statistics for a single calibration bin
/// Ported from Go: BinReport (calibration.go:317-325)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinReport {
    /// (lower, upper) bounds
    pub range: (f64, f64),
    /// Number of samples in bin
    pub count: u64,
    /// Mean predicted confidence
    pub mean_predicted: f64,
    /// Mean actual outcome rate
    pub mean_actual: f64,
    /// Calibration error |predicted - actual|
    pub error: f64,
    /// Has enough samples for calibration
    pub is_calibrated: bool,
}

impl CalibrationReport {
    /// Check if well calibrated with custom threshold
    pub fn is_well_calibrated_with_threshold(&self, threshold: f64) -> bool {
        self.ece < threshold
    }
}

// =============================================================================
// Tests - Ported from calibration_test.go patterns + rstest
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
        let config = CalibrationConfig::default();

        assert!(config.enabled);
        assert_eq!(config.num_bins, 10);
        assert_eq!(config.min_samples_per_bin, 20);
        assert!((config.smoothing_factor - 1.0).abs() < 0.001);
        assert_eq!(config.window_size, 1000);
    }

    #[rstest]
    #[case(CalibrationConfig::default(), true, 10, 20)]
    #[case(CalibrationConfig::disabled(), false, 10, 20)]
    #[case(CalibrationConfig::conservative(), true, 10, 50)]
    #[case(CalibrationConfig::aggressive(), true, 10, 10)]
    fn test_config_presets(
        #[case] config: CalibrationConfig,
        #[case] enabled: bool,
        #[case] num_bins: usize,
        #[case] min_samples: usize,
    ) {
        assert_eq!(config.enabled, enabled);
        assert_eq!(config.num_bins, num_bins);
        assert_eq!(config.min_samples_per_bin, min_samples);
    }

    // =========================================================================
    // Bin Tests
    // =========================================================================

    #[test]
    fn test_bin_creation() {
        let bin = CalibrationBin::new(0.0, 0.1);

        assert!((bin.lower_bound - 0.0).abs() < 0.001);
        assert!((bin.upper_bound - 0.1).abs() < 0.001);
        assert_eq!(bin.total_count, 0);
        assert_eq!(bin.positive_count, 0);
    }

    #[rstest]
    #[case(0.8, 0.6, 0.2)] // Overconfident
    #[case(0.5, 0.5, 0.0)] // Perfect
    #[case(0.3, 0.7, 0.4)] // Underconfident
    fn test_bin_calibration_error(
        #[case] mean_predicted: f64,
        #[case] mean_actual: f64,
        #[case] expected_error: f64,
    ) {
        let mut bin = CalibrationBin::new(0.0, 1.0);
        bin.total_count = 100;
        bin.mean_predicted = mean_predicted;
        bin.mean_actual = mean_actual;

        assert!((bin.calibration_error() - expected_error).abs() < 0.001);
    }

    #[rstest]
    #[case(0, 20, false)]
    #[case(19, 20, false)]
    #[case(20, 20, true)]
    #[case(100, 20, true)]
    fn test_bin_is_calibrated(
        #[case] count: u64,
        #[case] min_samples: usize,
        #[case] expected: bool,
    ) {
        let mut bin = CalibrationBin::new(0.0, 1.0);
        bin.total_count = count;

        assert_eq!(bin.is_calibrated(min_samples), expected);
    }

    // =========================================================================
    // Calibrator Creation Tests
    // =========================================================================

    #[test]
    fn test_calibrator_creation() {
        let calibrator = ConfidenceCalibrator::default();

        let config = calibrator.get_config();
        assert!(config.enabled);
        assert_eq!(config.num_bins, 10);

        let bins = calibrator.bins.read().unwrap();
        assert_eq!(bins.len(), 10);

        // Check bin boundaries
        assert!((bins[0].lower_bound - 0.0).abs() < 0.001);
        assert!((bins[0].upper_bound - 0.1).abs() < 0.001);
        assert!((bins[9].lower_bound - 0.9).abs() < 0.001);
        assert!((bins[9].upper_bound - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_calibrator_disabled() {
        let calibrator = ConfidenceCalibrator::disabled();

        assert!(!calibrator.is_enabled());
        // Should return raw confidence when disabled
        assert!((calibrator.calibrate(0.75) - 0.75).abs() < 0.001);
    }

    // =========================================================================
    // Bin Index Tests
    // =========================================================================

    #[rstest]
    #[case(0.0, 0)]
    #[case(0.05, 0)]
    #[case(0.1, 1)]
    #[case(0.15, 1)]
    #[case(0.5, 5)]
    #[case(0.95, 9)]
    #[case(1.0, 9)] // Edge case: 1.0 goes to last bin
    #[case(-0.1, 0)] // Edge case: negative clamped to 0
    #[case(1.5, 9)] // Edge case: >1 clamped to last bin
    fn test_get_bin_index(#[case] confidence: f64, #[case] expected_idx: usize) {
        let calibrator = ConfidenceCalibrator::default();
        let idx = calibrator.get_bin_index(confidence, 10);
        assert_eq!(idx, expected_idx);
    }

    // =========================================================================
    // Record Outcome Tests
    // =========================================================================

    #[test]
    fn test_record_outcome_updates_bin() {
        let calibrator = ConfidenceCalibrator::default();

        // Record some outcomes in the 0.7-0.8 bin (index 7)
        calibrator.record_outcome(0.75, true);
        calibrator.record_outcome(0.72, true);
        calibrator.record_outcome(0.78, false);

        let bins = calibrator.bins.read().unwrap();
        let bin = &bins[7];

        assert_eq!(bin.total_count, 3);
        assert_eq!(bin.positive_count, 2);
        // Mean predicted ≈ (0.75 + 0.72 + 0.78) / 3 = 0.75
        assert!((bin.mean_predicted - 0.75).abs() < 0.01);
        // Mean actual = 2/3 ≈ 0.667
        assert!((bin.mean_actual - 0.667).abs() < 0.01);
    }

    #[test]
    fn test_record_outcome_tracks_overconfidence() {
        let calibrator = ConfidenceCalibrator::default();

        // High confidence, negative outcome = overconfident
        calibrator.record_outcome(0.85, false);
        calibrator.record_outcome(0.90, false);

        let stats = calibrator.get_stats();
        assert_eq!(stats.overconfident_hits, 2);
        assert_eq!(stats.underconfident_hits, 0);
    }

    #[test]
    fn test_record_outcome_tracks_underconfidence() {
        let calibrator = ConfidenceCalibrator::default();

        // Low confidence, positive outcome = underconfident
        calibrator.record_outcome(0.30, true);
        calibrator.record_outcome(0.40, true);

        let stats = calibrator.get_stats();
        assert_eq!(stats.overconfident_hits, 0);
        assert_eq!(stats.underconfident_hits, 2);
    }

    #[test]
    fn test_record_outcome_updates_stats() {
        let calibrator = ConfidenceCalibrator::default();

        for i in 0..10 {
            calibrator.record_outcome(0.5 + (i as f64 * 0.01), i % 2 == 0);
        }

        let stats = calibrator.get_stats();
        assert_eq!(stats.total_samples, 10);
    }

    // =========================================================================
    // Calibration Tests
    // =========================================================================

    #[test]
    fn test_calibrate_returns_raw_when_insufficient_samples() {
        let calibrator = ConfidenceCalibrator::default();

        // Record only 5 samples (less than min_samples_per_bin=20)
        for _ in 0..5 {
            calibrator.record_outcome(0.75, true);
        }

        // Should return raw confidence
        let calibrated = calibrator.calibrate(0.75);
        assert!((calibrated - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_calibrate_adjusts_with_sufficient_samples() {
        let config = CalibrationConfig {
            min_samples_per_bin: 5, // Lower threshold for testing
            smoothing_factor: 1.0,  // Full calibration
            ..Default::default()
        };
        let calibrator = ConfidenceCalibrator::new(config);

        // Record outcomes: 80% predicted, but only 50% actual positive
        for i in 0..10 {
            calibrator.record_outcome(0.82, i < 5); // 5 positive, 5 negative
        }

        // Calibrated should be closer to 0.5 (actual rate)
        let calibrated = calibrator.calibrate(0.82);
        assert!(calibrated < 0.82); // Should be pulled down
        assert!((calibrated - 0.5).abs() < 0.1); // Should be near 0.5
    }

    #[test]
    fn test_calibrate_with_smoothing() {
        let config = CalibrationConfig {
            min_samples_per_bin: 5,
            smoothing_factor: 0.5, // 50% calibrated, 50% raw
            ..Default::default()
        };
        let calibrator = ConfidenceCalibrator::new(config);

        // Record outcomes: 80% predicted, 40% actual
        for i in 0..10 {
            calibrator.record_outcome(0.82, i < 4);
        }

        let calibrated = calibrator.calibrate(0.82);
        // Expected: 0.5 * 0.4 + 0.5 * 0.82 = 0.61
        assert!((calibrated - 0.61).abs() < 0.05);
    }

    #[test]
    fn test_calibrate_disabled_returns_raw() {
        let calibrator = ConfidenceCalibrator::disabled();

        // Even with samples, should return raw
        for _ in 0..50 {
            calibrator.record_outcome(0.75, true);
        }

        let calibrated = calibrator.calibrate(0.75);
        assert!((calibrated - 0.75).abs() < 0.001);
    }

    // =========================================================================
    // Calibration Metrics Tests
    // =========================================================================

    #[test]
    fn test_ece_zero_when_no_samples() {
        let calibrator = ConfidenceCalibrator::default();
        assert!((calibrator.expected_calibration_error() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ece_calculation() {
        let config = CalibrationConfig {
            num_bins: 2, // Simple 2-bin setup
            min_samples_per_bin: 1,
            ..Default::default()
        };
        let calibrator = ConfidenceCalibrator::new(config);

        // Bin 0 (0.0-0.5): predict 0.3, actual 0.5 -> error 0.2
        for i in 0..10 {
            calibrator.record_outcome(0.3, i < 5);
        }

        // Bin 1 (0.5-1.0): predict 0.8, actual 0.8 -> error 0.0
        for i in 0..10 {
            calibrator.record_outcome(0.8, i < 8);
        }

        // ECE = (10 * 0.2 + 10 * 0.0) / 20 = 0.1
        let ece = calibrator.expected_calibration_error();
        assert!((ece - 0.1).abs() < 0.05);
    }

    #[test]
    fn test_mce_calculation() {
        let config = CalibrationConfig {
            num_bins: 2,
            min_samples_per_bin: 1,
            ..Default::default()
        };
        let calibrator = ConfidenceCalibrator::new(config);

        // Bin 0: error 0.2
        for i in 0..10 {
            calibrator.record_outcome(0.3, i < 5);
        }

        // Bin 1: error 0.3
        for i in 0..10 {
            calibrator.record_outcome(0.8, i < 5);
        }

        let mce = calibrator.max_calibration_error();
        // Max error should be around 0.3
        assert!(mce > 0.2);
    }

    #[test]
    fn test_calibration_report() {
        let config = CalibrationConfig {
            num_bins: 5,
            min_samples_per_bin: 2,
            ..Default::default()
        };
        let calibrator = ConfidenceCalibrator::new(config);

        // Add some samples
        for _ in 0..10 {
            calibrator.record_outcome(0.75, true);
        }
        for _ in 0..5 {
            calibrator.record_outcome(0.85, false); // Overconfident
        }

        let report = calibrator.get_report();

        assert_eq!(report.total_samples, 15);
        assert_eq!(report.bin_reports.len(), 5);
        assert!(report.overconfident_pct > 0.0);
    }

    // =========================================================================
    // Configuration Management Tests
    // =========================================================================

    #[test]
    fn test_enable_toggle() {
        let calibrator = ConfidenceCalibrator::default();

        assert!(calibrator.is_enabled());

        calibrator.enable(false);
        assert!(!calibrator.is_enabled());

        calibrator.enable(true);
        assert!(calibrator.is_enabled());
    }

    #[test]
    fn test_set_smoothing_factor() {
        let calibrator = ConfidenceCalibrator::default();

        calibrator.set_smoothing_factor(0.5);
        assert!((calibrator.get_config().smoothing_factor - 0.5).abs() < 0.001);

        // Test clamping
        calibrator.set_smoothing_factor(-0.5);
        assert!((calibrator.get_config().smoothing_factor - 0.0).abs() < 0.001);

        calibrator.set_smoothing_factor(1.5);
        assert!((calibrator.get_config().smoothing_factor - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_set_config() {
        let calibrator = ConfidenceCalibrator::default();

        let new_config = CalibrationConfig {
            enabled: false,
            num_bins: 20,
            min_samples_per_bin: 50,
            smoothing_factor: 0.8,
            window_size: 500,
        };

        calibrator.set_config(new_config);

        let config = calibrator.get_config();
        assert!(!config.enabled);
        assert_eq!(config.num_bins, 20);
        assert_eq!(config.min_samples_per_bin, 50);
    }

    #[test]
    fn test_reset() {
        let calibrator = ConfidenceCalibrator::default();

        // Add some data
        for _ in 0..50 {
            calibrator.record_outcome(0.75, true);
        }

        assert!(calibrator.get_stats().total_samples > 0);

        // Reset
        calibrator.reset();

        let stats = calibrator.get_stats();
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.calibrated_samples, 0);

        // Bins should be reset too
        let bins = calibrator.bins.read().unwrap();
        for bin in bins.iter() {
            assert_eq!(bin.total_count, 0);
        }
    }

    // =========================================================================
    // Concurrent Access Tests
    // =========================================================================

    #[test]
    fn test_concurrent_record_outcome() {
        use std::sync::Arc;
        use std::thread;

        let calibrator = Arc::new(ConfidenceCalibrator::default());
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&calibrator);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    c.record_outcome(0.5 + (i as f64 * 0.001), i % 2 == 0);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(calibrator.get_stats().total_samples, 400);
    }
}
