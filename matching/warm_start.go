package matching

import (
	"context"
	"math"
	"sync"
	"time"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

// =============================================================================
// Warm Start Configuration
// =============================================================================

// WarmStartConfig configures warm start behavior for cold start scenarios.
type WarmStartConfig struct {
	// Prior weights to use when insufficient data
	PriorWeights Weights

	// Equivalent sample count for prior (higher = stronger prior influence)
	PriorStrength int

	// Days until prior influence halves (decay)
	DecayHalfLife int

	// Minimum samples before learning kicks in
	MinSamplesForLearning int

	// Enable warm start
	Enabled bool
}

// DefaultWarmStartConfig returns sensible defaults.
func DefaultWarmStartConfig() WarmStartConfig {
	return WarmStartConfig{
		PriorWeights: Weights{
			Medication: 0.35, // Medication match is most important
			Dosage:     0.25,
			Quantity:   0.15,
			Price:      0.15,
			Recency:    0.10,
		},
		PriorStrength:         50, // Equivalent to 50 samples
		DecayHalfLife:         14, // Prior halves every 2 weeks
		MinSamplesForLearning: 20, // Start blending at 20 samples
		Enabled:               true,
	}
}

// =============================================================================
// Warm Start Manager
// =============================================================================

// WarmStartManager handles cold start scenarios with prior knowledge.
type WarmStartManager struct {
	config      WarmStartConfig
	startTime   time.Time
	sampleCount int
	log         zerolog.Logger
	mu          sync.RWMutex
}

// NewWarmStartManager creates a new warm start manager.
func NewWarmStartManager(cfg WarmStartConfig, log zerolog.Logger) *WarmStartManager {
	return &WarmStartManager{
		config:    cfg,
		startTime: time.Now(),
		log:       log.With().Str("component", "warm-start").Logger(),
	}
}

// GetEffectiveWeights returns weights blended with priors based on sample count.
func (m *WarmStartManager) GetEffectiveWeights(learnedWeights Weights, sampleCount int) Weights {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if !m.config.Enabled {
		return learnedWeights
	}

	// Calculate effective prior strength (decays over time)
	daysSinceStart := time.Since(m.startTime).Hours() / 24
	decayFactor := math.Pow(0.5, daysSinceStart/float64(m.config.DecayHalfLife))
	effectivePriorStrength := float64(m.config.PriorStrength) * decayFactor

	// If not enough samples, use pure prior
	if sampleCount < m.config.MinSamplesForLearning {
		m.log.Debug().
			Int("samples", sampleCount).
			Int("min_required", m.config.MinSamplesForLearning).
			Msg("Using prior weights (insufficient samples)")
		return m.config.PriorWeights
	}

	// Calculate blend ratio
	totalWeight := float64(sampleCount) + effectivePriorStrength
	priorWeight := effectivePriorStrength / totalWeight
	dataWeight := 1 - priorWeight

	// Blend weights
	blended := Weights{
		Medication: dataWeight*learnedWeights.Medication + priorWeight*m.config.PriorWeights.Medication,
		Dosage:     dataWeight*learnedWeights.Dosage + priorWeight*m.config.PriorWeights.Dosage,
		Quantity:   dataWeight*learnedWeights.Quantity + priorWeight*m.config.PriorWeights.Quantity,
		Price:      dataWeight*learnedWeights.Price + priorWeight*m.config.PriorWeights.Price,
		Recency:    dataWeight*learnedWeights.Recency + priorWeight*m.config.PriorWeights.Recency,
	}

	m.log.Debug().
		Int("samples", sampleCount).
		Float64("prior_weight", priorWeight).
		Float64("data_weight", dataWeight).
		Msg("Blended weights with prior")

	return blended
}

// GetPriorInfluence returns the current prior influence percentage.
func (m *WarmStartManager) GetPriorInfluence(sampleCount int) float64 {
	m.mu.RLock()
	defer m.mu.RUnlock()

	if !m.config.Enabled || sampleCount < m.config.MinSamplesForLearning {
		return 100.0
	}

	daysSinceStart := time.Since(m.startTime).Hours() / 24
	decayFactor := math.Pow(0.5, daysSinceStart/float64(m.config.DecayHalfLife))
	effectivePriorStrength := float64(m.config.PriorStrength) * decayFactor

	totalWeight := float64(sampleCount) + effectivePriorStrength
	return (effectivePriorStrength / totalWeight) * 100
}

// SetConfig updates the warm start configuration.
func (m *WarmStartManager) SetConfig(cfg WarmStartConfig) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.config = cfg
}

// GetConfig returns the current configuration.
func (m *WarmStartManager) GetConfig() WarmStartConfig {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.config
}

// Enable enables or disables warm start.
func (m *WarmStartManager) Enable(enabled bool) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.config.Enabled = enabled
}

// Reset resets the start time (useful after major changes).
func (m *WarmStartManager) Reset() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.startTime = time.Now()
	m.log.Info().Msg("Warm start timer reset")
}

// =============================================================================
// Outlier Detection
// =============================================================================

// OutlierDetectorConfig configures outlier detection.
type OutlierDetectorConfig struct {
	// Window size for calculating statistics
	WindowSize int

	// Z-score threshold for outlier detection (default: 2.5)
	ZScoreThreshold float64

	// Minimum samples before detection activates
	MinSamples int

	// Enable outlier detection
	Enabled bool
}

// DefaultOutlierDetectorConfig returns sensible defaults.
func DefaultOutlierDetectorConfig() OutlierDetectorConfig {
	return OutlierDetectorConfig{
		WindowSize:      100,
		ZScoreThreshold: 2.5,
		MinSamples:      20,
		Enabled:         true,
	}
}

// OutlierDetector detects and filters outlier feedback.
type OutlierDetector struct {
	config       OutlierDetectorConfig
	recentScores []float64
	idx          int
	log          zerolog.Logger
	mu           sync.RWMutex
}

// NewOutlierDetector creates a new outlier detector.
func NewOutlierDetector(cfg OutlierDetectorConfig, log zerolog.Logger) *OutlierDetector {
	return &OutlierDetector{
		config:       cfg,
		recentScores: make([]float64, 0, cfg.WindowSize),
		log:          log.With().Str("component", "outlier-detector").Logger(),
	}
}

// AddScore adds a score to the window.
func (d *OutlierDetector) AddScore(score float64) {
	d.mu.Lock()
	defer d.mu.Unlock()

	if len(d.recentScores) < d.config.WindowSize {
		d.recentScores = append(d.recentScores, score)
	} else {
		d.recentScores[d.idx] = score
		d.idx = (d.idx + 1) % d.config.WindowSize
	}
}

// IsOutlier checks if a score is an outlier.
func (d *OutlierDetector) IsOutlier(score float64) bool {
	d.mu.RLock()
	defer d.mu.RUnlock()

	if !d.config.Enabled {
		return false
	}

	if len(d.recentScores) < d.config.MinSamples {
		return false // Not enough data
	}

	mean, stdDev := d.calculateStats()
	if stdDev == 0 {
		return false
	}

	zScore := math.Abs(score-mean) / stdDev
	isOutlier := zScore > d.config.ZScoreThreshold

	if isOutlier {
		d.log.Debug().
			Float64("score", score).
			Float64("mean", mean).
			Float64("std_dev", stdDev).
			Float64("z_score", zScore).
			Msg("🚨 Outlier detected")
	}

	return isOutlier
}

// calculateStats calculates mean and standard deviation.
func (d *OutlierDetector) calculateStats() (mean, stdDev float64) {
	if len(d.recentScores) == 0 {
		return 0, 0
	}

	// Calculate mean
	var sum float64
	for _, s := range d.recentScores {
		sum += s
	}
	mean = sum / float64(len(d.recentScores))

	// Calculate standard deviation
	var sumSq float64
	for _, s := range d.recentScores {
		diff := s - mean
		sumSq += diff * diff
	}
	variance := sumSq / float64(len(d.recentScores))
	stdDev = math.Sqrt(variance)

	return mean, stdDev
}

// GetStats returns current statistics.
func (d *OutlierDetector) GetStats() (mean, stdDev float64, count int) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	mean, stdDev = d.calculateStats()
	count = len(d.recentScores)
	return
}

// FilterFeedback filters outliers from a feedback slice.
func (d *OutlierDetector) FilterFeedback(feedbacks []*entity.FeedbackRecord) []*entity.FeedbackRecord {
	if !d.config.Enabled {
		return feedbacks
	}

	var filtered []*entity.FeedbackRecord
	outlierCount := 0

	for _, f := range feedbacks {
		if !d.IsOutlier(f.TotalScore) {
			filtered = append(filtered, f)
			d.AddScore(f.TotalScore)
		} else {
			outlierCount++
		}
	}

	if outlierCount > 0 {
		d.log.Info().
			Int("total", len(feedbacks)).
			Int("outliers", outlierCount).
			Int("kept", len(filtered)).
			Msg("Filtered outlier feedback")
	}

	return filtered
}

// SetConfig updates the configuration.
func (d *OutlierDetector) SetConfig(cfg OutlierDetectorConfig) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.config = cfg
}

// GetConfig returns the current configuration.
func (d *OutlierDetector) GetConfig() OutlierDetectorConfig {
	d.mu.RLock()
	defer d.mu.RUnlock()
	return d.config
}

// Enable enables or disables outlier detection.
func (d *OutlierDetector) Enable(enabled bool) {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.config.Enabled = enabled
}

// Reset clears the score window.
func (d *OutlierDetector) Reset() {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.recentScores = make([]float64, 0, d.config.WindowSize)
	d.idx = 0
	d.log.Info().Msg("Outlier detector reset")
}

// =============================================================================
// Multi-Level Rollback Manager
// =============================================================================

// RollbackManager manages multi-level weight rollback.
type RollbackManager struct {
	historyRepo WeightHistoryRepository
	scorer      *Scorer
	maxHistory  int
	log         zerolog.Logger
}

// NewRollbackManager creates a new rollback manager.
func NewRollbackManager(historyRepo WeightHistoryRepository, scorer *Scorer, maxHistory int, log zerolog.Logger) *RollbackManager {
	if maxHistory <= 0 {
		maxHistory = 10
	}
	return &RollbackManager{
		historyRepo: historyRepo,
		scorer:      scorer,
		maxHistory:  maxHistory,
		log:         log.With().Str("component", "rollback").Logger(),
	}
}

// RollbackToVersion rolls back to a specific version in history.
func (rm *RollbackManager) RollbackToVersion(ctx context.Context, version int) error {
	history, err := rm.historyRepo.GetHistory(ctx, rm.maxHistory)
	if err != nil {
		return err
	}

	if version >= len(history) {
		rm.log.Error().
			Int("version", version).
			Int("max_available", len(history)-1).
			Msg("Rollback version not found")
		return ErrVersionNotFound
	}

	target := history[version]
	weights := Weights{
		Medication: target.MedicationWeight,
		Dosage:     target.DosageWeight,
		Quantity:   target.QuantityWeight,
		Price:      target.PriceWeight,
		Recency:    target.RecencyWeight,
	}

	rm.scorer.UpdateWeights(weights)

	rm.log.Info().
		Int("version", version).
		Float64("medication", weights.Medication).
		Float64("dosage", weights.Dosage).
		Msg("⏪ Rolled back to version")

	return nil
}

// GetAvailableVersions returns available rollback versions.
func (rm *RollbackManager) GetAvailableVersions(ctx context.Context) ([]*entity.WeightHistory, error) {
	return rm.historyRepo.GetHistory(ctx, rm.maxHistory)
}

// ErrVersionNotFound is returned when rollback version doesn't exist.
var ErrVersionNotFound = &RollbackError{Message: "version not found in history"}

// RollbackError represents a rollback error.
type RollbackError struct {
	Message string
}

func (e *RollbackError) Error() string {
	return e.Message
}
