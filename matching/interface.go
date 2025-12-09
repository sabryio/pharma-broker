// Package matching provides offer-request matching service interfaces.
package matching

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
)

// Service defines the matching service interface
type Service interface {
	// FindMatches finds matching offers for a request
	FindMatches(ctx context.Context, request *entity.Request) ([]*Match, error)

	// FindMatchesForOffer finds matching requests for an offer
	FindMatchesForOffer(ctx context.Context, offer *entity.Offer) ([]*Match, error)

	// ScoreMatch scores a specific offer-request pair
	ScoreMatch(offer *entity.Offer, request *entity.Request) *Score

	// ProcessQueue processes pending items from match queue
	ProcessQueue(ctx context.Context, batchSize int) error
}

// Match represents a potential match between offer and request
type Match struct {
	OfferID   string
	RequestID string
	Score     *Score
}

// Score represents detailed match scoring breakdown
type Score struct {
	Total           float64
	MedicationScore float64
	DosageScore     float64
	QuantityScore   float64
	PriceScore      float64
	RecencyScore    float64
	Confidence      ConfidenceBand
	Breakdown       string
}

// ConfidenceBand categorizes match quality
type ConfidenceBand string

const (
	ConfidenceAuto    ConfidenceBand = "AUTO"    // >= 0.9 - Auto-confirm
	ConfidenceSuggest ConfidenceBand = "SUGGEST" // 0.7 - 0.9 - Suggest to operator
	ConfidenceReview  ConfidenceBand = "REVIEW"  // 0.5 - 0.7 - Needs manual review
	ConfidenceNone    ConfidenceBand = "NONE"    // < 0.5 - No match
)

// Weights holds configurable weights for each scoring field
type Weights struct {
	Medication float64
	Dosage     float64
	Quantity   float64
	Price      float64
	Recency    float64
}

// DefaultWeights returns the default scoring weights
func DefaultWeights() Weights {
	return Weights{
		Medication: 0.40, // Medication match is most important
		Dosage:     0.15, // Correct dosage matters
		Quantity:   0.15, // Quantity fulfillment
		Price:      0.15, // Price within budget
		Recency:    0.15, // Prefer fresh listings
	}
}

// Thresholds defines score boundaries for each confidence band
type Thresholds struct {
	Auto    float64 // >= this -> AUTO
	Suggest float64 // >= this -> SUGGEST
	Review  float64 // >= this -> REVIEW
}

// DefaultThresholds returns the default confidence thresholds
func DefaultThresholds() Thresholds {
	return Thresholds{
		Auto:    0.90,
		Suggest: 0.70,
		Review:  0.50,
	}
}

// DecayType defines the type of recency decay curve
type DecayType string

const (
	DecayExponential DecayType = "EXPONENTIAL" // e^(-λt) - Default, natural decay
	DecayLinear      DecayType = "LINEAR"      // 1 - t/max - Constant rate
	DecayLogarithmic DecayType = "LOGARITHMIC" // 1 - log(t+1)/log(max) - Slower decay
)

// Config holds matching configuration
type Config struct {
	Weights         Weights
	Thresholds      Thresholds
	RecencyHalfLife time.Duration
	DecayType       DecayType
}

// DefaultConfig returns the default matching configuration
func DefaultConfig() Config {
	return Config{
		Weights:         DefaultWeights(),
		Thresholds:      DefaultThresholds(),
		RecencyHalfLife: 24 * time.Hour,
		DecayType:       DecayExponential,
	}
}
