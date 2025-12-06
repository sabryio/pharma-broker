package ai

import (
	"fmt"
	"math"
	"strings"
	"time"

	"pharmabroker/internal/domain"
)

// ScoringWeights holds configurable weights for each scoring field
type ScoringWeights struct {
	Medication float64 `json:"medication"` // Default: 0.5
	Quantity   float64 `json:"quantity"`   // Default: 0.2
	Price      float64 `json:"price"`      // Default: 0.15
	Recency    float64 `json:"recency"`    // Default: 0.15
}

// DefaultWeights returns the default scoring weights
func DefaultWeights() ScoringWeights {
	return ScoringWeights{
		Medication: 0.50,
		Quantity:   0.20,
		Price:      0.15,
		Recency:    0.15,
	}
}

// ConfidenceBand categorizes match quality
type ConfidenceBand string

const (
	ConfidenceAuto    ConfidenceBand = "AUTO"    // >= 0.9 - Auto-confirm
	ConfidenceSuggest ConfidenceBand = "SUGGEST" // 0.7 - 0.9 - Suggest to operator
	ConfidenceReview  ConfidenceBand = "REVIEW"  // 0.5 - 0.7 - Needs manual review
	ConfidenceNone    ConfidenceBand = "NONE"    // < 0.5 - No match
)

// ConfidenceThresholds defines the score boundaries for each band
type ConfidenceThresholds struct {
	Auto    float64 `json:"auto"`    // Default: 0.9
	Suggest float64 `json:"suggest"` // Default: 0.7
	Review  float64 `json:"review"`  // Default: 0.5
}

// DefaultThresholds returns the default confidence thresholds
func DefaultThresholds() ConfidenceThresholds {
	return ConfidenceThresholds{
		Auto:    0.90,
		Suggest: 0.70,
		Review:  0.50,
	}
}

// MatchScore represents the detailed breakdown of a match
type MatchScore struct {
	MedicationScore float64        `json:"medication_score"` // 0-1
	QuantityScore   float64        `json:"quantity_score"`   // 0-1
	PriceScore      float64        `json:"price_score"`      // 0-1
	RecencyScore    float64        `json:"recency_score"`    // 0-1
	Total           float64        `json:"total"`            // Weighted sum
	Confidence      ConfidenceBand `json:"confidence"`       // Band classification
	Breakdown       string         `json:"breakdown"`        // Human-readable explanation
}

// Scorer provides multi-field scoring for offer-request matching
type Scorer struct {
	weights    ScoringWeights
	thresholds ConfidenceThresholds
	// For hybrid scoring (Phase 2)
	semanticWeight float64 // Alpha for semantic vs lexical balance
}

// NewScorer creates a new Scorer with the given configuration
func NewScorer(weights *ScoringWeights, thresholds *ConfidenceThresholds) *Scorer {
	w := DefaultWeights()
	if weights != nil {
		w = *weights
	}

	t := DefaultThresholds()
	if thresholds != nil {
		t = *thresholds
	}

	return &Scorer{
		weights:        w,
		thresholds:     t,
		semanticWeight: 0.6, // Default: 60% semantic, 40% lexical
	}
}

// QuantityScore calculates how well the offer quantity satisfies the request
// Returns 1.0 if offer has enough or more, otherwise returns the ratio
func (s *Scorer) QuantityScore(offerQty, requestQty float64) float64 {
	// If request doesn't specify quantity, any amount is acceptable
	if requestQty <= 0 {
		return 1.0
	}

	// If offer has no quantity, it can't satisfy the request
	if offerQty <= 0 {
		return 0.0
	}

	// Calculate fulfillment ratio
	ratio := offerQty / requestQty
	if ratio >= 1.0 {
		return 1.0 // Full or over-fulfillment
	}
	return ratio // Partial fulfillment (0 to 1)
}

// PriceScore calculates how well the offer price matches the request's max price
// Returns 1.0 if within budget, decays linearly for prices above max
func (s *Scorer) PriceScore(offerPrice, maxPrice float64) float64 {
	// If request doesn't specify max price, any price is acceptable
	if maxPrice <= 0 {
		return 1.0
	}

	// If offer has no price, consider it neutral
	if offerPrice <= 0 {
		return 0.8 // Slight penalty for unknown price
	}

	// Within or at budget
	if offerPrice <= maxPrice {
		return 1.0
	}

	// Linear decay for prices above max (0% at 2x the max price)
	overage := (offerPrice - maxPrice) / maxPrice
	score := 1.0 - overage
	return math.Max(0, score)
}

// RecencyScore calculates a score based on how recent the item is
// Uses exponential decay with a configurable half-life (default 24 hours)
func (s *Scorer) RecencyScore(createdAt time.Time) float64 {
	return s.RecencyScoreWithHalfLife(createdAt, 24.0)
}

// RecencyScoreWithHalfLife calculates recency score with custom half-life in hours
func (s *Scorer) RecencyScoreWithHalfLife(createdAt time.Time, halfLifeHours float64) float64 {
	age := time.Since(createdAt).Hours()
	if age <= 0 {
		return 1.0
	}

	// Exponential decay: score = e^(-λt) where λ = ln(2)/halfLife
	lambda := 0.693 / halfLifeHours // ln(2) ≈ 0.693
	return math.Exp(-lambda * age)
}

// GetConfidenceBand returns the confidence band for a given total score
func (s *Scorer) GetConfidenceBand(score float64) ConfidenceBand {
	switch {
	case score >= s.thresholds.Auto:
		return ConfidenceAuto
	case score >= s.thresholds.Suggest:
		return ConfidenceSuggest
	case score >= s.thresholds.Review:
		return ConfidenceReview
	default:
		return ConfidenceNone
	}
}

// ScoreMatch calculates the full multi-field match score between an offer and request
func (s *Scorer) ScoreMatch(offer *domain.Offer, request *domain.Request, medicationScore float64) *MatchScore {
	// Calculate individual scores
	qtyScore := s.QuantityScore(offer.Quantity, request.Quantity)
	priceScore := s.PriceScore(offer.Price, request.MaxPrice)
	recencyScore := s.RecencyScore(offer.CreatedAt)

	// Calculate weighted total
	total := s.weights.Medication*medicationScore +
		s.weights.Quantity*qtyScore +
		s.weights.Price*priceScore +
		s.weights.Recency*recencyScore

	// Clamp to [0, 1]
	total = math.Max(0, math.Min(1, total))

	// Get confidence band
	confidence := s.GetConfidenceBand(total)

	// Generate breakdown explanation
	breakdown := s.generateBreakdown(medicationScore, qtyScore, priceScore, recencyScore, total)

	return &MatchScore{
		MedicationScore: medicationScore,
		QuantityScore:   qtyScore,
		PriceScore:      priceScore,
		RecencyScore:    recencyScore,
		Total:           total,
		Confidence:      confidence,
		Breakdown:       breakdown,
	}
}

// generateBreakdown creates a human-readable explanation of the score
func (s *Scorer) generateBreakdown(medScore, qtyScore, priceScore, recencyScore, total float64) string {
	var parts []string

	// Medication match quality
	switch {
	case medScore >= 0.95:
		parts = append(parts, "Exact medication match")
	case medScore >= 0.8:
		parts = append(parts, "Strong medication match")
	case medScore >= 0.6:
		parts = append(parts, "Moderate medication match")
	default:
		parts = append(parts, "Weak medication match")
	}

	// Quantity fulfillment
	switch {
	case qtyScore >= 1.0:
		parts = append(parts, "Quantity fully satisfied")
	case qtyScore >= 0.7:
		parts = append(parts, fmt.Sprintf("%.0f%% quantity available", qtyScore*100))
	default:
		parts = append(parts, "Insufficient quantity")
	}

	// Price check
	switch {
	case priceScore >= 1.0:
		parts = append(parts, "Within budget")
	case priceScore >= 0.8:
		parts = append(parts, "Slightly over budget")
	default:
		parts = append(parts, "Over budget")
	}

	// Recency
	switch {
	case recencyScore >= 0.9:
		parts = append(parts, "Very recent listing")
	case recencyScore >= 0.5:
		parts = append(parts, "Recent listing")
	default:
		parts = append(parts, "Older listing")
	}

	return strings.Join(parts, " • ")
}

// UpdateWeights allows dynamic weight adjustment (for feedback loop)
func (s *Scorer) UpdateWeights(weights ScoringWeights) {
	s.weights = weights
}

// UpdateThresholds allows dynamic threshold adjustment
func (s *Scorer) UpdateThresholds(thresholds ConfidenceThresholds) {
	s.thresholds = thresholds
}

// GetWeights returns the current scoring weights
func (s *Scorer) GetWeights() ScoringWeights {
	return s.weights
}

// GetThresholds returns the current confidence thresholds
func (s *Scorer) GetThresholds() ConfidenceThresholds {
	return s.thresholds
}
