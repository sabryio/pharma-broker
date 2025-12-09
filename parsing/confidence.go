package parsing

// AI Confidence Score Constants and Utilities
// These define thresholds for interpreting AI confidence in medication extraction

// ConfidenceLevel categorizes AI confidence scores
type ConfidenceLevel string

const (
	ConfidenceLevelHigh   ConfidenceLevel = "HIGH"   // ≥0.8 - Clear, unambiguous extraction
	ConfidenceLevelMedium ConfidenceLevel = "MEDIUM" // ≥0.5 - Some uncertainty
	ConfidenceLevelLow    ConfidenceLevel = "LOW"    // <0.5 - Needs review
)

// Confidence thresholds
const (
	ConfidenceThresholdHigh   = 0.8 // High confidence: clear medication, matched in map
	ConfidenceThresholdMedium = 0.5 // Medium confidence: spelling variations, unclear quantity
	ConfidenceThresholdLow    = 0.0 // Low confidence: unfamiliar medication, heavy transliteration
)

// GetConfidenceLevel categorizes a confidence score
func GetConfidenceLevel(score float64) ConfidenceLevel {
	if score >= ConfidenceThresholdHigh {
		return ConfidenceLevelHigh
	}
	if score >= ConfidenceThresholdMedium {
		return ConfidenceLevelMedium
	}
	return ConfidenceLevelLow
}

// IsLowConfidence returns true if the score is below medium threshold
func IsLowConfidence(score float64) bool {
	return score < ConfidenceThresholdMedium
}

// NeedsReview returns true if the item needs human review
// based on low AI confidence
func NeedsReview(score float64) bool {
	return score < ConfidenceThresholdMedium
}
