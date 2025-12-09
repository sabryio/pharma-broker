package matching

import (
	"context"
	"errors"
	"math"
	"time"

	"pharmabroker/domain/entity"
)

// LearningConfig holds configuration for the weight learning algorithm
type LearningConfig struct {
	LearningRate   float64 // alpha - how quickly weights adjust (default: 0.1)
	MinWeight      float64 // minimum allowed weight (default: 0.05)
	MaxWeight      float64 // maximum allowed weight (default: 0.70)
	MinChange      float64 // ignore changes smaller than this (default: 0.02)
	MinSamples     int     // minimum feedback samples required (default: 100)
	AnalysisWindow int     // days of feedback to analyze (default: 30)
}

// DefaultLearningConfig returns conservative default configuration
func DefaultLearningConfig() LearningConfig {
	return LearningConfig{
		LearningRate:   0.1,  // Conservative: slow but stable learning
		MinWeight:      0.05, // Keep all factors relevant
		MaxWeight:      0.70, // Prevent single factor dominance
		MinChange:      0.02, // Ignore noise
		MinSamples:     100,  // Require sufficient data
		AnalysisWindow: 30,   // Last 30 days
	}
}

// WeightLearner implements adaptive weight learning based on feedback
type WeightLearner struct {
	feedbackRepo FeedbackRecordRepository
	historyRepo  WeightHistoryRepository
	scorer       *Scorer
	config       LearningConfig
}

// FeedbackRecordRepository defines the interface for feedback storage
type FeedbackRecordRepository interface {
	GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*entity.FeedbackStats, error)
	GetByDateRange(ctx context.Context, startDate, endDate time.Time) ([]*entity.FeedbackRecord, error)
}

// WeightHistoryRepository defines the interface for weight history storage
type WeightHistoryRepository interface {
	Save(ctx context.Context, history *entity.WeightHistory) error
	GetCurrent(ctx context.Context) (*entity.WeightHistory, error)
	GetHistory(ctx context.Context, limit int) ([]*entity.WeightHistory, error)
	SaveWithMetrics(ctx context.Context,
		medicationWeight, dosageWeight, quantityWeight, priceWeight, recencyWeight float64,
		source entity.WeightSource,
		metrics *entity.PerformanceMetrics,
		notes string) error
}

// NewWeightLearner creates a new weight learner with default config
func NewWeightLearner(
	feedbackRepo FeedbackRecordRepository,
	historyRepo WeightHistoryRepository,
	scorer *Scorer,
) *WeightLearner {
	return &WeightLearner{
		feedbackRepo: feedbackRepo,
		historyRepo:  historyRepo,
		scorer:       scorer,
		config:       DefaultLearningConfig(),
	}
}

// NewWeightLearnerWithConfig creates a new weight learner with custom config
func NewWeightLearnerWithConfig(
	feedbackRepo FeedbackRecordRepository,
	historyRepo WeightHistoryRepository,
	scorer *Scorer,
	config LearningConfig,
) *WeightLearner {
	return &WeightLearner{
		feedbackRepo: feedbackRepo,
		historyRepo:  historyRepo,
		scorer:       scorer,
		config:       config,
	}
}

// CalculateOptimalWeights analyzes feedback and computes optimal weights
func (wl *WeightLearner) CalculateOptimalWeights(ctx context.Context, startDate, endDate time.Time) (*Weights, *entity.PerformanceMetrics, error) {
	// Get feedback statistics
	stats, err := wl.feedbackRepo.GetFeedbackStats(ctx, startDate, endDate)
	if err != nil {
		return nil, nil, err
	}

	// Check minimum sample size
	if stats.TotalFeedbacks < wl.config.MinSamples {
		return nil, nil, errors.New("insufficient feedback data for learning")
	}

	// Calculate correlations for each factor
	correlations := wl.calculateCorrelations(stats)

	// Get current weights
	currentWeights := wl.scorer.GetWeights()

	// Adjust weights based on correlations
	newWeights := wl.adjustWeights(currentWeights, correlations)

	// Apply safety constraints
	newWeights = wl.applyConstraints(currentWeights, newWeights)

	// Normalize to sum = 1.0
	newWeights = wl.normalizeWeights(newWeights)

	// Calculate performance metrics
	metrics := wl.calculateMetrics(stats)

	return &newWeights, &metrics, nil
}

// calculateCorrelations computes correlation coefficients for each factor
func (wl *WeightLearner) calculateCorrelations(stats *entity.FeedbackStats) map[string]float64 {
	correlations := make(map[string]float64)

	// For each factor, calculate: (confirmed_avg - rejected_avg) / max(confirmed_avg, rejected_avg)
	// This gives normalized correlation in range [-1, 1]

	// Medication correlation
	if maxVal := math.Max(stats.ConfirmedAvgMedication, stats.RejectedAvgMedication); maxVal > 0 {
		correlations["medication"] = stats.MedicationDiff / maxVal
	}

	// Dosage correlation
	if maxVal := math.Max(stats.ConfirmedAvgDosage, stats.RejectedAvgDosage); maxVal > 0 {
		correlations["dosage"] = stats.DosageDiff / maxVal
	}

	// Quantity correlation
	if maxVal := math.Max(stats.ConfirmedAvgQuantity, stats.RejectedAvgQuantity); maxVal > 0 {
		correlations["quantity"] = stats.QuantityDiff / maxVal
	}

	// Price correlation
	if maxVal := math.Max(stats.ConfirmedAvgPrice, stats.RejectedAvgPrice); maxVal > 0 {
		correlations["price"] = stats.PriceDiff / maxVal
	}

	// Recency correlation
	if maxVal := math.Max(stats.ConfirmedAvgRecency, stats.RejectedAvgRecency); maxVal > 0 {
		correlations["recency"] = stats.RecencyDiff / maxVal
	}

	return correlations
}

// adjustWeights applies learning rate to adjust weights based on correlations
func (wl *WeightLearner) adjustWeights(current Weights, correlations map[string]float64) Weights {
	adjusted := Weights{}

	// Apply formula: new_weight = current_weight * (1 + alpha * correlation)
	adjusted.Medication = current.Medication * (1 + wl.config.LearningRate*correlations["medication"])
	adjusted.Dosage = current.Dosage * (1 + wl.config.LearningRate*correlations["dosage"])
	adjusted.Quantity = current.Quantity * (1 + wl.config.LearningRate*correlations["quantity"])
	adjusted.Price = current.Price * (1 + wl.config.LearningRate*correlations["price"])
	adjusted.Recency = current.Recency * (1 + wl.config.LearningRate*correlations["recency"])

	return adjusted
}

// applyConstraints enforces safety constraints on weights
func (wl *WeightLearner) applyConstraints(current, adjusted Weights) Weights {
	constrained := Weights{}

	// Helper to apply constraints to a single weight
	applyConstraint := func(currentVal, adjustedVal float64) float64 {
		// Check minimum change threshold
		if math.Abs(adjustedVal-currentVal) < wl.config.MinChange {
			return currentVal // Keep unchanged if change is too small
		}

		// Clamp to min/max bounds
		if adjustedVal < wl.config.MinWeight {
			return wl.config.MinWeight
		}
		if adjustedVal > wl.config.MaxWeight {
			return wl.config.MaxWeight
		}

		return adjustedVal
	}

	constrained.Medication = applyConstraint(current.Medication, adjusted.Medication)
	constrained.Dosage = applyConstraint(current.Dosage, adjusted.Dosage)
	constrained.Quantity = applyConstraint(current.Quantity, adjusted.Quantity)
	constrained.Price = applyConstraint(current.Price, adjusted.Price)
	constrained.Recency = applyConstraint(current.Recency, adjusted.Recency)

	return constrained
}

// normalizeWeights ensures all weights sum to 1.0
func (wl *WeightLearner) normalizeWeights(weights Weights) Weights {
	sum := weights.Medication + weights.Dosage + weights.Quantity + weights.Price + weights.Recency

	if sum == 0 {
		// Fallback to equal weights
		return Weights{
			Medication: 0.20,
			Dosage:     0.20,
			Quantity:   0.20,
			Price:      0.20,
			Recency:    0.20,
		}
	}

	return Weights{
		Medication: weights.Medication / sum,
		Dosage:     weights.Dosage / sum,
		Quantity:   weights.Quantity / sum,
		Price:      weights.Price / sum,
		Recency:    weights.Recency / sum,
	}
}

// calculateMetrics computes performance metrics from feedback stats
func (wl *WeightLearner) calculateMetrics(stats *entity.FeedbackStats) entity.PerformanceMetrics {
	metrics := entity.PerformanceMetrics{
		ConfirmationRate:  stats.ConfirmationRate,
		AvgScoreConfirmed: stats.ConfirmedAvgTotal,
		AvgScoreRejected:  stats.RejectedAvgTotal,
		SampleSize:        stats.TotalFeedbacks,
		EvaluatedAt:       time.Now(),
	}

	// Calculate precision (same as confirmation rate in this context)
	metrics.Precision = stats.ConfirmationRate

	// Calculate separation (how well we distinguish good from bad matches)
	metrics.Recall = stats.ConfirmationRate // Simplified

	// F1 Score (harmonic mean of precision and recall)
	if metrics.Precision+metrics.Recall > 0 {
		metrics.F1Score = 2 * (metrics.Precision * metrics.Recall) / (metrics.Precision + metrics.Recall)
	}

	return metrics
}

// ApplyWeights saves and applies new weights to the scorer
func (wl *WeightLearner) ApplyWeights(ctx context.Context, weights Weights, source entity.WeightSource, metrics *entity.PerformanceMetrics, notes string) error {
	// Save to history
	err := wl.historyRepo.SaveWithMetrics(ctx,
		weights.Medication,
		weights.Dosage,
		weights.Quantity,
		weights.Price,
		weights.Recency,
		source,
		metrics,
		notes,
	)
	if err != nil {
		return err
	}

	// Apply to scorer
	wl.scorer.UpdateWeights(weights)

	return nil
}

// ShouldApply determines if new weights should be auto-applied
// Returns true if new weights show improvement
func (wl *WeightLearner) ShouldApply(oldMetrics, newMetrics entity.PerformanceMetrics) bool {
	// Calculate separation (how well we distinguish confirmed from rejected)
	oldSeparation := oldMetrics.AvgScoreConfirmed - oldMetrics.AvgScoreRejected
	newSeparation := newMetrics.AvgScoreConfirmed - newMetrics.AvgScoreRejected

	// Apply if:
	// 1. Separation improved (better discrimination)
	// 2. Confirmation rate didn't drop by more than 5%
	separationImproved := newSeparation > oldSeparation
	confirmationRateOK := newMetrics.ConfirmationRate >= (oldMetrics.ConfirmationRate - 0.05)

	return separationImproved && confirmationRateOK
}

// Rollback reverts to the previous weight configuration
func (wl *WeightLearner) Rollback(ctx context.Context) error {
	// Get last 2 weight configurations
	history, err := wl.historyRepo.GetHistory(ctx, 2)
	if err != nil {
		return err
	}

	if len(history) < 2 {
		return errors.New("no previous weights available for rollback")
	}

	// Apply previous weights
	previousWeights := Weights{
		Medication: history[1].MedicationWeight,
		Dosage:     history[1].DosageWeight,
		Quantity:   history[1].QuantityWeight,
		Price:      history[1].PriceWeight,
		Recency:    history[1].RecencyWeight,
	}

	wl.scorer.UpdateWeights(previousWeights)

	return nil
}

// SetConfig updates the learning configuration
func (wl *WeightLearner) SetConfig(config LearningConfig) {
	wl.config = config
}

// GetConfig returns the current learning configuration
func (wl *WeightLearner) GetConfig() LearningConfig {
	return wl.config
}
