package matching

import (
	"math"
	"testing"
	"time"
)

func TestDecayTypes(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	// Test each decay type with the same input
	_ = 24.0                            // 24 hours
	testAge := now.Add(-24 * time.Hour) // Exactly at half-life

	t.Run("Exponential decay at half-life", func(t *testing.T) {
		s.SetDecayType(DecayExponential)
		score := s.RecencyScore(testAge)
		// Should be ~0.5 at half-life
		if math.Abs(score-0.5) > 0.02 {
			t.Errorf("Exponential decay at half-life = %v, want ~0.5", score)
		}
	})

	t.Run("Linear decay at half-life", func(t *testing.T) {
		s.SetDecayType(DecayLinear)
		score := s.RecencyScore(testAge)
		// Linear: at 24h of 48h max = 0.5
		if math.Abs(score-0.5) > 0.02 {
			t.Errorf("Linear decay at half-life = %v, want ~0.5", score)
		}
	})

	t.Run("Logarithmic decay at half-life", func(t *testing.T) {
		s.SetDecayType(DecayLogarithmic)
		score := s.RecencyScore(testAge)
		// Logarithmic decays slower, should be > 0.5
		if score <= 0.5 {
			t.Errorf("Logarithmic decay at half-life = %v, want > 0.5", score)
		}
	})
}

func TestSetRecencyHalfLife(t *testing.T) {
	s := NewScorer(nil, nil)

	// Test setting custom half-life
	s.SetRecencyHalfLife(12.0)
	if s.GetRecencyHalfLife() != 12.0 {
		t.Errorf("GetRecencyHalfLife() = %v, want 12.0", s.GetRecencyHalfLife())
	}

	// Test that it's actually used
	now := time.Now()
	testAge := now.Add(-12 * time.Hour) // At new half-life
	score := s.RecencyScore(testAge)
	if math.Abs(score-0.5) > 0.02 {
		t.Errorf("Score at custom half-life = %v, want ~0.5", score)
	}
}

func TestSetDecayType(t *testing.T) {
	s := NewScorer(nil, nil)

	// Test default
	if s.GetDecayType() != DecayExponential {
		t.Errorf("Default decay type = %v, want EXPONENTIAL", s.GetDecayType())
	}

	// Test setting linear
	s.SetDecayType(DecayLinear)
	if s.GetDecayType() != DecayLinear {
		t.Errorf("GetDecayType() = %v, want LINEAR", s.GetDecayType())
	}

	// Test setting logarithmic
	s.SetDecayType(DecayLogarithmic)
	if s.GetDecayType() != DecayLogarithmic {
		t.Errorf("GetDecayType() = %v, want LOGARITHMIC", s.GetDecayType())
	}
}

func TestRecencyScoreWithParams(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()

	tests := []struct {
		name      string
		age       time.Duration
		halfLife  float64
		decayType DecayType
		minScore  float64
		maxScore  float64
	}{
		{"Exponential 12h at half-life", -12 * time.Hour, 12.0, DecayExponential, 0.48, 0.52},
		{"Linear 24h at half-point", -24 * time.Hour, 24.0, DecayLinear, 0.48, 0.52},
		{"Logarithmic slow decay", -24 * time.Hour, 24.0, DecayLogarithmic, 0.5, 1.0},
		{"Exponential very old", -7 * 24 * time.Hour, 24.0, DecayExponential, 0.0, 0.01},
		{"Linear at max age", -48 * time.Hour, 24.0, DecayLinear, 0.0, 0.01},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			testTime := now.Add(tt.age)
			score := s.RecencyScoreWithParams(testTime, tt.halfLife, tt.decayType)
			if score < tt.minScore || score > tt.maxScore {
				t.Errorf("Score = %v, want between %v and %v", score, tt.minScore, tt.maxScore)
			}
		})
	}
}

func TestDecayComparison(t *testing.T) {
	s := NewScorer(nil, nil)
	now := time.Now()
	testAge := now.Add(-36 * time.Hour) // 1.5 days old

	s.SetDecayType(DecayExponential)
	exp := s.RecencyScore(testAge)

	s.SetDecayType(DecayLinear)
	lin := s.RecencyScore(testAge)

	s.SetDecayType(DecayLogarithmic)
	log := s.RecencyScore(testAge)

	// At 36h: Logarithmic should decay slowest, Linear fastest
	// log > exp > lin (generally)
	if !(log >= exp) {
		t.Errorf("Logarithmic (%v) should be >= Exponential (%v)", log, exp)
	}

	t.Logf("At 36h: Exponential=%v, Linear=%v, Logarithmic=%v", exp, lin, log)
}
