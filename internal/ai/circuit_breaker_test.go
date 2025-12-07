package ai

import (
	"testing"
	"time"
)

func TestCircuitBreaker_StartsCloscd(t *testing.T) {
	cb := NewCircuitBreaker("test", 3, time.Second)

	if cb.State() != CircuitClosed {
		t.Errorf("Initial state = %v, want CLOSED", cb.State())
	}

	if !cb.Allow() {
		t.Error("Should allow requests when closed")
	}
}

func TestCircuitBreaker_OpensAfterThreshold(t *testing.T) {
	cb := NewCircuitBreaker("test", 3, time.Second)

	// Record failures up to threshold
	for range 3 {
		cb.RecordFailure()
	}

	if cb.State() != CircuitOpen {
		t.Errorf("State = %v, want OPEN after %d failures", cb.State(), 3)
	}

	if cb.Allow() {
		t.Error("Should block requests when open")
	}
}

func TestCircuitBreaker_TransitionsToHalfOpen(t *testing.T) {
	cb := NewCircuitBreaker("test", 2, 10*time.Millisecond)

	// Open the circuit
	cb.RecordFailure()
	cb.RecordFailure()

	if cb.State() != CircuitOpen {
		t.Fatal("Circuit should be open")
	}

	// Wait for reset timeout
	time.Sleep(15 * time.Millisecond)

	// Should transition to half-open on Allow()
	if !cb.Allow() {
		t.Error("Should allow test request after reset timeout")
	}

	if cb.State() != CircuitHalfOpen {
		t.Errorf("State = %v, want HALF_OPEN", cb.State())
	}
}

func TestCircuitBreaker_ClosesAfterSuccesses(t *testing.T) {
	cb := NewCircuitBreaker("test", 2, 10*time.Millisecond)
	cb.halfOpenMax = 2

	// Open and transition to half-open
	cb.RecordFailure()
	cb.RecordFailure()
	time.Sleep(15 * time.Millisecond)
	cb.Allow() // Transitions to half-open

	// Record successes
	cb.RecordSuccess()
	cb.RecordSuccess()

	if cb.State() != CircuitClosed {
		t.Errorf("State = %v, want CLOSED after successes", cb.State())
	}
}

func TestCircuitBreaker_ReopensOnHalfOpenFailure(t *testing.T) {
	cb := NewCircuitBreaker("test", 2, 10*time.Millisecond)

	// Open and transition to half-open
	cb.RecordFailure()
	cb.RecordFailure()
	time.Sleep(15 * time.Millisecond)
	cb.Allow()

	// Fail in half-open
	cb.RecordFailure()

	if cb.State() != CircuitOpen {
		t.Errorf("State = %v, want OPEN after half-open failure", cb.State())
	}
}

func TestCircuitBreaker_ResetsFailuresOnSuccess(t *testing.T) {
	cb := NewCircuitBreaker("test", 3, time.Second)

	cb.RecordFailure()
	cb.RecordFailure()
	cb.RecordSuccess() // Resets failure count

	// Should need 3 more failures to open
	cb.RecordFailure()
	cb.RecordFailure()

	if cb.State() != CircuitClosed {
		t.Error("Should still be closed - failures were reset")
	}

	cb.RecordFailure() // Now at threshold

	if cb.State() != CircuitOpen {
		t.Error("Should be open after reaching threshold")
	}
}

func TestCircuitState_String(t *testing.T) {
	tests := []struct {
		state    CircuitState
		expected string
	}{
		{CircuitClosed, "CLOSED"},
		{CircuitOpen, "OPEN"},
		{CircuitHalfOpen, "HALF_OPEN"},
		{CircuitState(99), "UNKNOWN"},
	}

	for _, tt := range tests {
		if tt.state.String() != tt.expected {
			t.Errorf("State %v String() = %s, want %s", tt.state, tt.state.String(), tt.expected)
		}
	}
}
