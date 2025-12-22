package parsing

import (
	"testing"
)

// TestCalculateMedicationScore tests the medication score calculation logic
// Note: Full integration tests would require complete mock setup
func TestCalculateMedicationScore_Lexical(t *testing.T) {
	tests := []struct {
		name       string
		offerMed   string
		requestMed string
		minScore   float64
		maxScore   float64
	}{
		{
			name:       "exact match",
			offerMed:   "Paracetamol",
			requestMed: "Paracetamol",
			minScore:   0.9,
			maxScore:   1.0,
		},
		{
			name:       "case insensitive exact match",
			offerMed:   "PARACETAMOL",
			requestMed: "paracetamol",
			minScore:   0.9,
			maxScore:   1.0,
		},
		{
			name:       "similar names with typo",
			offerMed:   "Paracetamol",
			requestMed: "Paracetamole",
			minScore:   0.5,
			maxScore:   1.0,
		},
		{
			name:       "substring match",
			offerMed:   "Paracetamol 500mg",
			requestMed: "Paracetamol",
			minScore:   0.3,
			maxScore:   0.8,
		},
		{
			name:       "different medications",
			offerMed:   "Aspirin",
			requestMed: "Ibuprofen",
			minScore:   0.0,
			maxScore:   0.4,
		},
	}

	// Test using fuzzyMatch directly since it's the core of lexical scoring
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			score := fuzzyMatch(tt.offerMed, tt.requestMed)
			if score < tt.minScore || score > tt.maxScore {
				t.Errorf("fuzzyMatch(%q, %q) = %.4f, want between %.4f and %.4f",
					tt.offerMed, tt.requestMed, score, tt.minScore, tt.maxScore)
			}
		})
	}
}

// TestMatchingScoreRanges ensures scoring produces reasonable ranges
func TestMatchingScoreRanges(t *testing.T) {
	// Perfect matches should score very high
	perfectScore := fuzzyMatch("Aspirin 100mg", "Aspirin 100mg")
	if perfectScore < 0.99 {
		t.Errorf("Perfect match should score >= 0.99, got %.4f", perfectScore)
	}

	// Similar items should score moderately
	similarScore := fuzzyMatch("Paracetamol 500", "Paracetamol 500mg")
	if similarScore < 0.7 || similarScore > 1.0 {
		t.Errorf("Similar match should score between 0.7-1.0, got %.4f", similarScore)
	}

	// Different items should score low
	differentScore := fuzzyMatch("Vitamin C", "Ibuprofen")
	if differentScore > 0.5 {
		t.Errorf("Different items should score < 0.5, got %.4f", differentScore)
	}
}

// TestSSEBroadcasterInterface verifies the interface definition
func TestSSEBroadcasterInterface(t *testing.T) {
	// This test just ensures the interface is defined correctly
	var _ SSEBroadcaster = (*mockBroadcaster)(nil)
}

type mockBroadcaster struct{}

func (m *mockBroadcaster) BroadcastNewOffer(offerID, medication string)       {}
func (m *mockBroadcaster) BroadcastNewRequest(requestID, medication string)   {}
func (m *mockBroadcaster) BroadcastNewMatch(matchID string, score float64)    {}
func (m *mockBroadcaster) BroadcastMatchUpdate(matchID string, status string) {}
