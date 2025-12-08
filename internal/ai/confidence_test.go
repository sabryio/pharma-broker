package ai

import (
	"testing"
)

func TestGetConfidenceLevel(t *testing.T) {
	tests := []struct {
		name     string
		score    float64
		expected ConfidenceLevel
	}{
		{"Perfect confidence", 1.0, ConfidenceLevelHigh},
		{"High threshold exact", 0.8, ConfidenceLevelHigh},
		{"High confidence", 0.95, ConfidenceLevelHigh},
		{"High confidence lower", 0.85, ConfidenceLevelHigh},
		{"Medium threshold exact", 0.5, ConfidenceLevelMedium},
		{"Medium confidence", 0.7, ConfidenceLevelMedium},
		{"Medium confidence lower", 0.6, ConfidenceLevelMedium},
		{"Low confidence", 0.4, ConfidenceLevelLow},
		{"Very low confidence", 0.2, ConfidenceLevelLow},
		{"Zero confidence", 0.0, ConfidenceLevelLow},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := GetConfidenceLevel(tt.score)
			if result != tt.expected {
				t.Errorf("GetConfidenceLevel(%v) = %v, want %v", tt.score, result, tt.expected)
			}
		})
	}
}

func TestIsLowConfidence(t *testing.T) {
	tests := []struct {
		name     string
		score    float64
		expected bool
	}{
		{"High confidence", 0.9, false},
		{"High threshold", 0.8, false},
		{"Medium high", 0.7, false},
		{"Medium exact", 0.5, false},
		{"Just below medium", 0.49, true},
		{"Low confidence", 0.3, true},
		{"Very low", 0.1, true},
		{"Zero", 0.0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := IsLowConfidence(tt.score)
			if result != tt.expected {
				t.Errorf("IsLowConfidence(%v) = %v, want %v", tt.score, result, tt.expected)
			}
		})
	}
}

func TestNeedsReview(t *testing.T) {
	tests := []struct {
		name     string
		score    float64
		expected bool
	}{
		{"Perfect - no review", 1.0, false},
		{"High - no review", 0.9, false},
		{"Threshold high - no review", 0.8, false},
		{"Medium - no review", 0.6, false},
		{"Threshold exact - no review", 0.5, false},
		{"Below threshold - needs review", 0.49, true},
		{"Low - needs review", 0.3, true},
		{"Very low - needs review", 0.1, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := NeedsReview(tt.score)
			if result != tt.expected {
				t.Errorf("NeedsReview(%v) = %v, want %v", tt.score, result, tt.expected)
			}
		})
	}
}

func TestConfidenceThresholds(t *testing.T) {
	// Verify threshold constants are set correctly
	if ConfidenceThresholdHigh != 0.8 {
		t.Errorf("ConfidenceThresholdHigh = %v, want 0.8", ConfidenceThresholdHigh)
	}
	if ConfidenceThresholdMedium != 0.5 {
		t.Errorf("ConfidenceThresholdMedium = %v, want 0.5", ConfidenceThresholdMedium)
	}
	if ConfidenceThresholdLow != 0.0 {
		t.Errorf("ConfidenceThresholdLow = %v, want 0.0", ConfidenceThresholdLow)
	}

	// Verify thresholds are in correct order
	if ConfidenceThresholdMedium >= ConfidenceThresholdHigh {
		t.Error("ConfidenceThresholdMedium should be less than ConfidenceThresholdHigh")
	}
	if ConfidenceThresholdLow >= ConfidenceThresholdMedium {
		t.Error("ConfidenceThresholdLow should be less than ConfidenceThresholdMedium")
	}
}
