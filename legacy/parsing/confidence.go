package parsing

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// Dynamic Confidence Threshold Configuration
// =============================================================================

// ConfidenceConfig holds configuration for dynamic confidence thresholds.
type ConfidenceConfig struct {
	// Base thresholds (starting points)
	BaseStrictThreshold  float64 // Default: 0.7
	BaseRelaxedThreshold float64 // Default: 0.4

	// Adaptive adjustment settings
	EnableAdaptive      bool    // Enable automatic threshold adjustment
	AdjustmentStep      float64 // How much to adjust per evaluation (default: 0.02)
	MinThreshold        float64 // Minimum allowed threshold (default: 0.3)
	MaxThreshold        float64 // Maximum allowed threshold (default: 0.95)
	EvaluationWindow    int     // Number of results to evaluate before adjusting (default: 100)
	TargetAcceptRate    float64 // Target acceptance rate (default: 0.85)
	AcceptRateTolerance float64 // Tolerance around target (default: 0.05)
}

// DefaultConfidenceConfig returns sensible defaults for confidence configuration.
func DefaultConfidenceConfig() ConfidenceConfig {
	return ConfidenceConfig{
		BaseStrictThreshold:  DefaultStrictConfidence,
		BaseRelaxedThreshold: DefaultRelaxedConfidence,
		EnableAdaptive:       false, // Disabled by default for stability
		AdjustmentStep:       DefaultConfidenceAdjustmentStep,
		MinThreshold:         DefaultMinConfidenceThreshold,
		MaxThreshold:         DefaultMaxConfidenceThreshold,
		EvaluationWindow:     DefaultConfidenceEvaluationWindow,
		TargetAcceptRate:     DefaultTargetAcceptRate,
		AcceptRateTolerance:  DefaultAcceptRateTolerance,
	}
}

// ConfidenceStats tracks confidence-related statistics.
type ConfidenceStats struct {
	TotalEvaluations     atomic.Int64 // Total items evaluated
	AcceptedItems        atomic.Int64 // Items above threshold
	RejectedItems        atomic.Int64 // Items below threshold
	ThresholdAdjustments atomic.Int64 // Number of threshold adjustments made
	AvgConfidenceSum     atomic.Int64 // Sum of confidence * 1000 (for averaging)
}

// GetStats returns a snapshot of confidence statistics.
func (s *ConfidenceStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_evaluations":     s.TotalEvaluations.Load(),
		"accepted_items":        s.AcceptedItems.Load(),
		"rejected_items":        s.RejectedItems.Load(),
		"threshold_adjustments": s.ThresholdAdjustments.Load(),
	}
}

// GetAcceptanceRate returns the current acceptance rate.
func (s *ConfidenceStats) GetAcceptanceRate() float64 {
	total := s.TotalEvaluations.Load()
	if total == 0 {
		return 1.0
	}
	return float64(s.AcceptedItems.Load()) / float64(total)
}

// GetAverageConfidence returns the average confidence score.
func (s *ConfidenceStats) GetAverageConfidence() float64 {
	total := s.TotalEvaluations.Load()
	if total == 0 {
		return 0.0
	}
	return float64(s.AvgConfidenceSum.Load()) / float64(total) / 1000.0
}

// =============================================================================
// Dynamic Confidence Manager
// =============================================================================

// ConfidenceManager manages dynamic confidence thresholds.
type ConfidenceManager struct {
	config ConfidenceConfig
	stats  ConfidenceStats
	log    zerolog.Logger

	// Current thresholds (may differ from base if adaptive is enabled)
	currentStrict  float64
	currentRelaxed float64

	// Window tracking for adaptive adjustment
	windowAccepted int
	windowTotal    int
	windowMu       sync.Mutex

	// Last adjustment time
	lastAdjustment time.Time
}

// NewConfidenceManager creates a new confidence manager.
func NewConfidenceManager(cfg ConfidenceConfig, log zerolog.Logger) *ConfidenceManager {
	// Apply defaults for zero values
	if cfg.BaseStrictThreshold <= 0 {
		cfg.BaseStrictThreshold = DefaultStrictConfidence
	}
	if cfg.BaseRelaxedThreshold <= 0 {
		cfg.BaseRelaxedThreshold = DefaultRelaxedConfidence
	}
	if cfg.AdjustmentStep <= 0 {
		cfg.AdjustmentStep = DefaultConfidenceAdjustmentStep
	}
	if cfg.MinThreshold <= 0 {
		cfg.MinThreshold = DefaultMinConfidenceThreshold
	}
	if cfg.MaxThreshold <= 0 {
		cfg.MaxThreshold = DefaultMaxConfidenceThreshold
	}
	if cfg.EvaluationWindow <= 0 {
		cfg.EvaluationWindow = DefaultConfidenceEvaluationWindow
	}
	if cfg.TargetAcceptRate <= 0 {
		cfg.TargetAcceptRate = DefaultTargetAcceptRate
	}
	if cfg.AcceptRateTolerance <= 0 {
		cfg.AcceptRateTolerance = DefaultAcceptRateTolerance
	}

	return &ConfidenceManager{
		config:         cfg,
		log:            log.With().Str("component", "confidence-manager").Logger(),
		currentStrict:  cfg.BaseStrictThreshold,
		currentRelaxed: cfg.BaseRelaxedThreshold,
		lastAdjustment: time.Now(),
	}
}

// GetStrictThreshold returns the current strict confidence threshold.
func (cm *ConfidenceManager) GetStrictThreshold() float64 {
	return cm.currentStrict
}

// GetRelaxedThreshold returns the current relaxed confidence threshold.
func (cm *ConfidenceManager) GetRelaxedThreshold() float64 {
	return cm.currentRelaxed
}

// SetStrictThreshold manually sets the strict threshold.
func (cm *ConfidenceManager) SetStrictThreshold(threshold float64) {
	if threshold < cm.config.MinThreshold {
		threshold = cm.config.MinThreshold
	}
	if threshold > cm.config.MaxThreshold {
		threshold = cm.config.MaxThreshold
	}
	cm.currentStrict = threshold
	cm.log.Info().
		Float64("strict_threshold", threshold).
		Msg("Strict confidence threshold updated")
}

// SetRelaxedThreshold manually sets the relaxed threshold.
func (cm *ConfidenceManager) SetRelaxedThreshold(threshold float64) {
	if threshold < cm.config.MinThreshold {
		threshold = cm.config.MinThreshold
	}
	if threshold > cm.config.MaxThreshold {
		threshold = cm.config.MaxThreshold
	}
	cm.currentRelaxed = threshold
	cm.log.Info().
		Float64("relaxed_threshold", threshold).
		Msg("Relaxed confidence threshold updated")
}

// EvaluateConfidence evaluates a confidence score and tracks statistics.
// Returns true if the confidence meets the strict threshold.
func (cm *ConfidenceManager) EvaluateConfidence(confidence float64) bool {
	cm.stats.TotalEvaluations.Add(1)
	cm.stats.AvgConfidenceSum.Add(int64(confidence * 1000))

	accepted := confidence >= cm.currentStrict
	if accepted {
		cm.stats.AcceptedItems.Add(1)
	} else {
		cm.stats.RejectedItems.Add(1)
	}

	// Track for adaptive adjustment
	if cm.config.EnableAdaptive {
		cm.trackForAdaptive(accepted)
	}

	return accepted
}

// EvaluateRelaxed evaluates against the relaxed threshold.
func (cm *ConfidenceManager) EvaluateRelaxed(confidence float64) bool {
	return confidence >= cm.currentRelaxed
}

// trackForAdaptive tracks results for adaptive threshold adjustment.
func (cm *ConfidenceManager) trackForAdaptive(accepted bool) {
	cm.windowMu.Lock()
	defer cm.windowMu.Unlock()

	cm.windowTotal++
	if accepted {
		cm.windowAccepted++
	}

	// Check if we've reached the evaluation window
	if cm.windowTotal >= cm.config.EvaluationWindow {
		cm.adjustThresholds()
	}
}

// adjustThresholds adjusts thresholds based on acceptance rate.
func (cm *ConfidenceManager) adjustThresholds() {
	if cm.windowTotal == 0 {
		return
	}

	acceptRate := float64(cm.windowAccepted) / float64(cm.windowTotal)
	targetLow := cm.config.TargetAcceptRate - cm.config.AcceptRateTolerance
	targetHigh := cm.config.TargetAcceptRate + cm.config.AcceptRateTolerance

	var adjustment float64
	var direction string

	if acceptRate < targetLow {
		// Too many rejections - lower threshold
		adjustment = -cm.config.AdjustmentStep
		direction = "lowered"
	} else if acceptRate > targetHigh {
		// Too many acceptances - raise threshold
		adjustment = cm.config.AdjustmentStep
		direction = "raised"
	} else {
		// Within tolerance - no adjustment needed
		cm.windowAccepted = 0
		cm.windowTotal = 0
		return
	}

	// Apply adjustment to strict threshold
	newStrict := cm.currentStrict + adjustment
	if newStrict < cm.config.MinThreshold {
		newStrict = cm.config.MinThreshold
	}
	if newStrict > cm.config.MaxThreshold {
		newStrict = cm.config.MaxThreshold
	}

	// Only log and update if actually changed
	if newStrict != cm.currentStrict {
		cm.currentStrict = newStrict
		cm.stats.ThresholdAdjustments.Add(1)
		cm.lastAdjustment = time.Now()

		cm.log.Info().
			Float64("accept_rate", acceptRate).
			Float64("target_rate", cm.config.TargetAcceptRate).
			Float64("new_strict_threshold", newStrict).
			Str("direction", direction).
			Msg("🎚️ Adaptive threshold adjustment")
	}

	// Reset window
	cm.windowAccepted = 0
	cm.windowTotal = 0
}

// GetStats returns the current confidence statistics.
func (cm *ConfidenceManager) GetStats() map[string]interface{} {
	stats := cm.stats.GetStats()
	return map[string]interface{}{
		"total_evaluations":     stats["total_evaluations"],
		"accepted_items":        stats["accepted_items"],
		"rejected_items":        stats["rejected_items"],
		"threshold_adjustments": stats["threshold_adjustments"],
		"acceptance_rate":       cm.stats.GetAcceptanceRate(),
		"average_confidence":    cm.stats.GetAverageConfidence(),
		"current_strict":        cm.currentStrict,
		"current_relaxed":       cm.currentRelaxed,
		"adaptive_enabled":      cm.config.EnableAdaptive,
	}
}

// GetConfig returns the current configuration.
func (cm *ConfidenceManager) GetConfig() ConfidenceConfig {
	return cm.config
}

// SetConfig updates the configuration.
func (cm *ConfidenceManager) SetConfig(cfg ConfidenceConfig) {
	cm.config = cfg
	cm.log.Info().
		Float64("base_strict", cfg.BaseStrictThreshold).
		Float64("base_relaxed", cfg.BaseRelaxedThreshold).
		Bool("adaptive", cfg.EnableAdaptive).
		Msg("Confidence configuration updated")
}

// EnableAdaptive enables or disables adaptive threshold adjustment.
func (cm *ConfidenceManager) EnableAdaptive(enabled bool) {
	cm.config.EnableAdaptive = enabled
	cm.log.Info().
		Bool("enabled", enabled).
		Msg("Adaptive confidence adjustment toggled")
}

// ResetToBase resets thresholds to base values.
func (cm *ConfidenceManager) ResetToBase() {
	cm.currentStrict = cm.config.BaseStrictThreshold
	cm.currentRelaxed = cm.config.BaseRelaxedThreshold
	cm.windowAccepted = 0
	cm.windowTotal = 0
	cm.log.Info().
		Float64("strict", cm.currentStrict).
		Float64("relaxed", cm.currentRelaxed).
		Msg("Confidence thresholds reset to base values")
}
