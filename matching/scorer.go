package matching

import (
	"fmt"
	"math"
	"strings"
	"sync"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/dosage"
)

// MatchScore represents the detailed breakdown of a match
type MatchScore struct {
	MedicationScore float64        `json:"medication_score"` // 0-1
	DosageScore     float64        `json:"dosage_score"`     // 0-1
	QuantityScore   float64        `json:"quantity_score"`   // 0-1
	PriceScore      float64        `json:"price_score"`      // 0-1
	RecencyScore    float64        `json:"recency_score"`    // 0-1
	Total           float64        `json:"total"`            // Weighted sum
	Confidence      ConfidenceBand `json:"confidence"`       // Band classification
	Breakdown       string         `json:"breakdown"`        // Human-readable explanation
}

// Scorer provides multi-field scoring for offer-request matching
type Scorer struct {
	mu                    sync.RWMutex
	weights               Weights
	thresholds            Thresholds
	recencyHalfLife       float64   // Hours until score decays to 50% (default: 24)
	decayType             DecayType // Type of decay curve (default: Exponential)
	semanticWeight        float64   // Alpha for semantic vs lexical balance (Phase 2)
	minMedicationScore    float64   // Minimum medication score to consider a match (default: 0.5)
	medicationGateEnabled bool      // If true, reject matches below minMedicationScore
}

// NewScorer creates a new Scorer with the given configuration
func NewScorer(weights *Weights, thresholds *Thresholds) *Scorer {
	w := DefaultWeights()
	if weights != nil {
		w = *weights
	}

	t := DefaultThresholds()
	if thresholds != nil {
		t = *thresholds
	}

	return &Scorer{
		weights:               w,
		thresholds:            t,
		recencyHalfLife:       24.0,             // Default 24 hours
		decayType:             DecayExponential, // Default exponential
		semanticWeight:        0.6,              // Default: 60% semantic, 40% lexical
		minMedicationScore:    0.5,              // Default: require at least 50% medication match
		medicationGateEnabled: true,             // Default: enabled - medication must match
	}
}

func (s *Scorer) GetSemanticWeight() float64 {
	return s.semanticWeight
}

// QuantityScore calculates how well the offer quantity satisfies the request
// Returns 1.0 if offer is within ±10% of request, or has more than requested
// Otherwise returns the fulfillment ratio
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

	// NEW: Accept ±10% as perfect match (90% to 110% of requested)
	// This accounts for rounding and realistic availability
	if ratio >= 0.9 && ratio <= 1.1 {
		return 1.0
	}

	// Over-fulfillment beyond tolerance is still perfect
	if ratio > 1.1 {
		return 1.0
	}

	// Under-fulfillment: return ratio (e.g., 80% available = 0.8 score)
	return ratio
}

// PriceScore calculates how well the offer price matches the request's max price
// Returns 1.0 if within budget (±5% tolerance), decays linearly for prices above tolerance
// Rewards better prices with bonus scores
func (s *Scorer) PriceScore(offerPrice, maxPrice float64) float64 {
	// If request doesn't specify max price, any price is acceptable
	if maxPrice <= 0 {
		// If offer also has no price, neutral
		if offerPrice <= 0 {
			return 1.0
		}
		// Offer has price but no budget constraint - slightly favor this
		return 0.95 // Small penalty for uncertainty
	}

	// If offer has no price, treat as neutral with moderate penalty
	if offerPrice <= 0 {
		return 0.85 // Moderate penalty for unknown price when budget exists
	}

	// Calculate price ratio
	ratio := offerPrice / maxPrice

	// NEW: ±5% tolerance for price fluctuations
	// Prices between 95% and 105% of max budget are perfect
	if ratio >= 0.95 && ratio <= 1.05 {
		return 1.0
	}

	// Bonus for significantly cheaper (below 95% of budget)
	// Better deals get slight bonus up to 1.0
	if ratio < 0.95 {
		// Already at max score for cheaper prices
		return 1.0
	}

	// Above tolerance (>105% of budget): linear decay
	// 105% -> 1.0, 205% -> 0.0
	// Formula: 1.0 - (ratio - 1.05) / 1.0
	overage := ratio - 1.05
	score := 1.0 - overage

	return math.Max(0, score)
}

// RecencyScore calculates a score based on how recent the item is
// Uses the configured decay type and half-life
func (s *Scorer) RecencyScore(createdAt time.Time) float64 {
	return s.RecencyScoreWithParams(createdAt, s.recencyHalfLife, s.decayType)
}

// RecencyScoreWithHalfLife calculates recency score with custom half-life in hours
// Uses the configured decay type
func (s *Scorer) RecencyScoreWithHalfLife(createdAt time.Time, halfLifeHours float64) float64 {
	return s.RecencyScoreWithParams(createdAt, halfLifeHours, s.decayType)
}

// RecencyScoreWithParams calculates recency score with full customization
func (s *Scorer) RecencyScoreWithParams(createdAt time.Time, halfLifeHours float64, decayType DecayType) float64 {
	age := time.Since(createdAt).Hours()
	if age <= 0 {
		return 1.0
	}

	switch decayType {
	case DecayLinear:
		// Linear decay: 1 - (age / maxAge)
		// Reaches 0 at 2x halfLife
		maxAge := halfLifeHours * 2
		if age >= maxAge {
			return 0.0
		}
		return 1.0 - (age / maxAge)

	case DecayLogarithmic:
		// Logarithmic decay: slower decay over time
		// Uses square root for slower, smoother decay
		maxAge := halfLifeHours * 4 // Longer effective range
		if age >= maxAge {
			return 0.1 // Keep minimum score longer
		}
		// Use sqrt for slower decay
		ratio := age / maxAge
		score := math.Sqrt(1.0 - ratio)
		return math.Max(0.1, score)

	default: // DecayExponential
		// Exponential decay: score = e^(-λt) where λ = ln(2)/halfLife
		lambda := 0.693 / halfLifeHours // ln(2) ≈ 0.693
		return math.Exp(-lambda * age)
	}
}

// DosageScore compares dosages between offer and request medications
// Returns 1.0 for equivalent dosages, 0 for  very different, uses CompareDosages internally
func (s *Scorer) DosageScore(offerMedication, requestMedication string) float64 {
	offerDosage := dosage.ParseDosage(offerMedication)
	requestDosage := dosage.ParseDosage(requestMedication)

	// If neither has dosage info, consider neutral (don't penalize)
	if offerDosage == nil && requestDosage == nil {
		return 0.9 // Slight penalty for missing dosage info
	}

	// If only one has dosage, partial penalty
	if offerDosage == nil || requestDosage == nil {
		return 0.7 // Dosage info mismatch
	}

	// Both have dosages - compare them
	return dosage.CompareDosages(offerDosage, requestDosage)
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
func (s *Scorer) ScoreMatch(offer *entity.Offer, request *entity.Request, medicationScore float64) *MatchScore {
	// MEDICATION GATE: If medication score is too low, reject the match entirely
	// This ensures medication name is the dominant factor - no match without medication match
	if s.medicationGateEnabled && medicationScore < s.minMedicationScore {
		return &MatchScore{
			MedicationScore: medicationScore,
			DosageScore:     0,
			QuantityScore:   0,
			PriceScore:      0,
			RecencyScore:    0,
			Total:           0,
			Confidence:      ConfidenceNone,
			Breakdown:       fmt.Sprintf("Medication mismatch (%.0f%% < %.0f%% required)", medicationScore*100, s.minMedicationScore*100),
		}
	}

	// Calculate individual scores
	dosageScore := s.DosageScore(offer.Medication, request.Medication)
	qtyScore := s.QuantityScore(offer.Quantity, request.Quantity)
	priceScore := s.PriceScore(offer.Price, request.MaxPrice)
	recencyScore := s.RecencyScore(offer.CreatedAt)

	// Calculate weighted total
	total := s.weights.Medication*medicationScore +
		s.weights.Dosage*dosageScore +
		s.weights.Quantity*qtyScore +
		s.weights.Price*priceScore +
		s.weights.Recency*recencyScore

	// Clamp to [0, 1]
	total = math.Max(0, math.Min(1, total))

	// Get confidence band
	confidence := s.GetConfidenceBand(total)

	// Generate breakdown explanation
	breakdown := s.generateBreakdown(medicationScore, dosageScore, qtyScore, priceScore, recencyScore, total)

	return &MatchScore{
		MedicationScore: medicationScore,
		DosageScore:     dosageScore,
		QuantityScore:   qtyScore,
		PriceScore:      priceScore,
		RecencyScore:    recencyScore,
		Total:           total,
		Confidence:      confidence,
		Breakdown:       breakdown,
	}
}

// generateBreakdown creates a human-readable explanation of the score
func (s *Scorer) generateBreakdown(medScore, dosageScore, qtyScore, priceScore, recencyScore, _ float64) string {
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

	// Dosage match quality
	switch {
	case dosageScore >= 0.95:
		parts = append(parts, "Exact dosage match")
	case dosageScore >= 0.8:
		parts = append(parts, "Similar dosage")
	case dosageScore >= 0.6:
		parts = append(parts, "Different dosage")
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
// Thread-safe for runtime updates from scheduler.
func (s *Scorer) UpdateWeights(weights Weights) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.weights = weights
}

// UpdateThresholds allows dynamic threshold adjustment
func (s *Scorer) UpdateThresholds(thresholds Thresholds) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.thresholds = thresholds
}

// GetWeights returns the current scoring weights
// Thread-safe for concurrent access.
func (s *Scorer) GetWeights() Weights {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.weights
}

// GetThresholds returns the current confidence thresholds
// Thread-safe for concurrent access.
func (s *Scorer) GetThresholds() Thresholds {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.thresholds
}

// SetRecencyHalfLife sets the half-life for recency decay (in hours)
func (s *Scorer) SetRecencyHalfLife(hours float64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.recencyHalfLife = hours
}

// GetRecencyHalfLife returns the current recency half-life
// Thread-safe for concurrent access.
func (s *Scorer) GetRecencyHalfLife() float64 {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.recencyHalfLife
}

// SetDecayType sets the type of decay curve for recency scoring
func (s *Scorer) SetDecayType(decayType DecayType) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.decayType = decayType
}

// GetDecayType returns the current decay type
func (s *Scorer) GetDecayType() DecayType {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.decayType
}

// =============================================================================
// Medication Gate Configuration
// =============================================================================

// SetMinMedicationScore sets the minimum medication score required for a match.
// Matches with medication scores below this threshold will be rejected.
func (s *Scorer) SetMinMedicationScore(minScore float64) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if minScore < 0 {
		minScore = 0
	}
	if minScore > 1 {
		minScore = 1
	}
	s.minMedicationScore = minScore
}

// GetMinMedicationScore returns the current minimum medication score threshold.
func (s *Scorer) GetMinMedicationScore() float64 {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.minMedicationScore
}

// EnableMedicationGate enables or disables the medication gate.
// When enabled, matches with medication scores below minMedicationScore are rejected.
func (s *Scorer) EnableMedicationGate(enabled bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.medicationGateEnabled = enabled
}

// IsMedicationGateEnabled returns whether the medication gate is enabled.
func (s *Scorer) IsMedicationGateEnabled() bool {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.medicationGateEnabled
}
