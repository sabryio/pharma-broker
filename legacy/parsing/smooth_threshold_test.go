package parsing

import (
	"math"
	"testing"

	"github.com/rs/zerolog"

	"pharmabroker/matching"
)

// =============================================================================
// SmoothThresholdConfig Tests
// =============================================================================

func TestDefaultSmoothThresholdConfig(t *testing.T) {
	cfg := DefaultSmoothThresholdConfig()

	if cfg.TransitionWidth != DefaultTransitionWidth {
		t.Errorf("TransitionWidth = %f, want %f", cfg.TransitionWidth, DefaultTransitionWidth)
	}
	if !cfg.EnableSmoothing {
		t.Error("EnableSmoothing should be true by default")
	}
	if cfg.Thresholds.Auto != 0.9 {
		t.Errorf("Thresholds.Auto = %f, want 0.9", cfg.Thresholds.Auto)
	}
}

// =============================================================================
// SmoothStep Function Tests
// =============================================================================

func TestSmoothStep(t *testing.T) {
	tests := []struct {
		input    float64
		expected float64
	}{
		{0.0, 0.0},
		{1.0, 1.0},
		{0.5, 0.5}, // Midpoint should be 0.5
	}

	for _, tt := range tests {
		result := smoothStep(tt.input)
		if math.Abs(result-tt.expected) > 0.001 {
			t.Errorf("smoothStep(%f) = %f, want %f", tt.input, result, tt.expected)
		}
	}

	// Test clamping
	if smoothStep(-0.5) != 0.0 {
		t.Error("smoothStep should clamp negative values to 0")
	}
	if smoothStep(1.5) != 1.0 {
		t.Error("smoothStep should clamp values > 1 to 1")
	}

	// Test smoothness (derivative should be 0 at endpoints)
	// At t=0.1, result should be small but non-zero
	if smoothStep(0.1) <= 0 {
		t.Error("smoothStep(0.1) should be > 0")
	}
	// At t=0.9, result should be close to 1 but not quite
	if smoothStep(0.9) >= 1 {
		t.Error("smoothStep(0.9) should be < 1")
	}
}

// =============================================================================
// CalculateSmoothConfidence Tests
// =============================================================================

func TestSmoothThreshold_CalculateSmoothConfidence_AutoBand(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score well within AUTO band
	result := stc.CalculateSmoothConfidence(0.95)

	if result.PrimaryBand != matching.ConfidenceAuto {
		t.Errorf("PrimaryBand = %s, want AUTO", result.PrimaryBand)
	}
	if result.RawScore != 0.95 {
		t.Errorf("RawScore = %f, want 0.95", result.RawScore)
	}
	if result.BandStrength < 0.9 {
		t.Errorf("BandStrength = %f, should be high for score well within band", result.BandStrength)
	}
}

func TestSmoothThreshold_CalculateSmoothConfidence_TransitionZone(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score at AUTO threshold boundary (0.90)
	result := stc.CalculateSmoothConfidence(0.90)

	if result.PrimaryBand != matching.ConfidenceAuto {
		t.Errorf("PrimaryBand = %s, want AUTO", result.PrimaryBand)
	}
	if !result.InTransitionZone {
		t.Error("Should be in transition zone at boundary")
	}
	if result.NearBoundary != "lower" {
		t.Errorf("NearBoundary = %s, want lower", result.NearBoundary)
	}
}

func TestSmoothThreshold_CalculateSmoothConfidence_SuggestBand(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score in middle of SUGGEST band
	result := stc.CalculateSmoothConfidence(0.80)

	if result.PrimaryBand != matching.ConfidenceSuggest {
		t.Errorf("PrimaryBand = %s, want SUGGEST", result.PrimaryBand)
	}
	if result.InTransitionZone {
		t.Error("Should not be in transition zone in middle of band")
	}
	if result.BandStrength != 1.0 {
		t.Errorf("BandStrength = %f, want 1.0 for middle of band", result.BandStrength)
	}
}

func TestSmoothThreshold_CalculateSmoothConfidence_ReviewBand(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score in REVIEW band
	result := stc.CalculateSmoothConfidence(0.60)

	if result.PrimaryBand != matching.ConfidenceReview {
		t.Errorf("PrimaryBand = %s, want REVIEW", result.PrimaryBand)
	}
}

func TestSmoothThreshold_CalculateSmoothConfidence_NoneBand(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score in NONE band
	result := stc.CalculateSmoothConfidence(0.30)

	if result.PrimaryBand != matching.ConfidenceNone {
		t.Errorf("PrimaryBand = %s, want NONE", result.PrimaryBand)
	}
}

func TestSmoothThreshold_CalculateSmoothConfidence_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultSmoothThresholdConfig()
	cfg.EnableSmoothing = false
	stc := NewSmoothThresholdCalculator(cfg, log)

	result := stc.CalculateSmoothConfidence(0.90)

	// With smoothing disabled, should return raw values
	if result.SmoothConfidence != result.RawScore {
		t.Error("With smoothing disabled, SmoothConfidence should equal RawScore")
	}
	if result.BandStrength != 1.0 {
		t.Error("With smoothing disabled, BandStrength should be 1.0")
	}
}

// =============================================================================
// GetAdjustedAction Tests
// =============================================================================

func TestSmoothThreshold_GetAdjustedAction_StrongBand(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	// Score well within AUTO band
	result := stc.GetAdjustedAction(0.95)

	if result.PrimaryBand != matching.ConfidenceAuto {
		t.Errorf("PrimaryBand = %s, want AUTO", result.PrimaryBand)
	}
	if result.ShouldConsiderAdjacent {
		t.Error("Should not consider adjacent band for strong band membership")
	}
}

func TestSmoothThreshold_GetAdjustedAction_WeakBand(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultSmoothThresholdConfig()
	cfg.TransitionWidth = 0.2 // Wider transition for testing
	stc := NewSmoothThresholdCalculator(cfg, log)

	// Score at boundary
	result := stc.GetAdjustedAction(0.90)

	if result.PrimaryBand != matching.ConfidenceAuto {
		t.Errorf("PrimaryBand = %s, want AUTO", result.PrimaryBand)
	}
	// May or may not suggest adjacent depending on band strength
	if result.InTransitionZone && result.BandStrength < 0.5 {
		if !result.ShouldConsiderAdjacent {
			t.Error("Should consider adjacent band for weak band membership")
		}
	}
}

func TestSmoothThreshold_GetAdjustedAction_Recommendations(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultSmoothThresholdConfig()
	cfg.TransitionWidth = 0.3 // Wide transition for testing
	stc := NewSmoothThresholdCalculator(cfg, log)

	tests := []struct {
		score      float64
		expectBand matching.ConfidenceBand
	}{
		{0.95, matching.ConfidenceAuto},
		{0.80, matching.ConfidenceSuggest},
		{0.60, matching.ConfidenceReview},
		{0.30, matching.ConfidenceNone},
	}

	for _, tt := range tests {
		result := stc.GetAdjustedAction(tt.score)
		if result.PrimaryBand != tt.expectBand {
			t.Errorf("GetAdjustedAction(%f).PrimaryBand = %s, want %s",
				tt.score, result.PrimaryBand, tt.expectBand)
		}
	}
}

// =============================================================================
// Configuration Methods Tests
// =============================================================================

func TestSmoothThreshold_SetTransitionWidth(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	stc.SetTransitionWidth(0.2)
	cfg := stc.GetConfig()
	if cfg.TransitionWidth != 0.2 {
		t.Errorf("TransitionWidth = %f, want 0.2", cfg.TransitionWidth)
	}

	// Invalid values should be ignored
	stc.SetTransitionWidth(0)
	cfg = stc.GetConfig()
	if cfg.TransitionWidth != 0.2 {
		t.Error("Invalid value (0) should be ignored")
	}

	stc.SetTransitionWidth(0.6)
	cfg = stc.GetConfig()
	if cfg.TransitionWidth != 0.2 {
		t.Error("Invalid value (> 0.5) should be ignored")
	}
}

func TestSmoothThreshold_EnableSmoothing(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	stc.EnableSmoothing(false)
	cfg := stc.GetConfig()
	if cfg.EnableSmoothing {
		t.Error("EnableSmoothing should be false")
	}

	stc.EnableSmoothing(true)
	cfg = stc.GetConfig()
	if !cfg.EnableSmoothing {
		t.Error("EnableSmoothing should be true")
	}
}

func TestSmoothThreshold_SetThresholds(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	newThresholds := matching.Thresholds{
		Auto:    0.85,
		Suggest: 0.65,
		Review:  0.45,
	}
	stc.SetThresholds(newThresholds)

	cfg := stc.GetConfig()
	if cfg.Thresholds.Auto != 0.85 {
		t.Errorf("Thresholds.Auto = %f, want 0.85", cfg.Thresholds.Auto)
	}
}

// =============================================================================
// Utility Function Tests
// =============================================================================

func TestSmoothThreshold_GetBandRange(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	tests := []struct {
		band     matching.ConfidenceBand
		minScore float64
		maxScore float64
	}{
		{matching.ConfidenceAuto, 0.9, 1.0},
		{matching.ConfidenceSuggest, 0.7, 0.9},
		{matching.ConfidenceReview, 0.5, 0.7},
		{matching.ConfidenceNone, 0.0, 0.5},
	}

	for _, tt := range tests {
		min, max := stc.GetBandRange(tt.band)
		if min != tt.minScore {
			t.Errorf("GetBandRange(%s) min = %f, want %f", tt.band, min, tt.minScore)
		}
		if max != tt.maxScore {
			t.Errorf("GetBandRange(%s) max = %f, want %f", tt.band, max, tt.maxScore)
		}
	}
}

func TestSmoothThreshold_GetTransitionZone(t *testing.T) {
	log := zerolog.Nop()
	stc := NewSmoothThresholdCalculator(DefaultSmoothThresholdConfig(), log)

	lower, upper := stc.GetTransitionZone(0.9)

	// With 10% transition width, zone should be 0.9 ± 0.045
	expectedLower := 0.9 - (0.9 * 0.1 / 2)
	expectedUpper := 0.9 + (0.9 * 0.1 / 2)

	if math.Abs(lower-expectedLower) > 0.001 {
		t.Errorf("GetTransitionZone(0.9) lower = %f, want %f", lower, expectedLower)
	}
	if math.Abs(upper-expectedUpper) > 0.001 {
		t.Errorf("GetTransitionZone(0.9) upper = %f, want %f", upper, expectedUpper)
	}
}

// =============================================================================
// Smooth Threshold Constants Tests
// =============================================================================

func TestSmoothThresholdConstants(t *testing.T) {
	if DefaultTransitionWidth <= 0 || DefaultTransitionWidth > 0.5 {
		t.Error("DefaultTransitionWidth should be between 0 and 0.5")
	}
}
