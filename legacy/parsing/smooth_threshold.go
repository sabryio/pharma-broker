package parsing

import (
	"math"
	"sync"

	"github.com/rs/zerolog"

	"pharmabroker/matching"
)

// =============================================================================
// Smooth Threshold Configuration
// =============================================================================

// SmoothThresholdConfig holds configuration for smooth threshold transitions.
type SmoothThresholdConfig struct {
	// Transition zone width (as fraction of threshold gap)
	TransitionWidth float64 // Default: 0.1 (10% of gap between thresholds)

	// Enable smooth transitions
	EnableSmoothing bool // Default: true

	// Thresholds (from matching package)
	Thresholds matching.Thresholds
}

// DefaultSmoothThresholdConfig returns sensible defaults.
func DefaultSmoothThresholdConfig() SmoothThresholdConfig {
	return SmoothThresholdConfig{
		TransitionWidth: DefaultTransitionWidth,
		EnableSmoothing: true,
		Thresholds:      matching.DefaultThresholds(),
	}
}

// =============================================================================
// Smooth Confidence Result
// =============================================================================

// SmoothConfidenceResult contains the result of smooth confidence calculation.
type SmoothConfidenceResult struct {
	// Primary band (the band the score falls into)
	PrimaryBand matching.ConfidenceBand

	// Raw score (original score)
	RawScore float64

	// Smooth confidence (0-1, interpolated within band)
	SmoothConfidence float64

	// Band strength (0-1, how strongly in this band vs adjacent)
	BandStrength float64

	// Transition info
	InTransitionZone   bool
	NearBoundary       string // "upper", "lower", or ""
	DistanceToBoundary float64
}

// =============================================================================
// Smooth Threshold Calculator
// =============================================================================

// SmoothThresholdCalculator provides smooth confidence transitions.
type SmoothThresholdCalculator struct {
	config SmoothThresholdConfig
	log    zerolog.Logger
	mu     sync.RWMutex
}

// NewSmoothThresholdCalculator creates a new smooth threshold calculator.
func NewSmoothThresholdCalculator(cfg SmoothThresholdConfig, log zerolog.Logger) *SmoothThresholdCalculator {
	if cfg.TransitionWidth <= 0 || cfg.TransitionWidth > 0.5 {
		cfg.TransitionWidth = DefaultTransitionWidth
	}

	return &SmoothThresholdCalculator{
		config: cfg,
		log:    log.With().Str("component", "smooth-threshold").Logger(),
	}
}

// CalculateSmoothConfidence calculates smooth confidence with transition handling.
func (stc *SmoothThresholdCalculator) CalculateSmoothConfidence(score float64) SmoothConfidenceResult {
	stc.mu.RLock()
	defer stc.mu.RUnlock()

	// Get primary band using standard thresholds
	primaryBand := stc.getPrimaryBand(score)

	result := SmoothConfidenceResult{
		PrimaryBand: primaryBand,
		RawScore:    score,
	}

	if !stc.config.EnableSmoothing {
		// No smoothing - return hard boundaries
		result.SmoothConfidence = score
		result.BandStrength = 1.0
		return result
	}

	// Calculate smooth confidence based on position within band
	switch primaryBand {
	case matching.ConfidenceAuto:
		result = stc.calculateAutoZone(score, result)
	case matching.ConfidenceSuggest:
		result = stc.calculateSuggestZone(score, result)
	case matching.ConfidenceReview:
		result = stc.calculateReviewZone(score, result)
	default:
		result = stc.calculateNoneZone(score, result)
	}

	return result
}

// getPrimaryBand returns the primary confidence band for a score.
func (stc *SmoothThresholdCalculator) getPrimaryBand(score float64) matching.ConfidenceBand {
	switch {
	case score >= stc.config.Thresholds.Auto:
		return matching.ConfidenceAuto
	case score >= stc.config.Thresholds.Suggest:
		return matching.ConfidenceSuggest
	case score >= stc.config.Thresholds.Review:
		return matching.ConfidenceReview
	default:
		return matching.ConfidenceNone
	}
}

// calculateAutoZone handles scores in the AUTO band.
func (stc *SmoothThresholdCalculator) calculateAutoZone(score float64, result SmoothConfidenceResult) SmoothConfidenceResult {
	autoThreshold := stc.config.Thresholds.Auto
	transitionWidth := (1.0 - autoThreshold) * stc.config.TransitionWidth

	// Distance from lower boundary
	distanceFromLower := score - autoThreshold

	if distanceFromLower < transitionWidth {
		// In transition zone near lower boundary
		result.InTransitionZone = true
		result.NearBoundary = "lower"
		result.DistanceToBoundary = distanceFromLower

		// Smooth interpolation using sigmoid-like curve
		t := distanceFromLower / transitionWidth
		result.BandStrength = smoothStep(t)
		result.SmoothConfidence = autoThreshold + (score-autoThreshold)*result.BandStrength
	} else {
		// Fully in AUTO zone
		result.BandStrength = 1.0
		result.SmoothConfidence = score
	}

	return result
}

// calculateSuggestZone handles scores in the SUGGEST band.
func (stc *SmoothThresholdCalculator) calculateSuggestZone(score float64, result SmoothConfidenceResult) SmoothConfidenceResult {
	suggestThreshold := stc.config.Thresholds.Suggest
	autoThreshold := stc.config.Thresholds.Auto
	bandWidth := autoThreshold - suggestThreshold
	transitionWidth := bandWidth * stc.config.TransitionWidth

	// Distance from boundaries
	distanceFromLower := score - suggestThreshold
	distanceFromUpper := autoThreshold - score

	if distanceFromUpper < transitionWidth {
		// Near upper boundary (approaching AUTO)
		result.InTransitionZone = true
		result.NearBoundary = "upper"
		result.DistanceToBoundary = distanceFromUpper

		t := distanceFromUpper / transitionWidth
		result.BandStrength = smoothStep(t)
		// Blend towards AUTO threshold
		result.SmoothConfidence = score + (autoThreshold-score)*(1-result.BandStrength)*0.5
	} else if distanceFromLower < transitionWidth {
		// Near lower boundary (approaching REVIEW)
		result.InTransitionZone = true
		result.NearBoundary = "lower"
		result.DistanceToBoundary = distanceFromLower

		t := distanceFromLower / transitionWidth
		result.BandStrength = smoothStep(t)
		// Blend towards REVIEW threshold
		result.SmoothConfidence = suggestThreshold + (score-suggestThreshold)*result.BandStrength
	} else {
		// Fully in SUGGEST zone
		result.BandStrength = 1.0
		result.SmoothConfidence = score
	}

	return result
}

// calculateReviewZone handles scores in the REVIEW band.
func (stc *SmoothThresholdCalculator) calculateReviewZone(score float64, result SmoothConfidenceResult) SmoothConfidenceResult {
	reviewThreshold := stc.config.Thresholds.Review
	suggestThreshold := stc.config.Thresholds.Suggest
	bandWidth := suggestThreshold - reviewThreshold
	transitionWidth := bandWidth * stc.config.TransitionWidth

	// Distance from boundaries
	distanceFromLower := score - reviewThreshold
	distanceFromUpper := suggestThreshold - score

	if distanceFromUpper < transitionWidth {
		// Near upper boundary (approaching SUGGEST)
		result.InTransitionZone = true
		result.NearBoundary = "upper"
		result.DistanceToBoundary = distanceFromUpper

		t := distanceFromUpper / transitionWidth
		result.BandStrength = smoothStep(t)
		result.SmoothConfidence = score + (suggestThreshold-score)*(1-result.BandStrength)*0.5
	} else if distanceFromLower < transitionWidth {
		// Near lower boundary (approaching NONE)
		result.InTransitionZone = true
		result.NearBoundary = "lower"
		result.DistanceToBoundary = distanceFromLower

		t := distanceFromLower / transitionWidth
		result.BandStrength = smoothStep(t)
		result.SmoothConfidence = reviewThreshold + (score-reviewThreshold)*result.BandStrength
	} else {
		// Fully in REVIEW zone
		result.BandStrength = 1.0
		result.SmoothConfidence = score
	}

	return result
}

// calculateNoneZone handles scores in the NONE band.
func (stc *SmoothThresholdCalculator) calculateNoneZone(score float64, result SmoothConfidenceResult) SmoothConfidenceResult {
	reviewThreshold := stc.config.Thresholds.Review
	transitionWidth := reviewThreshold * stc.config.TransitionWidth

	// Distance from upper boundary
	distanceFromUpper := reviewThreshold - score

	if distanceFromUpper < transitionWidth && score > 0 {
		// Near upper boundary (approaching REVIEW)
		result.InTransitionZone = true
		result.NearBoundary = "upper"
		result.DistanceToBoundary = distanceFromUpper

		t := distanceFromUpper / transitionWidth
		result.BandStrength = smoothStep(t)
		result.SmoothConfidence = score * result.BandStrength
	} else {
		// Fully in NONE zone
		result.BandStrength = 1.0
		result.SmoothConfidence = score
	}

	return result
}

// smoothStep provides smooth interpolation using Hermite curve (3t² - 2t³)
// This creates smooth transitions without sharp edges.
func smoothStep(t float64) float64 {
	// Clamp t to [0, 1]
	if t < 0 {
		t = 0
	}
	if t > 1 {
		t = 1
	}
	// Hermite interpolation: 3t² - 2t³
	return t * t * (3 - 2*t)
}

// SmootherStep provides even smoother interpolation (6t⁵ - 15t⁴ + 10t³)
// Ken Perlin's improved smoothstep.
func SmootherStep(t float64) float64 {
	if t < 0 {
		t = 0
	}
	if t > 1 {
		t = 1
	}
	// Perlin's smootherstep: 6t⁵ - 15t⁴ + 10t³
	return t * t * t * (t*(t*6-15) + 10)
}

// =============================================================================
// Adjusted Action Based on Smooth Confidence
// =============================================================================

// GetAdjustedAction returns an adjusted action based on smooth confidence.
// This helps prevent cliff effects by considering transition zones.
func (stc *SmoothThresholdCalculator) GetAdjustedAction(score float64) AdjustedActionResult {
	smooth := stc.CalculateSmoothConfidence(score)

	result := AdjustedActionResult{
		PrimaryBand:      smooth.PrimaryBand,
		RawScore:         score,
		SmoothConfidence: smooth.SmoothConfidence,
		BandStrength:     smooth.BandStrength,
		InTransitionZone: smooth.InTransitionZone,
	}

	// Determine if action should be adjusted based on transition zone
	if smooth.InTransitionZone && smooth.BandStrength < 0.5 {
		// Weak band strength - consider adjacent band
		result.ShouldConsiderAdjacent = true

		switch smooth.PrimaryBand {
		case matching.ConfidenceAuto:
			if smooth.NearBoundary == "lower" {
				result.AdjacentBand = matching.ConfidenceSuggest
				result.Recommendation = "Consider manual review - near AUTO/SUGGEST boundary"
			}
		case matching.ConfidenceSuggest:
			switch smooth.NearBoundary {
			case "upper":
				result.AdjacentBand = matching.ConfidenceAuto
				result.Recommendation = "Strong candidate for auto-confirm"
			case "lower":
				result.AdjacentBand = matching.ConfidenceReview
				result.Recommendation = "May need additional review"
			}
		case matching.ConfidenceReview:
			switch smooth.NearBoundary {
			case "upper":
				result.AdjacentBand = matching.ConfidenceSuggest
				result.Recommendation = "Consider suggesting to operator"
			case "lower":
				result.AdjacentBand = matching.ConfidenceNone
				result.Recommendation = "Borderline match - careful review needed"
			}
		case matching.ConfidenceNone:
			if smooth.NearBoundary == "upper" {
				result.AdjacentBand = matching.ConfidenceReview
				result.Recommendation = "Borderline - may warrant review"
			}
		}
	}

	return result
}

// AdjustedActionResult contains the result of adjusted action calculation.
type AdjustedActionResult struct {
	PrimaryBand            matching.ConfidenceBand
	AdjacentBand           matching.ConfidenceBand
	RawScore               float64
	SmoothConfidence       float64
	BandStrength           float64
	InTransitionZone       bool
	ShouldConsiderAdjacent bool
	Recommendation         string
}

// =============================================================================
// Configuration Methods
// =============================================================================

// GetConfig returns the current configuration.
func (stc *SmoothThresholdCalculator) GetConfig() SmoothThresholdConfig {
	stc.mu.RLock()
	defer stc.mu.RUnlock()
	return stc.config
}

// SetConfig updates the configuration.
func (stc *SmoothThresholdCalculator) SetConfig(cfg SmoothThresholdConfig) {
	stc.mu.Lock()
	defer stc.mu.Unlock()
	stc.config = cfg
	stc.log.Info().
		Float64("transition_width", cfg.TransitionWidth).
		Bool("smoothing_enabled", cfg.EnableSmoothing).
		Msg("Smooth threshold configuration updated")
}

// SetTransitionWidth sets the transition zone width.
func (stc *SmoothThresholdCalculator) SetTransitionWidth(width float64) {
	if width > 0 && width <= 0.5 {
		stc.mu.Lock()
		stc.config.TransitionWidth = width
		stc.mu.Unlock()
		stc.log.Info().
			Float64("transition_width", width).
			Msg("Transition width updated")
	}
}

// EnableSmoothing enables or disables smooth transitions.
func (stc *SmoothThresholdCalculator) EnableSmoothing(enabled bool) {
	stc.mu.Lock()
	stc.config.EnableSmoothing = enabled
	stc.mu.Unlock()
	stc.log.Info().
		Bool("enabled", enabled).
		Msg("Smooth transitions toggled")
}

// SetThresholds updates the threshold values.
func (stc *SmoothThresholdCalculator) SetThresholds(thresholds matching.Thresholds) {
	stc.mu.Lock()
	stc.config.Thresholds = thresholds
	stc.mu.Unlock()
	stc.log.Info().
		Float64("auto", thresholds.Auto).
		Float64("suggest", thresholds.Suggest).
		Float64("review", thresholds.Review).
		Msg("Thresholds updated")
}

// =============================================================================
// Utility Functions
// =============================================================================

// GetBandRange returns the score range for a given band.
func (stc *SmoothThresholdCalculator) GetBandRange(band matching.ConfidenceBand) (min, max float64) {
	stc.mu.RLock()
	defer stc.mu.RUnlock()

	switch band {
	case matching.ConfidenceAuto:
		return stc.config.Thresholds.Auto, 1.0
	case matching.ConfidenceSuggest:
		return stc.config.Thresholds.Suggest, stc.config.Thresholds.Auto
	case matching.ConfidenceReview:
		return stc.config.Thresholds.Review, stc.config.Thresholds.Suggest
	default:
		return 0, stc.config.Thresholds.Review
	}
}

// GetTransitionZone returns the transition zone boundaries for a threshold.
func (stc *SmoothThresholdCalculator) GetTransitionZone(threshold float64) (lower, upper float64) {
	stc.mu.RLock()
	defer stc.mu.RUnlock()

	halfWidth := threshold * stc.config.TransitionWidth / 2
	return math.Max(0, threshold-halfWidth), math.Min(1, threshold+halfWidth)
}
