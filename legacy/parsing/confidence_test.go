package parsing

import (
	"testing"

	"github.com/rs/zerolog"
)

// =============================================================================
// ConfidenceConfig Tests
// =============================================================================

func TestDefaultConfidenceConfig(t *testing.T) {
	cfg := DefaultConfidenceConfig()

	if cfg.BaseStrictThreshold != DefaultStrictConfidence {
		t.Errorf("BaseStrictThreshold = %f, want %f", cfg.BaseStrictThreshold, DefaultStrictConfidence)
	}
	if cfg.BaseRelaxedThreshold != DefaultRelaxedConfidence {
		t.Errorf("BaseRelaxedThreshold = %f, want %f", cfg.BaseRelaxedThreshold, DefaultRelaxedConfidence)
	}
	if cfg.EnableAdaptive != false {
		t.Error("EnableAdaptive should be false by default")
	}
	if cfg.AdjustmentStep != DefaultConfidenceAdjustmentStep {
		t.Errorf("AdjustmentStep = %f, want %f", cfg.AdjustmentStep, DefaultConfidenceAdjustmentStep)
	}
}

// =============================================================================
// ConfidenceStats Tests
// =============================================================================

func TestConfidenceStats_GetStats(t *testing.T) {
	stats := &ConfidenceStats{}
	stats.TotalEvaluations.Store(100)
	stats.AcceptedItems.Store(85)
	stats.RejectedItems.Store(15)
	stats.ThresholdAdjustments.Store(2)

	result := stats.GetStats()

	if result["total_evaluations"] != 100 {
		t.Errorf("total_evaluations = %d, want 100", result["total_evaluations"])
	}
	if result["accepted_items"] != 85 {
		t.Errorf("accepted_items = %d, want 85", result["accepted_items"])
	}
}

func TestConfidenceStats_GetAcceptanceRate(t *testing.T) {
	stats := &ConfidenceStats{}

	// Empty stats
	if rate := stats.GetAcceptanceRate(); rate != 1.0 {
		t.Errorf("GetAcceptanceRate() with no data = %f, want 1.0", rate)
	}

	// With data
	stats.TotalEvaluations.Store(100)
	stats.AcceptedItems.Store(80)

	if rate := stats.GetAcceptanceRate(); rate != 0.8 {
		t.Errorf("GetAcceptanceRate() = %f, want 0.8", rate)
	}
}

func TestConfidenceStats_GetAverageConfidence(t *testing.T) {
	stats := &ConfidenceStats{}

	// Empty stats
	if avg := stats.GetAverageConfidence(); avg != 0.0 {
		t.Errorf("GetAverageConfidence() with no data = %f, want 0.0", avg)
	}

	// With data (sum stored as confidence * 1000)
	stats.TotalEvaluations.Store(2)
	stats.AvgConfidenceSum.Store(1500) // 0.75 * 1000 * 2

	if avg := stats.GetAverageConfidence(); avg != 0.75 {
		t.Errorf("GetAverageConfidence() = %f, want 0.75", avg)
	}
}

// =============================================================================
// NewConfidenceManager Tests
// =============================================================================

func TestNewConfidenceManager_DefaultValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := ConfidenceConfig{} // All zero values

	cm := NewConfidenceManager(cfg, log)

	if cm.currentStrict != DefaultStrictConfidence {
		t.Errorf("currentStrict = %f, want %f", cm.currentStrict, DefaultStrictConfidence)
	}
	if cm.currentRelaxed != DefaultRelaxedConfidence {
		t.Errorf("currentRelaxed = %f, want %f", cm.currentRelaxed, DefaultRelaxedConfidence)
	}
}

func TestNewConfidenceManager_CustomConfig(t *testing.T) {
	log := zerolog.Nop()
	cfg := ConfidenceConfig{
		BaseStrictThreshold:  0.8,
		BaseRelaxedThreshold: 0.5,
		EnableAdaptive:       true,
	}

	cm := NewConfidenceManager(cfg, log)

	if cm.currentStrict != 0.8 {
		t.Errorf("currentStrict = %f, want 0.8", cm.currentStrict)
	}
	if cm.currentRelaxed != 0.5 {
		t.Errorf("currentRelaxed = %f, want 0.5", cm.currentRelaxed)
	}
}

// =============================================================================
// Threshold Getter/Setter Tests
// =============================================================================

func TestConfidenceManager_GetSetStrictThreshold(t *testing.T) {
	log := zerolog.Nop()
	cm := NewConfidenceManager(DefaultConfidenceConfig(), log)

	// Get initial
	if threshold := cm.GetStrictThreshold(); threshold != DefaultStrictConfidence {
		t.Errorf("GetStrictThreshold() = %f, want %f", threshold, DefaultStrictConfidence)
	}

	// Set new value
	cm.SetStrictThreshold(0.85)
	if threshold := cm.GetStrictThreshold(); threshold != 0.85 {
		t.Errorf("After SetStrictThreshold(0.85), got %f", threshold)
	}

	// Test clamping to min
	cm.SetStrictThreshold(0.1)
	if threshold := cm.GetStrictThreshold(); threshold != DefaultMinConfidenceThreshold {
		t.Errorf("Should clamp to min, got %f", threshold)
	}

	// Test clamping to max
	cm.SetStrictThreshold(0.99)
	if threshold := cm.GetStrictThreshold(); threshold != DefaultMaxConfidenceThreshold {
		t.Errorf("Should clamp to max, got %f", threshold)
	}
}

func TestConfidenceManager_GetSetRelaxedThreshold(t *testing.T) {
	log := zerolog.Nop()
	cm := NewConfidenceManager(DefaultConfidenceConfig(), log)

	cm.SetRelaxedThreshold(0.5)
	if threshold := cm.GetRelaxedThreshold(); threshold != 0.5 {
		t.Errorf("After SetRelaxedThreshold(0.5), got %f", threshold)
	}
}

// =============================================================================
// EvaluateConfidence Tests
// =============================================================================

func TestConfidenceManager_EvaluateConfidence(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultConfidenceConfig()
	cfg.EnableAdaptive = false // Disable adaptive for predictable testing
	cm := NewConfidenceManager(cfg, log)

	tests := []struct {
		confidence float64
		expected   bool
	}{
		{0.9, true},  // Above strict threshold
		{0.7, true},  // At strict threshold
		{0.6, false}, // Below strict threshold
		{0.3, false}, // Well below
	}

	for _, tt := range tests {
		result := cm.EvaluateConfidence(tt.confidence)
		if result != tt.expected {
			t.Errorf("EvaluateConfidence(%f) = %v, want %v", tt.confidence, result, tt.expected)
		}
	}

	// Check stats were updated
	stats := cm.GetStats()
	if stats["total_evaluations"].(int64) != 4 {
		t.Errorf("total_evaluations = %d, want 4", stats["total_evaluations"])
	}
}

func TestConfidenceManager_EvaluateRelaxed(t *testing.T) {
	log := zerolog.Nop()
	cm := NewConfidenceManager(DefaultConfidenceConfig(), log)

	tests := []struct {
		confidence float64
		expected   bool
	}{
		{0.5, true},  // Above relaxed threshold
		{0.4, true},  // At relaxed threshold
		{0.3, false}, // Below relaxed threshold
	}

	for _, tt := range tests {
		result := cm.EvaluateRelaxed(tt.confidence)
		if result != tt.expected {
			t.Errorf("EvaluateRelaxed(%f) = %v, want %v", tt.confidence, result, tt.expected)
		}
	}
}

// =============================================================================
// Adaptive Threshold Tests
// =============================================================================

func TestConfidenceManager_AdaptiveAdjustment_LowerThreshold(t *testing.T) {
	log := zerolog.Nop()
	cfg := ConfidenceConfig{
		BaseStrictThreshold:  0.7,
		BaseRelaxedThreshold: 0.4,
		EnableAdaptive:       true,
		AdjustmentStep:       0.05,
		MinThreshold:         0.3,
		MaxThreshold:         0.95,
		EvaluationWindow:     10, // Small window for testing
		TargetAcceptRate:     0.8,
		AcceptRateTolerance:  0.05,
	}
	cm := NewConfidenceManager(cfg, log)

	initialThreshold := cm.GetStrictThreshold()

	// Simulate many rejections (low acceptance rate)
	for i := 0; i < 10; i++ {
		cm.EvaluateConfidence(0.5) // All below threshold
	}

	// Threshold should have been lowered
	newThreshold := cm.GetStrictThreshold()
	if newThreshold >= initialThreshold {
		t.Errorf("Threshold should be lowered after many rejections, got %f (was %f)", newThreshold, initialThreshold)
	}
}

func TestConfidenceManager_AdaptiveAdjustment_RaiseThreshold(t *testing.T) {
	log := zerolog.Nop()
	cfg := ConfidenceConfig{
		BaseStrictThreshold:  0.5, // Low starting threshold
		BaseRelaxedThreshold: 0.3,
		EnableAdaptive:       true,
		AdjustmentStep:       0.05,
		MinThreshold:         0.3,
		MaxThreshold:         0.95,
		EvaluationWindow:     10,
		TargetAcceptRate:     0.8,
		AcceptRateTolerance:  0.05,
	}
	cm := NewConfidenceManager(cfg, log)

	initialThreshold := cm.GetStrictThreshold()

	// Simulate many acceptances (high acceptance rate > 85%)
	for i := 0; i < 10; i++ {
		cm.EvaluateConfidence(0.9) // All above threshold
	}

	// Threshold should have been raised
	newThreshold := cm.GetStrictThreshold()
	if newThreshold <= initialThreshold {
		t.Errorf("Threshold should be raised after many acceptances, got %f (was %f)", newThreshold, initialThreshold)
	}
}

// =============================================================================
// EnableAdaptive and ResetToBase Tests
// =============================================================================

func TestConfidenceManager_EnableAdaptive(t *testing.T) {
	log := zerolog.Nop()
	cm := NewConfidenceManager(DefaultConfidenceConfig(), log)

	cm.EnableAdaptive(true)
	if !cm.config.EnableAdaptive {
		t.Error("EnableAdaptive(true) should enable adaptive mode")
	}

	cm.EnableAdaptive(false)
	if cm.config.EnableAdaptive {
		t.Error("EnableAdaptive(false) should disable adaptive mode")
	}
}

func TestConfidenceManager_ResetToBase(t *testing.T) {
	log := zerolog.Nop()
	cm := NewConfidenceManager(DefaultConfidenceConfig(), log)

	// Modify thresholds
	cm.SetStrictThreshold(0.9)
	cm.SetRelaxedThreshold(0.6)

	// Reset
	cm.ResetToBase()

	if cm.GetStrictThreshold() != DefaultStrictConfidence {
		t.Errorf("After reset, strict = %f, want %f", cm.GetStrictThreshold(), DefaultStrictConfidence)
	}
	if cm.GetRelaxedThreshold() != DefaultRelaxedConfidence {
		t.Errorf("After reset, relaxed = %f, want %f", cm.GetRelaxedThreshold(), DefaultRelaxedConfidence)
	}
}

// =============================================================================
// Confidence Constants Tests
// =============================================================================

func TestConfidenceConstants(t *testing.T) {
	if DefaultStrictConfidence <= 0 || DefaultStrictConfidence > 1 {
		t.Error("DefaultStrictConfidence should be between 0 and 1")
	}
	if DefaultRelaxedConfidence <= 0 || DefaultRelaxedConfidence > 1 {
		t.Error("DefaultRelaxedConfidence should be between 0 and 1")
	}
	if DefaultRelaxedConfidence >= DefaultStrictConfidence {
		t.Error("DefaultRelaxedConfidence should be less than DefaultStrictConfidence")
	}
	if DefaultMinConfidenceThreshold >= DefaultMaxConfidenceThreshold {
		t.Error("DefaultMinConfidenceThreshold should be less than DefaultMaxConfidenceThreshold")
	}
	if DefaultTargetAcceptRate <= 0 || DefaultTargetAcceptRate > 1 {
		t.Error("DefaultTargetAcceptRate should be between 0 and 1")
	}
}
