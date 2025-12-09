package matching

import (
	"fmt"
	"math"
	"pharmabroker/internal/domain"
	"testing"
	"time"
)

func TestDefaultWeights(t *testing.T) {
	w := DefaultWeights()

	// Verify weights sum to 1.0
	sum := w.Medication + w.Dosage + w.Quantity + w.Price + w.Recency
	if math.Abs(sum-1.0) > 0.001 {
		t.Errorf("weights should sum to 1.0, got %v", sum)
	}

	// Verify default values
	if w.Medication != 0.45 {
		t.Errorf("expected Medication=0.45, got %v", w.Medication)
	}
	if w.Dosage != 0.10 {
		t.Errorf("expected Dosage=0.10, got %v", w.Dosage)
	}
	if w.Quantity != 0.2 {
		t.Errorf("expected Quantity=0.2, got %v", w.Quantity)
	}
	if w.Price != 0.15 {
		t.Errorf("expected Price=0.15, got %v", w.Price)
	}
	if w.Recency != 0.10 {
		t.Errorf("expected Recency=0.10, got %v", w.Recency)
	}
}

func TestQuantityScore(t *testing.T) {
	s := NewScorer(nil, nil)

	tests := []struct {
		name     string
		offer    float64
		request  float64
		expected float64
	}{
		// Exact and over-fulfillment
		{"exact match", 10, 10, 1.0},
		{"offer exceeds request", 20, 10, 1.0},
		{"large surplus", 100, 10, 1.0},

		// NEW: Within ±10% tolerance (perfect score)
		{"90% fulfillment (lower tolerance)", 9, 10, 1.0},
		{"95% fulfillment (within tolerance)", 9.5, 10, 1.0},
		{"105% fulfillment (within tolerance)", 10.5, 10, 1.0},
		{"110% fulfillment (upper tolerance)", 11, 10, 1.0},

		// Beyond tolerance - partial scores
		{"89% fulfillment (below tolerance)", 8.9, 10, 0.89},
		{"80% fulfillment (partial)", 8, 10, 0.8},
		{"partial fulfillment 50%", 5, 10, 0.5},
		{"partial fulfillment 25%", 2.5, 10, 0.25},
		{"minimal fulfillment", 1, 100, 0.01},

		// Edge cases
		{"zero request (any ok)", 10, 0, 1.0},
		{"negative request (any ok)", 10, -5, 1.0},
		{"zero offer", 0, 10, 0.0},
		{"negative offer", -5, 10, 0.0},
		{"both zero", 0, 0, 1.0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := s.QuantityScore(tt.offer, tt.request)
			if math.Abs(got-tt.expected) > 0.001 {
				t.Errorf("QuantityScore(%v, %v) = %v, want %v", tt.offer, tt.request, got, tt.expected)
			}
		})
	}
}

func TestPriceScore(t *testing.T) {
	s := NewScorer(nil, nil)

	tests := []struct {
		name     string
		offer    float64
		max      float64
		expected float64
	}{
		// Within budget (below or at tolerance)
		{"well within budget (50%)", 50, 100, 1.0},
		{"at exact budget", 100, 100, 1.0},

		// NEW: Within ±5% tolerance (perfect score)
		{"95% of budget (lower tolerance)", 95, 100, 1.0},
		{"98% of budget (within tolerance)", 98, 100, 1.0},
		{"102% of budget (within tolerance)", 102, 100, 1.0},
		{"105% of budget (upper tolerance)", 105, 100, 1.0},

		// Beyond tolerance - decay
		{"106% of budget (just over)", 106, 100, 0.99},
		{"110% over budget", 110, 100, 0.95},
		{"125% over budget", 125, 100, 0.80},
		{"150% over budget", 150, 100, 0.55},
		{"205% over budget (2x tolerance)", 205, 100, 0.0},
		{"300% over budget", 300, 100, 0.0}, // Clamped to 0

		// No price scenarios
		{"no max price, offer has price", 500, 0, 0.95},
		{"no max price, no offer price", 0, 0, 1.0},
		{"has max, no offer price", 0, 100, 0.85},

		// Edge cases
		{"negative max (treated as no constraint)", 500, -50, 0.95},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := s.PriceScore(tt.offer, tt.max)
			if math.Abs(got-tt.expected) > 0.01 {
				t.Errorf("PriceScore(%v, %v) = %v, want %v", tt.offer, tt.max, got, tt.expected)
			}
		})
	}
}

func TestRecencyScore(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	tests := []struct {
		name     string
		created  time.Time
		minScore float64
		maxScore float64
	}{
		{"just now", now, 0.99, 1.01},
		{"1 minute ago", now.Add(-1 * time.Minute), 0.99, 1.0},
		{"1 hour ago", now.Add(-1 * time.Hour), 0.97, 0.98},
		{"12 hours ago", now.Add(-12 * time.Hour), 0.70, 0.72},
		{"24 hours ago (half-life)", now.Add(-24 * time.Hour), 0.49, 0.51},
		{"48 hours ago", now.Add(-48 * time.Hour), 0.24, 0.26},
		{"3 days ago", now.Add(-72 * time.Hour), 0.12, 0.13},
		{"1 week ago", now.Add(-7 * 24 * time.Hour), 0.007, 0.009},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := s.RecencyScore(tt.created)
			if got < tt.minScore || got > tt.maxScore {
				t.Errorf("RecencyScore() = %v, want between %v and %v", got, tt.minScore, tt.maxScore)
			}
		})
	}
}

func TestRecencyScoreWithHalfLife(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	// Test with different half-lives
	tests := []struct {
		name           string
		created        time.Time
		halfLife       float64
		expectedApprox float64
	}{
		{"12h half-life, 12h ago", now.Add(-12 * time.Hour), 12.0, 0.5},
		{"48h half-life, 48h ago", now.Add(-48 * time.Hour), 48.0, 0.5},
		{"6h half-life, 6h ago", now.Add(-6 * time.Hour), 6.0, 0.5},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := s.RecencyScoreWithHalfLife(tt.created, tt.halfLife)
			if math.Abs(got-tt.expectedApprox) > 0.02 {
				t.Errorf("RecencyScoreWithHalfLife() = %v, want ~%v", got, tt.expectedApprox)
			}
		})
	}
}

func TestGetConfidenceBand(t *testing.T) {
	s := NewScorer(nil, nil) // Uses default thresholds

	tests := []struct {
		score    float64
		expected ConfidenceBand
	}{
		{1.0, ConfidenceAuto},
		{0.95, ConfidenceAuto},
		{0.90, ConfidenceAuto},
		{0.89, ConfidenceSuggest},
		{0.85, ConfidenceSuggest},
		{0.70, ConfidenceSuggest},
		{0.69, ConfidenceReview},
		{0.60, ConfidenceReview},
		{0.50, ConfidenceReview},
		{0.49, ConfidenceNone},
		{0.25, ConfidenceNone},
		{0.0, ConfidenceNone},
	}

	for _, tt := range tests {
		t.Run(fmt.Sprintf("score_%.2f", tt.score), func(t *testing.T) {
			got := s.GetConfidenceBand(tt.score)
			if got != tt.expected {
				t.Errorf("GetConfidenceBand(%v) = %v, want %v", tt.score, got, tt.expected)
			}
		})
	}
}

func TestScoreMatch(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	// Create test offer
	offer := &domain.Offer{
		Medication: "Panadol",
		Quantity:   10,
		Price:      50,
		CreatedAt:  now,
	}

	// Create test request
	request := &domain.Request{
		Medication: "Panadol",
		Quantity:   10,
		MaxPrice:   100,
	}

	// Perfect medication match
	medicationScore := 1.0

	result := s.ScoreMatch(offer, request, medicationScore)

	// Verify individual scores
	if result.MedicationScore != 1.0 {
		t.Errorf("MedicationScore = %v, want 1.0", result.MedicationScore)
	}
	if result.QuantityScore != 1.0 {
		t.Errorf("QuantityScore = %v, want 1.0", result.QuantityScore)
	}
	if result.PriceScore != 1.0 {
		t.Errorf("PriceScore = %v, want 1.0", result.PriceScore)
	}
	if result.RecencyScore < 0.99 {
		t.Errorf("RecencyScore = %v, want >= 0.99", result.RecencyScore)
	}

	// Total should be close to 1.0 for perfect match (use 0.98 for floating point tolerance)
	if result.Total < 0.98 {
		t.Errorf("Total = %v, want >= 0.98", result.Total)
	}

	// Should be AUTO confidence
	if result.Confidence != ConfidenceAuto {
		t.Errorf("Confidence = %v, want AUTO", result.Confidence)
	}

	// Breakdown should not be empty
	if result.Breakdown == "" {
		t.Error("Breakdown should not be empty")
	}
}

func TestScoreMatch_PartialMatch(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	// Offer with partial fulfillment
	offer := &domain.Offer{
		Medication: "Panadol",
		Quantity:   5,                        // Only 50% of requested
		Price:      120,                      // 20% over budget
		CreatedAt:  now.Add(-24 * time.Hour), // 24h old (half-life)
	}

	request := &domain.Request{
		Medication: "Panadol",
		Quantity:   10,
		MaxPrice:   100,
	}

	// Moderate medication match
	medicationScore := 0.7

	result := s.ScoreMatch(offer, request, medicationScore)

	// Verify reduced scores
	if result.QuantityScore != 0.5 {
		t.Errorf("QuantityScore = %v, want 0.5", result.QuantityScore)
	}
	// Price score is now 0.85 (improved from 0.8) for unknown price with budget
	if math.Abs(result.PriceScore-0.85) > 0.01 {
		t.Errorf("PriceScore = %v, want 0.85", result.PriceScore)
	}
	if result.RecencyScore < 0.48 || result.RecencyScore > 0.52 {
		t.Errorf("RecencyScore = %v, want ~0.5", result.RecencyScore)
	}

	// Total should be moderately lower
	// Now: Med=0.45, Dosage=0.10, Qty=0.20, Price=0.15, Recency=0.10
	// Assuming dosageScore ~0.9 (no dosage specified)
	expectedTotal := 0.45*0.7 + 0.10*0.9 + 0.20*0.5 + 0.15*0.8 + 0.10*0.5
	if math.Abs(result.Total-expectedTotal) > 0.05 {
		t.Errorf("Total = %v, want ~%v", result.Total, expectedTotal)
	}

	// Should be REVIEW confidence (0.5-0.7)
	if result.Confidence != ConfidenceReview {
		t.Errorf("Confidence = %v, want REVIEW", result.Confidence)
	}
}

func TestScoreMatch_NoMatch(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	// Poor offer
	offer := &domain.Offer{
		Medication: "Aspirin",
		Quantity:   1,                            // Only 10%
		Price:      250,                          // 150% over budget
		CreatedAt:  now.Add(-7 * 24 * time.Hour), // Week old
	}

	request := &domain.Request{
		Medication: "Panadol",
		Quantity:   10,
		MaxPrice:   100,
	}

	// Poor medication match
	medicationScore := 0.2

	result := s.ScoreMatch(offer, request, medicationScore)

	// Should be NONE confidence
	if result.Confidence != ConfidenceNone {
		t.Errorf("Confidence = %v, want NONE, total=%v", result.Confidence, result.Total)
	}
}

func TestUpdateWeights(t *testing.T) {
	s := NewScorer(nil, nil)

	newWeights := Weights{
		Medication: 0.6,
		Quantity:   0.2,
		Price:      0.1,
		Recency:    0.1,
	}

	s.UpdateWeights(newWeights)

	got := s.GetWeights()
	if got.Medication != 0.6 {
		t.Errorf("Medication weight = %v, want 0.6", got.Medication)
	}
}

func TestUpdateThresholds(t *testing.T) {
	s := NewScorer(nil, nil)

	newThresholds := Thresholds{
		Auto:    0.95,
		Suggest: 0.8,
		Review:  0.6,
	}

	s.UpdateThresholds(newThresholds)

	got := s.GetThresholds()
	if got.Auto != 0.95 {
		t.Errorf("Auto threshold = %v, want 0.95", got.Auto)
	}

	// Verify new thresholds are used
	if s.GetConfidenceBand(0.92) != ConfidenceSuggest {
		t.Error("0.92 should be SUGGEST with Auto=0.95")
	}
}

func TestCustomWeights(t *testing.T) {
	// Create scorer with custom weights
	weights := &Weights{
		Medication: 0.7,
		Quantity:   0.1,
		Price:      0.1,
		Recency:    0.1,
	}
	s := NewScorer(weights, nil)

	now := time.Now()
	offer := &domain.Offer{
		Medication: "Test",
		Quantity:   10,
		Price:      50,
		CreatedAt:  now,
	}
	request := &domain.Request{
		Medication: "Test",
		Quantity:   10,
		MaxPrice:   100,
	}

	// With high medication weight (0.7), total should heavily depend on medication score
	highMedResult := s.ScoreMatch(offer, request, 1.0)
	lowMedResult := s.ScoreMatch(offer, request, 0.5)

	// Difference should be significant due to high medication weight
	diff := highMedResult.Total - lowMedResult.Total
	expectedDiff := 0.5 * 0.7 // 50% medication score reduction * 70% weight
	if math.Abs(diff-expectedDiff) > 0.02 {
		t.Errorf("Score difference = %v, want ~%v", diff, expectedDiff)
	}
}

// Benchmark tests
func BenchmarkQuantityScore(b *testing.B) {
	s := NewScorer(nil, nil)
	for i := 0; i < b.N; i++ {
		s.QuantityScore(float64(i%100), 50)
	}
}

func BenchmarkPriceScore(b *testing.B) {
	s := NewScorer(nil, nil)
	for i := 0; i < b.N; i++ {
		s.PriceScore(float64(i%200), 100)
	}
}

func BenchmarkRecencyScore(b *testing.B) {
	s := NewScorer(nil, nil)
	t := time.Now().Add(-24 * time.Hour)
	for i := 0; i < b.N; i++ {
		s.RecencyScore(t)
	}
}

func BenchmarkScoreMatch(b *testing.B) {
	s := NewScorer(nil, nil)
	now := time.Now()
	offer := &domain.Offer{
		Medication: "Test",
		Quantity:   10,
		Price:      50,
		CreatedAt:  now,
	}
	request := &domain.Request{
		Medication: "Test",
		Quantity:   10,
		MaxPrice:   100,
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		s.ScoreMatch(offer, request, 0.85)
	}
}
