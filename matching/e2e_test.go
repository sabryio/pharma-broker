package matching

import (
	"testing"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/dosage"
)

// TestEndToEnd_AllEnhancements tests all Phase A and B enhancements working together
func TestEndToEnd_AllEnhancements(t *testing.T) {
	scorer := NewScorer(nil, nil)
	now := time.Now()

	// Create offers with various characteristics
	offers := []*entity.Offer{
		{
			ID:         "offer1",
			Medication: "Ozempic 2mg", // Exact dosage
			Quantity:   95,            // Within 10% tolerance
			Price:      1020,          // Within 5% price tolerance
			CreatedAt:  now.Add(-1 * time.Hour),
		},
		{
			ID:         "offer2",
			Medication: "Ozempic 2.1mg", // Within dosage tolerance
			Quantity:   110,             // Within quantity tolerance (upper)
			Price:      980,             // Below budget (good deal)
			CreatedAt:  now.Add(-12 * time.Hour),
		},
		{
			ID:         "offer3",
			Medication: "أوزمبك ٢ملغ", // Arabic dosage
			Quantity:   90,            // Lower tolerance
			Price:      1050,          // Just within tolerance
			CreatedAt:  now.Add(-24 * time.Hour),
		},
	}

	request := &entity.Request{
		ID:         "req1",
		Medication: "Ozempic 2mg",
		Quantity:   100,
		MaxPrice:   1000,
	}

	t.Run("All offers should match well due to tolerances", func(t *testing.T) {
		for i, offer := range offers {
			// Medication score (exact or synonym match)
			medScore := 1.0

			result := scorer.ScoreMatch(offer, request, medScore)

			t.Logf("Offer %d (%s):", i+1, offer.ID)
			t.Logf("  Medication: %s → Score: %.2f", offer.Medication, result.MedicationScore)
			t.Logf("  Dosage: → Score: %.2f", result.DosageScore)
			t.Logf("  Quantity: %v/%v → Score: %.2f", offer.Quantity, request.Quantity, result.QuantityScore)
			t.Logf("  Price: %v/%v → Score: %.2f", offer.Price, request.MaxPrice, result.PriceScore)
			t.Logf("  Recency: %v → Score: %.2f", time.Since(offer.CreatedAt), result.RecencyScore)
			t.Logf("  TOTAL: %.2f (%s)", result.Total, result.Confidence)
			t.Logf("  Breakdown: %s", result.Breakdown)
			t.Logf("")

			// All should have decent scores due to tolerances
			if result.Total < 0.7 {
				t.Errorf("Offer %d total score too low: %.2f", i+1, result.Total)
			}

			// Verify specific scores
			if result.QuantityScore < 0.9 {
				t.Errorf("Offer %d quantity score should benefit from tolerance: %.2f", i+1, result.QuantityScore)
			}

			if i < 2 && result.PriceScore < 0.9 {
				t.Errorf("Offer %d price score should benefit from tolerance: %.2f", i+1, result.PriceScore)
			}
		}
	})

	t.Run("Dosage normalization works across languages", func(t *testing.T) {
		// English dosage
		dosage1 := dosage.ParseDosage("Ozempic 2mg")
		// Arabic dosage
		dosage2 := dosage.ParseDosage("أوزمبك ٢ملغ")

		if dosage1 == nil || dosage2 == nil {
			t.Fatal("Dosage parsing failed")
		}

		similarity := dosage.CompareDosages(dosage1, dosage2)
		t.Logf("English '2mg' vs Arabic '٢ملغ': %.2f similarity", similarity)

		if similarity < 0.95 {
			t.Errorf("Cross-language dosage should match: %.2f", similarity)
		}
	})

	t.Run("Configurable recency decay", func(t *testing.T) {
		oldOffer := &entity.Offer{
			Medication: "Test",
			Quantity:   100,
			Price:      100,
			CreatedAt:  now.Add(-36 * time.Hour),
		}

		// Test with different decay types
		scorer.SetDecayType(DecayExponential)
		expScore := scorer.RecencyScore(oldOffer.CreatedAt)

		scorer.SetDecayType(DecayLinear)
		linScore := scorer.RecencyScore(oldOffer.CreatedAt)

		scorer.SetDecayType(DecayLogarithmic)
		logScore := scorer.RecencyScore(oldOffer.CreatedAt)

		t.Logf("36h old item scores:")
		t.Logf("  Exponential: %.2f", expScore)
		t.Logf("  Linear: %.2f", linScore)
		t.Logf("  Logarithmic: %.2f", logScore)

		// Logarithmic should be highest (slowest decay)
		if logScore <= expScore {
			t.Errorf("Logarithmic should decay slower: log=%.2f, exp=%.2f", logScore, expScore)
		}

		// Reset to default
		scorer.SetDecayType(DecayExponential)
	})

	t.Run("Custom half-life affects scoring", func(t *testing.T) {
		testOffer := &entity.Offer{
			Medication: "Test",
			Quantity:   100,
			Price:      100,
			CreatedAt:  now.Add(-12 * time.Hour),
		}

		// Default 24h half-life
		scorer.SetRecencyHalfLife(24.0)
		defaultScore := scorer.RecencyScore(testOffer.CreatedAt)

		// Shorter 12h half-life (faster decay)
		scorer.SetRecencyHalfLife(12.0)
		shortScore := scorer.RecencyScore(testOffer.CreatedAt)

		t.Logf("12h old item with different half-lives:")
		t.Logf("  24h half-life: %.2f", defaultScore)
		t.Logf("  12h half-life: %.2f", shortScore)

		// Shorter half-life should score lower (at half-life point)
		if defaultScore <= shortScore {
			t.Errorf("Shorter half-life should decay faster: default=%.2f, short=%.2f", defaultScore, shortScore)
		}

		// Reset
		scorer.SetRecencyHalfLife(24.0)
	})
}

// TestEndToEnd_RealisticScenario tests a realistic medication matching scenario
func TestEndToEnd_RealisticScenario(t *testing.T) {
	scorer := NewScorer(nil, nil)
	now := time.Now()

	// Realistic request
	request := &entity.Request{
		ID:         "req-diabetes",
		Medication: "Ozempic 1mg",
		Quantity:   10,
		MaxPrice:   500,
	}

	// Various realistic offers
	scenarios := []struct {
		name     string
		offer    *entity.Offer
		expected ConfidenceBand
	}{
		{
			name: "Perfect match - recent, exact",
			offer: &entity.Offer{
				Medication: "Ozempic 1mg",
				Quantity:   10,
				Price:      480,
				CreatedAt:  now.Add(-30 * time.Minute),
			},
			expected: ConfidenceAuto,
		},
		{
			name: "Good match - within tolerances",
			offer: &entity.Offer{
				Medication: "Ozempic 1mg",
				Quantity:   9,   // 90% - within tolerance
				Price:      520, // 104% - within tolerance
				CreatedAt:  now.Add(-6 * time.Hour),
			},
			expected: ConfidenceAuto,
		},
		{
			name: "Moderate match - slight differences",
			offer: &entity.Offer{
				Medication: "Ozempic 1mg",
				Quantity:   8,   // 80% - below tolerance
				Price:      550, // 110% - over budget
				CreatedAt:  now.Add(-18 * time.Hour),
			},
			expected: ConfidenceAuto,
		},
		{
			name: "Different dosage - still matches",
			offer: &entity.Offer{
				Medication: "Ozempic 0.5mg", // Different dosage
				Quantity:   10,
				Price:      450,
				CreatedAt:  now.Add(-2 * time.Hour),
			},
			expected: ConfidenceSuggest,
		},
	}

	for _, tc := range scenarios {
		t.Run(tc.name, func(t *testing.T) {
			medScore := 1.0
			if tc.name == "Different dosage - still matches" {
				medScore = 0.9 // Lower med score for different dosage
			}

			result := scorer.ScoreMatch(tc.offer, request, medScore)

			t.Logf("Scenario: %s", tc.name)
			t.Logf("  Scores: Med=%.2f, Dosage=%.2f, Qty=%.2f, Price=%.2f, Recency=%.2f",
				result.MedicationScore, result.DosageScore, result.QuantityScore,
				result.PriceScore, result.RecencyScore)
			t.Logf("  Total: %.2f (%s)", result.Total, result.Confidence)
			t.Logf("  Breakdown: %s", result.Breakdown)

			if result.Confidence != tc.expected {
				t.Errorf("Expected confidence %s, got %s (score: %.2f)",
					tc.expected, result.Confidence, result.Total)
			}
		})
	}
}

// TestEndToEnd_WeightAdjustment tests dynamic weight adjustment
func TestEndToEnd_WeightAdjustment(t *testing.T) {
	scorer := NewScorer(nil, nil)
	now := time.Now()

	offer := &entity.Offer{
		Medication: "Test Med",
		Quantity:   100,
		Price:      100,
		CreatedAt:  now.Add(-12 * time.Hour),
	}

	request := &entity.Request{
		Medication: "Test Med",
		Quantity:   100,
		MaxPrice:   100,
	}

	// Default weights
	defaultResult := scorer.ScoreMatch(offer, request, 1.0)
	t.Logf("Default weights - Total: %.3f", defaultResult.Total)

	// Emphasize recency (for fast-moving market)
	customWeights := Weights{
		Medication: 0.35,
		Dosage:     0.05,
		Quantity:   0.15,
		Price:      0.10,
		Recency:    0.35, // Higher recency weight
	}
	scorer.UpdateWeights(customWeights)

	recencyResult := scorer.ScoreMatch(offer, request, 1.0)
	t.Logf("High recency weight - Total: %.3f", recencyResult.Total)

	// Recency-focused should score lower for 12h old item
	if recencyResult.Total >= defaultResult.Total {
		t.Errorf("Higher recency weight should lower score for old item: default=%.3f, recency=%.3f",
			defaultResult.Total, recencyResult.Total)
	}
}
