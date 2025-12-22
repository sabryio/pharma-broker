package matching

import (
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

// =============================================================================
// WarmStartManager Tests
// =============================================================================

func TestDefaultWarmStartConfig(t *testing.T) {
	cfg := DefaultWarmStartConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.PriorStrength != 50 {
		t.Errorf("PriorStrength = %d, want 50", cfg.PriorStrength)
	}
	if cfg.DecayHalfLife != 14 {
		t.Errorf("DecayHalfLife = %d, want 14", cfg.DecayHalfLife)
	}
}

func TestNewWarmStartManager(t *testing.T) {
	log := zerolog.Nop()
	manager := NewWarmStartManager(DefaultWarmStartConfig(), log)

	if manager == nil {
		t.Fatal("NewWarmStartManager returned nil")
	}
}

func TestWarmStartManager_GetEffectiveWeights_InsufficientSamples(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultWarmStartConfig()
	cfg.MinSamplesForLearning = 20
	manager := NewWarmStartManager(cfg, log)

	learnedWeights := Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1}

	// With only 10 samples, should return prior weights
	result := manager.GetEffectiveWeights(learnedWeights, 10)

	if result.Medication != cfg.PriorWeights.Medication {
		t.Errorf("Should use prior weights with insufficient samples: got %f, want %f",
			result.Medication, cfg.PriorWeights.Medication)
	}
}

func TestWarmStartManager_GetEffectiveWeights_SufficientSamples(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultWarmStartConfig()
	cfg.MinSamplesForLearning = 20
	cfg.PriorStrength = 50
	manager := NewWarmStartManager(cfg, log)

	learnedWeights := Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1}

	// With 100 samples, should blend learned with prior
	result := manager.GetEffectiveWeights(learnedWeights, 100)

	// Result should be between prior and learned
	if result.Medication <= cfg.PriorWeights.Medication || result.Medication >= learnedWeights.Medication {
		t.Errorf("Blended weight should be between prior (%f) and learned (%f), got %f",
			cfg.PriorWeights.Medication, learnedWeights.Medication, result.Medication)
	}
}

func TestWarmStartManager_GetEffectiveWeights_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultWarmStartConfig()
	cfg.Enabled = false
	manager := NewWarmStartManager(cfg, log)

	learnedWeights := Weights{Medication: 0.5, Dosage: 0.2, Quantity: 0.1, Price: 0.1, Recency: 0.1}

	result := manager.GetEffectiveWeights(learnedWeights, 5)

	// Should return learned weights unchanged when disabled
	if result.Medication != learnedWeights.Medication {
		t.Errorf("Should return learned weights when disabled: got %f, want %f",
			result.Medication, learnedWeights.Medication)
	}
}

func TestWarmStartManager_GetPriorInfluence(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultWarmStartConfig()
	cfg.MinSamplesForLearning = 20
	manager := NewWarmStartManager(cfg, log)

	// With insufficient samples, prior influence should be 100%
	influence := manager.GetPriorInfluence(10)
	if influence != 100.0 {
		t.Errorf("Prior influence with insufficient samples = %f, want 100", influence)
	}

	// With more samples, prior influence should decrease
	influence = manager.GetPriorInfluence(100)
	if influence >= 100.0 || influence <= 0 {
		t.Errorf("Prior influence with 100 samples = %f, want between 0 and 100", influence)
	}
}

func TestWarmStartManager_Enable(t *testing.T) {
	log := zerolog.Nop()
	manager := NewWarmStartManager(DefaultWarmStartConfig(), log)

	manager.Enable(false)
	if manager.GetConfig().Enabled {
		t.Error("Enabled should be false after Enable(false)")
	}

	manager.Enable(true)
	if !manager.GetConfig().Enabled {
		t.Error("Enabled should be true after Enable(true)")
	}
}

func TestWarmStartManager_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	manager := NewWarmStartManager(DefaultWarmStartConfig(), log)

	newCfg := WarmStartConfig{
		PriorStrength: 100,
		DecayHalfLife: 7,
		Enabled:       true,
	}
	manager.SetConfig(newCfg)

	cfg := manager.GetConfig()
	if cfg.PriorStrength != 100 {
		t.Errorf("PriorStrength = %d, want 100", cfg.PriorStrength)
	}
}

// =============================================================================
// OutlierDetector Tests
// =============================================================================

func TestDefaultOutlierDetectorConfig(t *testing.T) {
	cfg := DefaultOutlierDetectorConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.WindowSize != 100 {
		t.Errorf("WindowSize = %d, want 100", cfg.WindowSize)
	}
	if cfg.ZScoreThreshold != 2.5 {
		t.Errorf("ZScoreThreshold = %f, want 2.5", cfg.ZScoreThreshold)
	}
}

func TestNewOutlierDetector(t *testing.T) {
	log := zerolog.Nop()
	detector := NewOutlierDetector(DefaultOutlierDetectorConfig(), log)

	if detector == nil {
		t.Fatal("NewOutlierDetector returned nil")
	}
}

func TestOutlierDetector_IsOutlier_InsufficientData(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultOutlierDetectorConfig()
	cfg.MinSamples = 20
	detector := NewOutlierDetector(cfg, log)

	// Add only 10 samples
	for i := range 10 {
		detector.AddScore(float64(i) * 0.1)
	}

	// Should not detect outliers with insufficient data
	if detector.IsOutlier(100.0) {
		t.Error("Should not detect outliers with insufficient data")
	}
}

func TestOutlierDetector_IsOutlier_DetectsOutlier(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultOutlierDetectorConfig()
	cfg.MinSamples = 10
	cfg.ZScoreThreshold = 2.0
	detector := NewOutlierDetector(cfg, log)

	// Add normal scores around 0.5
	for range 50 {
		detector.AddScore(0.5)
	}

	// 0.5 should not be an outlier
	if detector.IsOutlier(0.5) {
		t.Error("0.5 should not be an outlier when all scores are 0.5")
	}

	// Add some variance
	detector.Reset()
	for i := range 50 {
		detector.AddScore(0.5 + float64(i%10-5)*0.02) // 0.4 to 0.6
	}

	// Extreme value should be an outlier
	if !detector.IsOutlier(0.1) {
		t.Error("0.1 should be an outlier when scores are around 0.5")
	}
}

func TestOutlierDetector_IsOutlier_Disabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultOutlierDetectorConfig()
	cfg.Enabled = false
	detector := NewOutlierDetector(cfg, log)

	for range 50 {
		detector.AddScore(0.5)
	}

	// Should never detect outliers when disabled
	if detector.IsOutlier(100.0) {
		t.Error("Should not detect outliers when disabled")
	}
}

func TestOutlierDetector_AddScore_CircularBuffer(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultOutlierDetectorConfig()
	cfg.WindowSize = 10
	detector := NewOutlierDetector(cfg, log)

	// Add more scores than window size
	for i := range 20 {
		detector.AddScore(float64(i))
	}

	_, _, count := detector.GetStats()
	if count != 10 {
		t.Errorf("Count = %d, want 10 (window size)", count)
	}
}

func TestOutlierDetector_GetStats(t *testing.T) {
	log := zerolog.Nop()
	detector := NewOutlierDetector(DefaultOutlierDetectorConfig(), log)

	// Add scores 0.1 to 1.0
	for i := 1; i <= 10; i++ {
		detector.AddScore(float64(i) * 0.1)
	}

	mean, stdDev, count := detector.GetStats()

	if count != 10 {
		t.Errorf("Count = %d, want 10", count)
	}
	if mean < 0.5 || mean > 0.6 {
		t.Errorf("Mean = %f, want ~0.55", mean)
	}
	if stdDev <= 0 {
		t.Errorf("StdDev = %f, want > 0", stdDev)
	}
}

func TestOutlierDetector_FilterFeedback(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultOutlierDetectorConfig()
	cfg.MinSamples = 5
	cfg.ZScoreThreshold = 2.0
	detector := NewOutlierDetector(cfg, log)

	// Pre-populate with normal scores
	for range 20 {
		detector.AddScore(0.5)
	}

	feedbacks := []*entity.FeedbackRecord{
		{TotalScore: 0.5},
		{TotalScore: 0.55},
		{TotalScore: 0.45},
		{TotalScore: 0.1}, // Outlier
		{TotalScore: 0.5},
	}

	filtered := detector.FilterFeedback(feedbacks)

	if len(filtered) >= len(feedbacks) {
		t.Errorf("FilterFeedback should remove outliers: got %d, want < %d", len(filtered), len(feedbacks))
	}
}

func TestOutlierDetector_Enable(t *testing.T) {
	log := zerolog.Nop()
	detector := NewOutlierDetector(DefaultOutlierDetectorConfig(), log)

	detector.Enable(false)
	if detector.GetConfig().Enabled {
		t.Error("Enabled should be false after Enable(false)")
	}

	detector.Enable(true)
	if !detector.GetConfig().Enabled {
		t.Error("Enabled should be true after Enable(true)")
	}
}

func TestOutlierDetector_Reset(t *testing.T) {
	log := zerolog.Nop()
	detector := NewOutlierDetector(DefaultOutlierDetectorConfig(), log)

	for range 50 {
		detector.AddScore(0.5)
	}

	detector.Reset()

	_, _, count := detector.GetStats()
	if count != 0 {
		t.Errorf("Count after reset = %d, want 0", count)
	}
}

// =============================================================================
// RollbackManager Tests
// =============================================================================

func TestNewRollbackManager(t *testing.T) {
	log := zerolog.Nop()
	scorer := NewScorer(nil, nil)
	manager := NewRollbackManager(nil, scorer, 10, log)

	if manager == nil {
		t.Fatal("NewRollbackManager returned nil")
	}
	if manager.maxHistory != 10 {
		t.Errorf("maxHistory = %d, want 10", manager.maxHistory)
	}
}

func TestNewRollbackManager_DefaultMaxHistory(t *testing.T) {
	log := zerolog.Nop()
	scorer := NewScorer(nil, nil)
	manager := NewRollbackManager(nil, scorer, 0, log)

	if manager.maxHistory != 10 {
		t.Errorf("maxHistory = %d, want 10 (default)", manager.maxHistory)
	}
}

func TestRollbackError(t *testing.T) {
	err := &RollbackError{Message: "test error"}
	if err.Error() != "test error" {
		t.Errorf("Error() = %s, want 'test error'", err.Error())
	}
}

func TestErrVersionNotFound(t *testing.T) {
	if ErrVersionNotFound.Error() != "version not found in history" {
		t.Errorf("ErrVersionNotFound.Error() = %s", ErrVersionNotFound.Error())
	}
}
