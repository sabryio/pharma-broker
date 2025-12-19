package parsing

import (
	"math"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// Confidence Calibration Configuration
// =============================================================================

// CalibrationConfig holds configuration for confidence calibration.
type CalibrationConfig struct {
	// Enable calibration
	Enabled bool

	// Number of bins for calibration (default: 10)
	NumBins int

	// Minimum samples per bin before calibration applies (default: 20)
	MinSamplesPerBin int

	// Smoothing factor for Platt scaling (default: 1.0)
	SmoothingFactor float64

	// Window size for recent calibration (default: 1000)
	WindowSize int
}

// DefaultCalibrationConfig returns sensible defaults.
func DefaultCalibrationConfig() CalibrationConfig {
	return CalibrationConfig{
		Enabled:          true,
		NumBins:          DefaultCalibrationBins,
		MinSamplesPerBin: DefaultMinSamplesPerBin,
		SmoothingFactor:  1.0,
		WindowSize:       DefaultCalibrationWindow,
	}
}

// =============================================================================
// Calibration Statistics
// =============================================================================

// CalibrationStats tracks calibration statistics.
type CalibrationStats struct {
	TotalSamples       atomic.Int64
	CalibratedSamples  atomic.Int64
	OverconfidentHits  atomic.Int64 // Predicted high, actual low
	UnderconfidentHits atomic.Int64 // Predicted low, actual high
}

// GetStats returns a snapshot of calibration statistics.
func (s *CalibrationStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_samples":       s.TotalSamples.Load(),
		"calibrated_samples":  s.CalibratedSamples.Load(),
		"overconfident_hits":  s.OverconfidentHits.Load(),
		"underconfident_hits": s.UnderconfidentHits.Load(),
	}
}

// =============================================================================
// Calibration Bin
// =============================================================================

// CalibrationBin holds statistics for a confidence range.
type CalibrationBin struct {
	LowerBound    float64 // Lower bound of bin (inclusive)
	UpperBound    float64 // Upper bound of bin (exclusive)
	TotalCount    int64   // Total predictions in this bin
	PositiveCount int64   // Actual positive outcomes
	MeanPredicted float64 // Mean predicted confidence
	MeanActual    float64 // Mean actual outcome rate
}

// GetCalibrationError returns the calibration error for this bin.
func (b *CalibrationBin) GetCalibrationError() float64 {
	if b.TotalCount == 0 {
		return 0
	}
	return math.Abs(b.MeanPredicted - b.MeanActual)
}

// IsCalibrated returns true if the bin has enough samples.
func (b *CalibrationBin) IsCalibrated(minSamples int) bool {
	return b.TotalCount >= int64(minSamples)
}

// =============================================================================
// Confidence Calibrator
// =============================================================================

// ConfidenceCalibrator calibrates AI confidence scores based on actual outcomes.
type ConfidenceCalibrator struct {
	config CalibrationConfig
	stats  CalibrationStats
	bins   []CalibrationBin
	log    zerolog.Logger
	mu     sync.RWMutex

	// Recent outcomes for windowed calibration
	recentOutcomes []outcomeRecord
	recentIdx      int
}

// outcomeRecord stores a single prediction-outcome pair.
type outcomeRecord struct {
	Predicted float64
	Actual    bool // true = positive outcome (correct match)
	Timestamp time.Time
}

// NewConfidenceCalibrator creates a new confidence calibrator.
func NewConfidenceCalibrator(cfg CalibrationConfig, log zerolog.Logger) *ConfidenceCalibrator {
	if cfg.NumBins <= 0 {
		cfg.NumBins = DefaultCalibrationBins
	}
	if cfg.MinSamplesPerBin <= 0 {
		cfg.MinSamplesPerBin = DefaultMinSamplesPerBin
	}
	if cfg.WindowSize <= 0 {
		cfg.WindowSize = DefaultCalibrationWindow
	}

	// Initialize bins
	bins := make([]CalibrationBin, cfg.NumBins)
	binWidth := 1.0 / float64(cfg.NumBins)
	for i := range bins {
		bins[i] = CalibrationBin{
			LowerBound: float64(i) * binWidth,
			UpperBound: float64(i+1) * binWidth,
		}
	}

	return &ConfidenceCalibrator{
		config:         cfg,
		bins:           bins,
		log:            log.With().Str("component", "calibrator").Logger(),
		recentOutcomes: make([]outcomeRecord, cfg.WindowSize),
	}
}

// Calibrate adjusts a raw confidence score based on historical calibration.
func (cc *ConfidenceCalibrator) Calibrate(rawConfidence float64) float64 {
	if !cc.config.Enabled {
		return rawConfidence
	}

	cc.mu.RLock()
	defer cc.mu.RUnlock()

	// Find the bin for this confidence
	binIdx := cc.getBinIndex(rawConfidence)
	bin := cc.bins[binIdx]

	// If not enough samples, return raw confidence
	if !bin.IsCalibrated(cc.config.MinSamplesPerBin) {
		return rawConfidence
	}

	// Apply isotonic regression-style calibration
	// Map predicted confidence to actual outcome rate
	calibrated := cc.interpolateCalibration(rawConfidence)

	cc.stats.CalibratedSamples.Add(1)

	return calibrated
}

// RecordOutcome records a prediction-outcome pair for calibration.
func (cc *ConfidenceCalibrator) RecordOutcome(predictedConfidence float64, actualPositive bool) {
	cc.mu.Lock()
	defer cc.mu.Unlock()

	cc.stats.TotalSamples.Add(1)

	// Update bin statistics
	binIdx := cc.getBinIndex(predictedConfidence)
	bin := &cc.bins[binIdx]

	bin.TotalCount++
	if actualPositive {
		bin.PositiveCount++
	}

	// Update running means
	bin.MeanPredicted = (bin.MeanPredicted*float64(bin.TotalCount-1) + predictedConfidence) / float64(bin.TotalCount)
	actualVal := 0.0
	if actualPositive {
		actualVal = 1.0
	}
	bin.MeanActual = (bin.MeanActual*float64(bin.TotalCount-1) + actualVal) / float64(bin.TotalCount)

	// Track over/under confidence
	if predictedConfidence >= 0.7 && !actualPositive {
		cc.stats.OverconfidentHits.Add(1)
	} else if predictedConfidence < 0.5 && actualPositive {
		cc.stats.UnderconfidentHits.Add(1)
	}

	// Store in recent window
	cc.recentOutcomes[cc.recentIdx] = outcomeRecord{
		Predicted: predictedConfidence,
		Actual:    actualPositive,
		Timestamp: time.Now(),
	}
	cc.recentIdx = (cc.recentIdx + 1) % cc.config.WindowSize

	// Log calibration updates periodically
	if cc.stats.TotalSamples.Load()%100 == 0 {
		cc.logCalibrationStatus()
	}
}

// getBinIndex returns the bin index for a confidence value.
func (cc *ConfidenceCalibrator) getBinIndex(confidence float64) int {
	if confidence < 0 {
		confidence = 0
	}
	if confidence >= 1 {
		return len(cc.bins) - 1
	}
	binWidth := 1.0 / float64(len(cc.bins))
	return int(confidence / binWidth)
}

// interpolateCalibration performs linear interpolation between bins.
func (cc *ConfidenceCalibrator) interpolateCalibration(rawConfidence float64) float64 {
	binIdx := cc.getBinIndex(rawConfidence)
	bin := cc.bins[binIdx]

	// If bin has no data, return raw
	if bin.TotalCount == 0 {
		return rawConfidence
	}

	// Simple calibration: map to actual outcome rate
	// With smoothing to avoid extreme adjustments
	calibrated := bin.MeanActual

	// Apply smoothing factor to blend with raw confidence
	smoothed := cc.config.SmoothingFactor*calibrated + (1-cc.config.SmoothingFactor)*rawConfidence

	// Clamp to valid range
	if smoothed < 0 {
		smoothed = 0
	}
	if smoothed > 1 {
		smoothed = 1
	}

	return smoothed
}

// logCalibrationStatus logs current calibration status.
func (cc *ConfidenceCalibrator) logCalibrationStatus() {
	ece := cc.GetExpectedCalibrationError()
	cc.log.Info().
		Float64("ece", ece).
		Int64("total_samples", cc.stats.TotalSamples.Load()).
		Int64("overconfident", cc.stats.OverconfidentHits.Load()).
		Int64("underconfident", cc.stats.UnderconfidentHits.Load()).
		Msg("📊 Calibration status update")
}

// =============================================================================
// Calibration Metrics
// =============================================================================

// GetExpectedCalibrationError calculates the Expected Calibration Error (ECE).
// ECE is a standard metric for measuring calibration quality.
func (cc *ConfidenceCalibrator) GetExpectedCalibrationError() float64 {
	cc.mu.RLock()
	defer cc.mu.RUnlock()

	var totalSamples int64
	var weightedError float64

	for _, bin := range cc.bins {
		if bin.TotalCount > 0 {
			totalSamples += bin.TotalCount
			weightedError += float64(bin.TotalCount) * bin.GetCalibrationError()
		}
	}

	if totalSamples == 0 {
		return 0
	}

	return weightedError / float64(totalSamples)
}

// GetMaxCalibrationError returns the Maximum Calibration Error (MCE).
func (cc *ConfidenceCalibrator) GetMaxCalibrationError() float64 {
	cc.mu.RLock()
	defer cc.mu.RUnlock()

	var maxError float64
	for _, bin := range cc.bins {
		if bin.TotalCount > 0 {
			err := bin.GetCalibrationError()
			if err > maxError {
				maxError = err
			}
		}
	}

	return maxError
}

// GetCalibrationReport returns a detailed calibration report.
func (cc *ConfidenceCalibrator) GetCalibrationReport() CalibrationReport {
	cc.mu.RLock()
	defer cc.mu.RUnlock()

	report := CalibrationReport{
		ECE:               cc.GetExpectedCalibrationError(),
		MCE:               cc.GetMaxCalibrationError(),
		TotalSamples:      cc.stats.TotalSamples.Load(),
		OverconfidentPct:  0,
		UnderconfidentPct: 0,
		BinReports:        make([]BinReport, len(cc.bins)),
	}

	if report.TotalSamples > 0 {
		report.OverconfidentPct = float64(cc.stats.OverconfidentHits.Load()) / float64(report.TotalSamples) * 100
		report.UnderconfidentPct = float64(cc.stats.UnderconfidentHits.Load()) / float64(report.TotalSamples) * 100
	}

	for i, bin := range cc.bins {
		report.BinReports[i] = BinReport{
			Range:         [2]float64{bin.LowerBound, bin.UpperBound},
			Count:         bin.TotalCount,
			MeanPredicted: bin.MeanPredicted,
			MeanActual:    bin.MeanActual,
			Error:         bin.GetCalibrationError(),
			IsCalibrated:  bin.IsCalibrated(cc.config.MinSamplesPerBin),
		}
	}

	return report
}

// CalibrationReport contains a detailed calibration analysis.
type CalibrationReport struct {
	ECE               float64 // Expected Calibration Error
	MCE               float64 // Maximum Calibration Error
	TotalSamples      int64
	OverconfidentPct  float64 // Percentage of overconfident predictions
	UnderconfidentPct float64 // Percentage of underconfident predictions
	BinReports        []BinReport
}

// BinReport contains statistics for a single calibration bin.
type BinReport struct {
	Range         [2]float64 // [lower, upper)
	Count         int64
	MeanPredicted float64
	MeanActual    float64
	Error         float64
	IsCalibrated  bool
}

// IsWellCalibrated returns true if ECE is below threshold.
func (r *CalibrationReport) IsWellCalibrated(threshold float64) bool {
	return r.ECE < threshold
}

// =============================================================================
// Configuration Methods
// =============================================================================

// GetConfig returns the current configuration.
func (cc *ConfidenceCalibrator) GetConfig() CalibrationConfig {
	cc.mu.RLock()
	defer cc.mu.RUnlock()
	return cc.config
}

// SetConfig updates the configuration.
func (cc *ConfidenceCalibrator) SetConfig(cfg CalibrationConfig) {
	cc.mu.Lock()
	defer cc.mu.Unlock()
	cc.config = cfg
	cc.log.Info().
		Bool("enabled", cfg.Enabled).
		Int("num_bins", cfg.NumBins).
		Float64("smoothing", cfg.SmoothingFactor).
		Msg("Calibration configuration updated")
}

// Enable enables or disables calibration.
func (cc *ConfidenceCalibrator) Enable(enabled bool) {
	cc.mu.Lock()
	cc.config.Enabled = enabled
	cc.mu.Unlock()
	cc.log.Info().
		Bool("enabled", enabled).
		Msg("Calibration toggled")
}

// SetSmoothingFactor sets the smoothing factor for calibration.
func (cc *ConfidenceCalibrator) SetSmoothingFactor(factor float64) {
	if factor < 0 {
		factor = 0
	}
	if factor > 1 {
		factor = 1
	}
	cc.mu.Lock()
	cc.config.SmoothingFactor = factor
	cc.mu.Unlock()
	cc.log.Info().
		Float64("smoothing_factor", factor).
		Msg("Smoothing factor updated")
}

// GetStats returns the current calibration statistics.
func (cc *ConfidenceCalibrator) GetStats() map[string]int64 {
	return cc.stats.GetStats()
}

// Reset clears all calibration data.
func (cc *ConfidenceCalibrator) Reset() {
	cc.mu.Lock()
	defer cc.mu.Unlock()

	// Reset bins
	binWidth := 1.0 / float64(len(cc.bins))
	for i := range cc.bins {
		cc.bins[i] = CalibrationBin{
			LowerBound: float64(i) * binWidth,
			UpperBound: float64(i+1) * binWidth,
		}
	}

	// Reset stats
	cc.stats = CalibrationStats{}

	// Reset recent outcomes
	cc.recentOutcomes = make([]outcomeRecord, cc.config.WindowSize)
	cc.recentIdx = 0

	cc.log.Info().Msg("Calibration data reset")
}
