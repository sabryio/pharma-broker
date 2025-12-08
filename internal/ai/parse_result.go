package ai

// ParsePass indicates which parsing pass extracted the result
type ParsePass int

const (
	ParsePassStrict  ParsePass = 1 // Full structured extraction with strict validation
	ParsePassRelaxed ParsePass = 2 // Simplified prompt, lenient parsing
	ParsePassReview  ParsePass = 3 // Failed both passes, needs manual review
)

// String returns human-readable pass name
func (p ParsePass) String() string {
	switch p {
	case ParsePassStrict:
		return "STRICT"
	case ParsePassRelaxed:
		return "RELAXED"
	case ParsePassReview:
		return "REVIEW"
	default:
		return "UNKNOWN"
	}
}

// ParseConfidence represents overall extraction confidence level
type ParseConfidence string

const (
	ParseConfidenceHigh   ParseConfidence = "HIGH"   // >= 0.8 average AI confidence
	ParseConfidenceMedium ParseConfidence = "MEDIUM" // 0.5-0.79 average AI confidence
	ParseConfidenceLow    ParseConfidence = "LOW"    // < 0.5 average AI confidence
	ParseConfidenceFailed ParseConfidence = "FAILED" // Parsing failed entirely
)

// GetConfidenceLevelForScore returns the confidence level for a given score
func GetConfidenceLevelForScore(avgConfidence float64) ParseConfidence {
	switch {
	case avgConfidence >= 0.8:
		return ParseConfidenceHigh
	case avgConfidence >= 0.5:
		return ParseConfidenceMedium
	case avgConfidence > 0:
		return ParseConfidenceLow
	default:
		return ParseConfidenceFailed
	}
}

// MultiPassConfig configures multi-pass parsing behavior
type MultiPassConfig struct {
	// StrictMinConfidence is the minimum average confidence to accept Pass 1 results
	StrictMinConfidence float64 // Default: 0.7

	// RelaxedMinConfidence is the minimum confidence to accept Pass 2 results
	RelaxedMinConfidence float64 // Default: 0.4

	// EnablePass2 enables the relaxed fallback pass
	EnablePass2 bool // Default: true

	// EnableReviewQueue enables queuing low-confidence results for review
	EnableReviewQueue bool // Default: true
}

// DefaultMultiPassConfig returns sensible defaults
func DefaultMultiPassConfig() MultiPassConfig {
	return MultiPassConfig{
		StrictMinConfidence:  0.7,
		RelaxedMinConfidence: 0.4,
		EnablePass2:          true,
		EnableReviewQueue:    true,
	}
}
