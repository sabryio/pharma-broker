package parsing

import (
	"testing"

	"github.com/rs/zerolog"
)

// =============================================================================
// CalibrationConfig Tests
// =============================================================================

func TestDefaultCalibrationConfig(t *testing.T) {
	cfg := DefaultCalibrationConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.NumBins != DefaultCalibrationBins {
		t.Errorf("NumBins = %d, want %d", cfg.NumBins, DefaultCalibrationBins)
	}
	if cfg.MinSamplesPerBin != DefaultMinSamplesPerBin {
		t.Errorf("MinSamplesPerBin = %d, want %d", cfg.MinSamplesPerBin, DefaultMinSamplesPerBin)
	}
	if cfg.WindowSize != DefaultCalibrationWindow {
		t.Errorf("WindowSize = %d, want %d", cfg.WindowSize, DefaultCalibrationWindow)
	}
}

// =============================================================================
// CalibrationBin Tests
// =============================================================================

func TestCalibrationBin_GetCalibrationError(t *testing.T) {
	bin := CalibrationBin{
		TotalCount:    100,
		MeanPredicted: 0.8,
		MeanActual:    0.7,
	}

	err := bin.GetCalibrationError()
	if err < 0.099 || err > 0.101 {
		t.Errorf("GetCalibrationError() = %f, want ~0.1", err)
	}

	// Empty bin
	emptyBin := CalibrationBin{}
	if emptyBin.GetCalibrationError() != 0 {
		t.Error("Empty bin should have 0 error")
	}
}

func TestCalibrationBin_IsCalibrated(t *testing.T) {
	bin := CalibrationBin{TotalCount: 25}

	if !bin.IsCalibrated(20) {
		t.Error("Bin with 25 samples should be calibrated with min 20")
	}
	if bin.IsCalibrated(30) {
		t.Error("Bin with 25 samples should not be calibrated with min 30")
	}
}

// =============================================================================
// NewConfidenceCalibrator Tests
// =============================================================================

func TestNewConfidenceCalibrator(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	if len(cc.bins) != DefaultCalibrationBins {
		t.Errorf("len(bins) = %d, want %d", len(cc.bins), DefaultCalibrationBins)
	}

	// Check bin boundaries with tolerance for floating-point
	const epsilon = 0.0001
	for i, bin := range cc.bins {
		expectedLower := float64(i) / float64(DefaultCalibrationBins)
		expectedUpper := float64(i+1) / float64(DefaultCalibrationBins)

		if abs(bin.LowerBound-expectedLower) > epsilon {
			t.Errorf("Bin %d LowerBound = %f, want %f", i, bin.LowerBound, expectedLower)
		}
		if abs(bin.UpperBound-expectedUpper) > epsilon {
			t.Errorf("Bin %d UpperBound = %f, want %f", i, bin.UpperBound, expectedUpper)
		}
	}
}

// abs returns the absolute value of a float64
func abs(x float64) float64 {
	if x < 0 {
		return -x
	}
	return x
}

func TestNewConfidenceCalibrator_DefaultValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := CalibrationConfig{} // All zero values
	cc := NewConfidenceCalibrator(cfg, log)

	if len(cc.bins) != DefaultCalibrationBins {
		t.Errorf("Should use default bins, got %d", len(cc.bins))
	}
}

// =============================================================================
// Calibrate Tests
// =============================================================================

func TestConfidenceCalibrator_Calibrate_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultCalibrationConfig()
	cfg.Enabled = false
	cc := NewConfidenceCalibrator(cfg, log)

	// Should return raw confidence when disabled
	result := cc.Calibrate(0.8)
	if result != 0.8 {
		t.Errorf("Calibrate(0.8) with disabled = %f, want 0.8", result)
	}
}

func TestConfidenceCalibrator_Calibrate_NoData(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Should return raw confidence when no calibration data
	result := cc.Calibrate(0.8)
	if result != 0.8 {
		t.Errorf("Calibrate(0.8) with no data = %f, want 0.8", result)
	}
}

func TestConfidenceCalibrator_Calibrate_WithData(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultCalibrationConfig()
	cfg.MinSamplesPerBin = 5  // Lower threshold for testing
	cfg.SmoothingFactor = 1.0 // Full calibration
	cc := NewConfidenceCalibrator(cfg, log)

	// Record outcomes for the 0.8-0.9 bin
	// Simulate overconfident predictions (predicted 0.85, actual 60% positive)
	for i := range 10 {
		cc.RecordOutcome(0.85, i < 6) // 6 positive, 4 negative
	}

	// Calibrated confidence should be lower than raw
	result := cc.Calibrate(0.85)
	if result >= 0.85 {
		t.Errorf("Calibrate(0.85) should be < 0.85 for overconfident bin, got %f", result)
	}
}

// =============================================================================
// RecordOutcome Tests
// =============================================================================

func TestConfidenceCalibrator_RecordOutcome(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record some outcomes
	cc.RecordOutcome(0.85, true)
	cc.RecordOutcome(0.85, false)
	cc.RecordOutcome(0.25, true) // Underconfident

	stats := cc.GetStats()
	if stats["total_samples"] != 3 {
		t.Errorf("total_samples = %d, want 3", stats["total_samples"])
	}
}

func TestConfidenceCalibrator_RecordOutcome_OverconfidentTracking(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record overconfident prediction (high confidence, negative outcome)
	cc.RecordOutcome(0.85, false)

	stats := cc.GetStats()
	if stats["overconfident_hits"] != 1 {
		t.Errorf("overconfident_hits = %d, want 1", stats["overconfident_hits"])
	}
}

func TestConfidenceCalibrator_RecordOutcome_UnderconfidentTracking(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record underconfident prediction (low confidence, positive outcome)
	cc.RecordOutcome(0.35, true)

	stats := cc.GetStats()
	if stats["underconfident_hits"] != 1 {
		t.Errorf("underconfident_hits = %d, want 1", stats["underconfident_hits"])
	}
}

func TestConfidenceCalibrator_RecordOutcome_BinUpdate(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record outcomes in specific bin (0.5-0.6)
	cc.RecordOutcome(0.55, true)
	cc.RecordOutcome(0.55, true)
	cc.RecordOutcome(0.55, false)

	// Check bin statistics
	binIdx := cc.getBinIndex(0.55)
	bin := cc.bins[binIdx]

	if bin.TotalCount != 3 {
		t.Errorf("bin.TotalCount = %d, want 3", bin.TotalCount)
	}
	if bin.PositiveCount != 2 {
		t.Errorf("bin.PositiveCount = %d, want 2", bin.PositiveCount)
	}
}

// =============================================================================
// GetExpectedCalibrationError Tests
// =============================================================================

func TestConfidenceCalibrator_GetExpectedCalibrationError_NoData(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	ece := cc.GetExpectedCalibrationError()
	if ece != 0 {
		t.Errorf("ECE with no data = %f, want 0", ece)
	}
}

func TestConfidenceCalibrator_GetExpectedCalibrationError_PerfectCalibration(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record perfectly calibrated outcomes (80% confidence, 80% positive)
	for i := range 10 {
		cc.RecordOutcome(0.85, i < 8) // 8 positive, 2 negative
	}

	ece := cc.GetExpectedCalibrationError()
	// ECE should be low for well-calibrated predictions
	if ece > 0.1 {
		t.Errorf("ECE for well-calibrated = %f, want < 0.1", ece)
	}
}

func TestConfidenceCalibrator_GetExpectedCalibrationError_PoorCalibration(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record poorly calibrated outcomes (90% confidence, 20% positive)
	for i := range 10 {
		cc.RecordOutcome(0.95, i < 2) // 2 positive, 8 negative
	}

	ece := cc.GetExpectedCalibrationError()
	// ECE should be high for poorly calibrated predictions
	if ece < 0.5 {
		t.Errorf("ECE for poorly calibrated = %f, want > 0.5", ece)
	}
}

// =============================================================================
// GetCalibrationReport Tests
// =============================================================================

func TestConfidenceCalibrator_GetCalibrationReport(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record some outcomes
	for i := range 20 {
		cc.RecordOutcome(0.85, i < 12) // 60% positive
		cc.RecordOutcome(0.35, i < 14) // 70% positive (underconfident)
	}

	report := cc.GetCalibrationReport()

	if report.TotalSamples != 40 {
		t.Errorf("TotalSamples = %d, want 40", report.TotalSamples)
	}
	if len(report.BinReports) != DefaultCalibrationBins {
		t.Errorf("len(BinReports) = %d, want %d", len(report.BinReports), DefaultCalibrationBins)
	}
}

func TestConfidenceCalibrator_GetCalibrationReport_OverUnderConfident(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record overconfident predictions
	for range 5 {
		cc.RecordOutcome(0.85, false)
	}
	// Record underconfident predictions
	for range 5 {
		cc.RecordOutcome(0.35, true)
	}

	report := cc.GetCalibrationReport()

	if report.OverconfidentPct != 50 {
		t.Errorf("OverconfidentPct = %f, want 50", report.OverconfidentPct)
	}
	if report.UnderconfidentPct != 50 {
		t.Errorf("UnderconfidentPct = %f, want 50", report.UnderconfidentPct)
	}
}

func TestCalibrationReport_IsWellCalibrated(t *testing.T) {
	report := CalibrationReport{ECE: 0.03}
	if !report.IsWellCalibrated(0.05) {
		t.Error("Report with ECE 0.03 should be well calibrated with threshold 0.05")
	}

	report.ECE = 0.08
	if report.IsWellCalibrated(0.05) {
		t.Error("Report with ECE 0.08 should not be well calibrated with threshold 0.05")
	}
}

// =============================================================================
// Configuration Methods Tests
// =============================================================================

func TestConfidenceCalibrator_Enable(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	cc.Enable(false)
	if cc.GetConfig().Enabled {
		t.Error("Enabled should be false after Enable(false)")
	}

	cc.Enable(true)
	if !cc.GetConfig().Enabled {
		t.Error("Enabled should be true after Enable(true)")
	}
}

func TestConfidenceCalibrator_SetSmoothingFactor(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	cc.SetSmoothingFactor(0.5)
	if cc.GetConfig().SmoothingFactor != 0.5 {
		t.Errorf("SmoothingFactor = %f, want 0.5", cc.GetConfig().SmoothingFactor)
	}

	// Test clamping
	cc.SetSmoothingFactor(-0.5)
	if cc.GetConfig().SmoothingFactor != 0 {
		t.Errorf("SmoothingFactor should be clamped to 0, got %f", cc.GetConfig().SmoothingFactor)
	}

	cc.SetSmoothingFactor(1.5)
	if cc.GetConfig().SmoothingFactor != 1 {
		t.Errorf("SmoothingFactor should be clamped to 1, got %f", cc.GetConfig().SmoothingFactor)
	}
}

func TestConfidenceCalibrator_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	newCfg := CalibrationConfig{
		Enabled:          false,
		NumBins:          20,
		MinSamplesPerBin: 50,
		SmoothingFactor:  0.8,
		WindowSize:       500,
	}
	cc.SetConfig(newCfg)

	cfg := cc.GetConfig()
	if cfg.Enabled != false {
		t.Error("Enabled should be false")
	}
	if cfg.SmoothingFactor != 0.8 {
		t.Errorf("SmoothingFactor = %f, want 0.8", cfg.SmoothingFactor)
	}
}

func TestConfidenceCalibrator_Reset(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Record some data
	for range 10 {
		cc.RecordOutcome(0.85, true)
	}

	// Verify data exists
	stats := cc.GetStats()
	if stats["total_samples"] != 10 {
		t.Errorf("total_samples before reset = %d, want 10", stats["total_samples"])
	}

	// Reset
	cc.Reset()

	// Verify data cleared
	stats = cc.GetStats()
	if stats["total_samples"] != 0 {
		t.Errorf("total_samples after reset = %d, want 0", stats["total_samples"])
	}
}

// =============================================================================
// Edge Cases Tests
// =============================================================================

func TestConfidenceCalibrator_getBinIndex_EdgeCases(t *testing.T) {
	log := zerolog.Nop()
	cc := NewConfidenceCalibrator(DefaultCalibrationConfig(), log)

	// Test boundary cases
	if cc.getBinIndex(-0.1) != 0 {
		t.Error("Negative confidence should map to bin 0")
	}
	if cc.getBinIndex(1.0) != DefaultCalibrationBins-1 {
		t.Error("Confidence 1.0 should map to last bin")
	}
	if cc.getBinIndex(1.5) != DefaultCalibrationBins-1 {
		t.Error("Confidence > 1 should map to last bin")
	}
}

func TestConfidenceCalibrator_Calibrate_Smoothing(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultCalibrationConfig()
	cfg.MinSamplesPerBin = 5
	cfg.SmoothingFactor = 0.5 // 50% calibration, 50% raw
	cc := NewConfidenceCalibrator(cfg, log)

	// Record outcomes (predicted 0.85, actual 60% positive)
	for i := range 10 {
		cc.RecordOutcome(0.85, i < 6)
	}

	result := cc.Calibrate(0.85)
	// With 50% smoothing, result should be between raw (0.85) and actual (0.6)
	if result <= 0.6 || result >= 0.85 {
		t.Errorf("Calibrate with 50%% smoothing = %f, want between 0.6 and 0.85", result)
	}
}
