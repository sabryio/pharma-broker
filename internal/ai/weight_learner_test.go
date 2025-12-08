package ai

import (
	"context"
	"math"
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// Mock repositories for testing
type mockFeedbackRepo struct {
	stats *domain.FeedbackStats
	err   error
}

func (m *mockFeedbackRepo) GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*domain.FeedbackStats, error) {
	return m.stats, m.err
}

func (m *mockFeedbackRepo) GetByDateRange(ctx context.Context, startDate, endDate time.Time) ([]*domain.FeedbackRecord, error) {
	return nil, nil
}

type mockWeightHistoryRepo struct {
	saved   []*domain.WeightHistory
	current *domain.WeightHistory
	err     error
}

func (m *mockWeightHistoryRepo) Save(ctx context.Context, history *domain.WeightHistory) error {
	m.saved = append(m.saved, history)
	return m.err
}

func (m *mockWeightHistoryRepo) GetCurrent(ctx context.Context) (*domain.WeightHistory, error) {
	return m.current, m.err
}

func (m *mockWeightHistoryRepo) GetHistory(ctx context.Context, limit int) ([]*domain.WeightHistory, error) {
	if len(m.saved) <= limit {
		return m.saved, m.err
	}
	return m.saved[:limit], m.err
}

func (m *mockWeightHistoryRepo) SaveWithMetrics(ctx context.Context,
	medicationWeight, dosageWeight, quantityWeight, priceWeight, recencyWeight float64,
	source domain.WeightSource,
	metrics *domain.PerformanceMetrics,
	notes string) error {
	return m.err
}

func TestDefaultLearningConfig(t *testing.T) {
	config := DefaultLearningConfig()

	if config.LearningRate != 0.1 {
		t.Errorf("LearningRate = %v, want 0.1", config.LearningRate)
	}
	if config.MinWeight != 0.05 {
		t.Errorf("MinWeight = %v, want 0.05", config.MinWeight)
	}
	if config.MaxWeight != 0.70 {
		t.Errorf("MaxWeight = %v, want 0.70", config.MaxWeight)
	}
	if config.MinChange != 0.02 {
		t.Errorf("MinChange = %v, want 0.02", config.MinChange)
	}
	if config.MinSamples != 100 {
		t.Errorf("MinSamples = %v, want 100", config.MinSamples)
	}
	if config.AnalysisWindow != 30 {
		t.Errorf("AnalysisWindow = %v, want 30", config.AnalysisWindow)
	}
}

func TestCalculateCorrelations(t *testing.T) {
	wl := &WeightLearner{
		config: DefaultLearningConfig(),
	}

	stats := &domain.FeedbackStats{
		// Medication: strong positive correlation (confirmed much higher)
		ConfirmedAvgMedication: 0.90,
		RejectedAvgMedication:  0.60,
		MedicationDiff:         0.30,

		// Quantity: weak positive correlation
		ConfirmedAvgQuantity: 0.85,
		RejectedAvgQuantity:  0.80,
		QuantityDiff:         0.05,

		// Price: negative correlation (higher in rejected!)
		ConfirmedAvgPrice: 0.70,
		RejectedAvgPrice:  0.80,
		PriceDiff:         -0.10,
	}

	correlations := wl.calculateCorrelations(stats)

	// Medication: (0.90 - 0.60) / 0.90 = 0.30 / 0.90 ≈ 0.333
	expectedMed := 0.30 / 0.90
	if math.Abs(correlations["medication"]-expectedMed) > 0.001 {
		t.Errorf("Medication correlation = %v, want %v", correlations["medication"], expectedMed)
	}

	// Quantity: (0.85 - 0.80) / 0.85 ≈ 0.059
	expectedQty := 0.05 / 0.85
	if math.Abs(correlations["quantity"]-expectedQty) > 0.001 {
		t.Errorf("Quantity correlation = %v, want %v", correlations["quantity"], expectedQty)
	}

	// Price: (-0.10) / 0.80 = -0.125 (negative!)
	expectedPrice := -0.10 / 0.80
	if math.Abs(correlations["price"]-expectedPrice) > 0.001 {
		t.Errorf("Price correlation = %v, want %v", correlations["price"], expectedPrice)
	}
}

func TestAdjustWeights_PositiveCorrelation(t *testing.T) {
	wl := &WeightLearner{
		config: DefaultLearningConfig(),
	}

	current := ScoringWeights{
		Medication: 0.45,
		Dosage:     0.10,
		Quantity:   0.20,
		Price:      0.15,
		Recency:    0.10,
	}

	// Medication has strong positive correlation
	correlations := map[string]float64{
		"medication": 0.30,  // Strong positive
		"dosage":     0.0,   // Neutral
		"quantity":   0.0,   // Neutral
		"price":      -0.10, // Negative
		"recency":    0.0,   // Neutral
	}

	adjusted := wl.adjustWeights(current, correlations)

	// Medication should increase: 0.45 * (1 + 0.1 * 0.30) = 0.45 * 1.03 = 0.4635
	expected := 0.45 * (1 + 0.1*0.30)
	if math.Abs(adjusted.Medication-expected) > 0.0001 {
		t.Errorf("Adjusted medication = %v, want %v", adjusted.Medication, expected)
	}

	// Price should decrease: 0.15 * (1 + 0.1 * -0.10) = 0.15 * 0.99 = 0.1485
	expectedPrice := 0.15 * (1 + 0.1*-0.10)
	if math.Abs(adjusted.Price-expectedPrice) > 0.0001 {
		t.Errorf("Adjusted price = %v, want %v", adjusted.Price, expectedPrice)
	}
}

func TestApplyConstraints_MinMaxBounds(t *testing.T) {
	wl := &WeightLearner{
		config: LearningConfig{
			MinWeight: 0.05,
			MaxWeight: 0.70,
			MinChange: 0.02,
		},
	}

	current := ScoringWeights{
		Medication: 0.50,
		Dosage:     0.10,
		Quantity:   0.20,
		Price:      0.10,
		Recency:    0.10,
	}

	adjusted := ScoringWeights{
		Medication: 0.80, // Exceeds max
		Dosage:     0.02, // Below min
		Quantity:   0.21, // Small change (0.01 < 0.02)
		Price:      0.15, // Acceptable change
		Recency:    0.10, // No change
	}

	constrained := wl.applyConstraints(current, adjusted)

	// Medication should be clamped to max
	if constrained.Medication != 0.70 {
		t.Errorf("Medication = %v, want 0.70 (max)", constrained.Medication)
	}

	// Dosage should be raised to min
	if constrained.Dosage != 0.05 {
		t.Errorf("Dosage = %v, want 0.05 (min)", constrained.Dosage)
	}

	// Quantity change too small, should keep original
	if constrained.Quantity != 0.20 {
		t.Errorf("Quantity = %v, want 0.20 (unchanged)", constrained.Quantity)
	}

	// Price should be adjusted (change = 0.05 > 0.02)
	if constrained.Price != 0.15 {
		t.Errorf("Price = %v, want 0.15", constrained.Price)
	}
}

func TestNormalizeWeights(t *testing.T) {
	wl := &WeightLearner{}

	weights := ScoringWeights{
		Medication: 0.50,
		Dosage:     0.10,
		Quantity:   0.20,
		Price:      0.15,
		Recency:    0.10,
	}
	// Sum = 1.05

	normalized := wl.normalizeWeights(weights)

	// Sum should be 1.0
	sum := normalized.Medication + normalized.Dosage + normalized.Quantity + normalized.Price + normalized.Recency
	if math.Abs(sum-1.0) > 0.0001 {
		t.Errorf("Sum of weights = %v, want 1.0", sum)
	}

	// Proportionsshould be maintained
	expectedMed := 0.50 / 1.05
	if math.Abs(normalized.Medication-expectedMed) > 0.0001 {
		t.Errorf("Normalized medication = %v, want %v", normalized.Medication, expectedMed)
	}
}

func TestNormalizeWeights_ZeroSum(t *testing.T) {
	wl := &WeightLearner{}

	weights := ScoringWeights{} // All zeros

	normalized := wl.normalizeWeights(weights)

	// Should return equal weights
	if normalized.Medication != 0.20 {
		t.Errorf("Medication = %v, want 0.20 (equal)", normalized.Medication)
	}

	sum := normalized.Medication + normalized.Dosage + normalized.Quantity + normalized.Price + normalized.Recency
	if math.Abs(sum-1.0) > 0.0001 {
		t.Errorf("Sum of weights = %v, want 1.0", sum)
	}
}

func TestCalculateOptimalWeights_InsufficientData(t *testing.T) {
	feedbackRepo := &mockFeedbackRepo{
		stats: &domain.FeedbackStats{
			TotalFeedbacks: 50, // Less than minimum (100)
		},
	}

	scorer := NewScorer(nil, nil)
	wl := NewWeightLearner(feedbackRepo, &mockWeightHistoryRepo{}, scorer)

	ctx := context.Background()
	startDate := time.Now().Add(-30 * 24 * time.Hour)
	endDate := time.Now()

	_, _, err := wl.CalculateOptimalWeights(ctx, startDate, endDate)

	if err == nil {
		t.Error("Expected error for insufficient data, got nil")
	}
	if err.Error() != "insufficient feedback data for learning" {
		t.Errorf("Error = %v, want 'insufficient feedback data for learning'", err)
	}
}

func TestCalculateOptimalWeights_Success(t *testing.T) {
	feedbackRepo := &mockFeedbackRepo{
		stats: &domain.FeedbackStats{
			TotalFeedbacks:   200,
			ConfirmedCount:   150,
			RejectedCount:    50,
			ConfirmationRate: 0.75,

			// Strong medication correlation
			ConfirmedAvgMedication: 0.90,
			RejectedAvgMedication:  0.60,
			MedicationDiff:         0.30,

			// Weak other correlations
			ConfirmedAvgDosage:   0.85,
			RejectedAvgDosage:    0.82,
			DosageDiff:           0.03,
			ConfirmedAvgQuantity: 0.88,
			RejectedAvgQuantity:  0.85,
			QuantityDiff:         0.03,
			ConfirmedAvgPrice:    0.80,
			RejectedAvgPrice:     0.78,
			PriceDiff:            0.02,
			ConfirmedAvgRecency:  0.92,
			RejectedAvgRecency:   0.90,
			RecencyDiff:          0.02,

			ConfirmedAvgTotal: 0.88,
			RejectedAvgTotal:  0.75,
		},
	}

	scorer := NewScorer(nil, nil)
	wl := NewWeightLearner(feedbackRepo, &mockWeightHistoryRepo{}, scorer)

	ctx := context.Background()
	startDate := time.Now().Add(-30 * 24 * time.Hour)
	endDate := time.Now()

	weights, metrics, err := wl.CalculateOptimalWeights(ctx, startDate, endDate)

	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	if weights == nil {
		t.Fatal("Expected weights, got nil")
	}

	if metrics == nil {
		t.Fatal("Expected metrics, got nil")
	}

	// Medication should have highest weight (strongest correlation)
	if weights.Medication <= weights.Dosage || weights.Medication <= weights.Quantity {
		t.Errorf("Expected medication to have highest weight, got: med=%v, dosage=%v, qty=%v",
			weights.Medication, weights.Dosage, weights.Quantity)
	}

	// Sum should be 1.0
	sum := weights.Medication + weights.Dosage + weights.Quantity + weights.Price + weights.Recency
	if math.Abs(sum-1.0) > 0.0001 {
		t.Errorf("Sum of weights = %v, want 1.0", sum)
	}

	// Metrics should reflect stats
	if metrics.ConfirmationRate != 0.75 {
		t.Errorf("ConfirmationRate = %v, want 0.75", metrics.ConfirmationRate)
	}
	if metrics.SampleSize != 200 {
		t.Errorf("SampleSize = %v, want 200", metrics.SampleSize)
	}
}

func TestShouldApply(t *testing.T) {
	wl := &WeightLearner{}

	tests := []struct {
		name        string
		oldMetrics  domain.PerformanceMetrics
		newMetrics  domain.PerformanceMetrics
		shouldApply bool
	}{
		{
			name: "Better separation, stable confirmation rate",
			oldMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.85,
				AvgScoreRejected:  0.65,
				ConfirmationRate:  0.75,
			},
			newMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.88,
				AvgScoreRejected:  0.63,
				ConfirmationRate:  0.76,
			},
			shouldApply: true,
		},
		{
			name: "Worse separation",
			oldMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.85,
				AvgScoreRejected:  0.65,
				ConfirmationRate:  0.75,
			},
			newMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.82,
				AvgScoreRejected:  0.68,
				ConfirmationRate:  0.75,
			},
			shouldApply: false,
		},
		{
			name: "Better separation but confirmation rate dropped too much",
			oldMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.85,
				AvgScoreRejected:  0.65,
				ConfirmationRate:  0.75,
			},
			newMetrics: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.90,
				AvgScoreRejected:  0.60,
				ConfirmationRate:  0.65, // Dropped by 0.10 (> 0.05 threshold)
			},
			shouldApply: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := wl.ShouldApply(tt.oldMetrics, tt.newMetrics)
			if result != tt.shouldApply {
				t.Errorf("ShouldApply() = %v, want %v", result, tt.shouldApply)
			}
		})
	}
}

func TestSetGetConfig(t *testing.T) {
	wl := NewWeightLearner(nil, nil, nil)

	customConfig := LearningConfig{
		LearningRate:   0.05,
		MinWeight:      0.10,
		MaxWeight:      0.60,
		MinChange:      0.03,
		MinSamples:     200,
		AnalysisWindow: 60,
	}

	wl.SetConfig(customConfig)

	got := wl.GetConfig()

	if got.LearningRate != 0.05 {
		t.Errorf("LearningRate = %v, want 0.05", got.LearningRate)
	}
	if got.MinSamples != 200 {
		t.Errorf("MinSamples = %v, want 200", got.MinSamples)
	}
}

func TestCalculateMetrics(t *testing.T) {
	wl := &WeightLearner{}

	stats := &domain.FeedbackStats{
		TotalFeedbacks:    200,
		ConfirmedCount:    150,
		ConfirmationRate:  0.75,
		ConfirmedAvgTotal: 0.88,
		RejectedAvgTotal:  0.65,
	}

	metrics := wl.calculateMetrics(stats)

	if metrics.ConfirmationRate != 0.75 {
		t.Errorf("ConfirmationRate = %v, want 0.75", metrics.ConfirmationRate)
	}
	if metrics.AvgScoreConfirmed != 0.88 {
		t.Errorf("AvgScoreConfirmed = %v, want 0.88", metrics.AvgScoreConfirmed)
	}
	if metrics.AvgScoreRejected != 0.65 {
		t.Errorf("AvgScoreRejected = %v, want 0.65", metrics.AvgScoreRejected)
	}
	if metrics.SampleSize != 200 {
		t.Errorf("SampleSize = %v, want 200", metrics.SampleSize)
	}
	if metrics.Precision != 0.75 {
		t.Errorf("Precision = %v, want 0.75", metrics.Precision)
	}

	// F1 Score should be calculated
	expectedF1 := 2 * (0.75 * 0.75) / (0.75 + 0.75)
	if math.Abs(metrics.F1Score-expectedF1) > 0.0001 {
		t.Errorf("F1Score = %v, want %v", metrics.F1Score, expectedF1)
	}
}
