package circuitbreaker

import (
	"context"
	"errors"
	"io"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func newTestBreaker(threshold int, timeout time.Duration) *Breaker {
	log := zerolog.New(io.Discard)
	cfg := Config{
		Name:                "test",
		FailureThreshold:    threshold,
		SuccessThreshold:    2,
		Timeout:             timeout,
		MaxHalfOpenRequests: 1,
	}
	return New(cfg, log)
}

func TestBreaker_StartsClosed(t *testing.T) {
	cb := newTestBreaker(3, time.Second)

	if cb.State() != StateClosed {
		t.Errorf("Initial state = %v, want CLOSED", cb.State())
	}

	if !cb.IsClosed() {
		t.Error("IsClosed() should return true")
	}
}

func TestBreaker_OpensAfterThreshold(t *testing.T) {
	cb := newTestBreaker(3, time.Second)

	errTest := errors.New("test error")

	// Record failures up to threshold
	for i := 0; i < 3; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	if cb.State() != StateOpen {
		t.Errorf("State = %v, want OPEN after %d failures", cb.State(), 3)
	}

	if !cb.IsOpen() {
		t.Error("IsOpen() should return true")
	}

	// Should reject requests when open
	_, err := cb.Execute(func() (any, error) {
		return "should not run", nil
	})

	if !errors.Is(err, ErrCircuitOpen) {
		t.Errorf("Expected ErrCircuitOpen, got %v", err)
	}
}

func TestBreaker_TransitionsToHalfOpen(t *testing.T) {
	cb := newTestBreaker(2, 10*time.Millisecond)

	errTest := errors.New("test error")

	// Open the circuit
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	if cb.State() != StateOpen {
		t.Fatal("Circuit should be open")
	}

	// Wait for reset timeout
	time.Sleep(15 * time.Millisecond)

	// Should transition to half-open on next request
	_, _ = cb.Execute(func() (any, error) {
		return "test", nil
	})

	// After success in half-open, might still be half-open (needs 2 successes)
	// or closed depending on SuccessThreshold
	state := cb.State()
	if state != StateHalfOpen && state != StateClosed {
		t.Errorf("State = %v, want HALF_OPEN or CLOSED", state)
	}
}

func TestBreaker_ClosesAfterSuccesses(t *testing.T) {
	log := zerolog.New(io.Discard)
	cfg := Config{
		Name:                "test",
		FailureThreshold:    2,
		SuccessThreshold:    2,
		Timeout:             10 * time.Millisecond,
		MaxHalfOpenRequests: 2,
	}
	cb := New(cfg, log)

	errTest := errors.New("test error")

	// Open the circuit
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	// Wait for timeout
	time.Sleep(15 * time.Millisecond)

	// Record successes in half-open
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return "success", nil
		})
	}

	if cb.State() != StateClosed {
		t.Errorf("State = %v, want CLOSED after successes", cb.State())
	}
}

func TestBreaker_ReopensOnHalfOpenFailure(t *testing.T) {
	cb := newTestBreaker(2, 10*time.Millisecond)

	errTest := errors.New("test error")

	// Open the circuit
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	// Wait for timeout
	time.Sleep(15 * time.Millisecond)

	// Fail in half-open
	_, _ = cb.Execute(func() (any, error) {
		return nil, errTest
	})

	if cb.State() != StateOpen {
		t.Errorf("State = %v, want OPEN after half-open failure", cb.State())
	}
}

func TestBreaker_ExecuteWithContext(t *testing.T) {
	cb := newTestBreaker(3, time.Second)

	ctx := context.Background()

	result, err := cb.ExecuteWithContext(ctx, func(ctx context.Context) (any, error) {
		return "success", nil
	})

	if err != nil {
		t.Errorf("Unexpected error: %v", err)
	}

	if result != "success" {
		t.Errorf("Result = %v, want 'success'", result)
	}
}

func TestBreaker_ExecuteWithContext_Cancelled(t *testing.T) {
	cb := newTestBreaker(3, time.Second)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	_, err := cb.ExecuteWithContext(ctx, func(ctx context.Context) (any, error) {
		return "should not run", nil
	})

	if !errors.Is(err, context.Canceled) {
		t.Errorf("Expected context.Canceled, got %v", err)
	}

	// Circuit should still be closed (context cancellation doesn't count as failure)
	if cb.State() != StateClosed {
		t.Errorf("State = %v, want CLOSED (context cancel shouldn't trip circuit)", cb.State())
	}
}

func TestBreaker_Reset(t *testing.T) {
	cb := newTestBreaker(2, time.Second)

	errTest := errors.New("test error")

	// Open the circuit
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	if cb.State() != StateOpen {
		t.Fatal("Circuit should be open")
	}

	// Reset
	cb.Reset()

	if cb.State() != StateClosed {
		t.Errorf("State = %v, want CLOSED after reset", cb.State())
	}

	// Should allow requests again
	_, err := cb.Execute(func() (any, error) {
		return "success", nil
	})

	if err != nil {
		t.Errorf("Should allow requests after reset, got error: %v", err)
	}
}

func TestBreaker_Stats(t *testing.T) {
	cb := newTestBreaker(5, time.Second)

	// Execute some requests
	_, _ = cb.Execute(func() (any, error) { return nil, nil })
	_, _ = cb.Execute(func() (any, error) { return nil, errors.New("fail") })
	_, _ = cb.Execute(func() (any, error) { return nil, nil })

	state, counts, _ := cb.Stats()

	if state != StateClosed {
		t.Errorf("State = %v, want CLOSED", state)
	}

	if counts.Requests != 3 {
		t.Errorf("Requests = %d, want 3", counts.Requests)
	}

	if counts.TotalSuccesses != 2 {
		t.Errorf("TotalSuccesses = %d, want 2", counts.TotalSuccesses)
	}

	if counts.TotalFailures != 1 {
		t.Errorf("TotalFailures = %d, want 1", counts.TotalFailures)
	}
}

func TestBreaker_FailureRatio(t *testing.T) {
	log := zerolog.New(io.Discard)
	cfg := Config{
		Name:             "test",
		FailureThreshold: 100, // High threshold so ratio triggers first
		FailureRatio:     0.5,
		MinRequests:      4,
		Timeout:          time.Second,
	}
	cb := New(cfg, log)

	errTest := errors.New("test error")

	// 2 successes, 2 failures = 50% failure ratio
	_, _ = cb.Execute(func() (any, error) { return nil, nil })
	_, _ = cb.Execute(func() (any, error) { return nil, errTest })
	_, _ = cb.Execute(func() (any, error) { return nil, nil })
	_, _ = cb.Execute(func() (any, error) { return nil, errTest })

	if cb.State() != StateOpen {
		t.Errorf("State = %v, want OPEN (failure ratio >= 0.5)", cb.State())
	}
}

func TestState_String(t *testing.T) {
	tests := []struct {
		state    State
		expected string
	}{
		{StateClosed, "CLOSED"},
		{StateOpen, "OPEN"},
		{StateHalfOpen, "HALF_OPEN"},
		{State(99), "UNKNOWN"},
	}

	for _, tt := range tests {
		if tt.state.String() != tt.expected {
			t.Errorf("State %v String() = %s, want %s", tt.state, tt.state.String(), tt.expected)
		}
	}
}

func TestBreaker_Name(t *testing.T) {
	cb := newTestBreaker(3, time.Second)

	if cb.Name() != "test" {
		t.Errorf("Name() = %s, want 'test'", cb.Name())
	}
}

func TestBreaker_TooManyRequestsInHalfOpen(t *testing.T) {
	log := zerolog.New(io.Discard)
	cfg := Config{
		Name:                "test",
		FailureThreshold:    2,
		Timeout:             10 * time.Millisecond,
		MaxHalfOpenRequests: 1,
	}
	cb := New(cfg, log)

	errTest := errors.New("test error")

	// Open the circuit
	for i := 0; i < 2; i++ {
		_, _ = cb.Execute(func() (any, error) {
			return nil, errTest
		})
	}

	// Wait for timeout
	time.Sleep(15 * time.Millisecond)

	// First request should be allowed (transitions to half-open)
	done := make(chan struct{})
	go func() {
		_, _ = cb.Execute(func() (any, error) {
			time.Sleep(50 * time.Millisecond) // Hold the slot
			return nil, nil
		})
		close(done)
	}()

	// Give the goroutine time to start
	time.Sleep(5 * time.Millisecond)

	// Second request should be rejected
	_, err := cb.Execute(func() (any, error) {
		return "should not run", nil
	})

	if !errors.Is(err, ErrTooManyRequests) {
		t.Errorf("Expected ErrTooManyRequests, got %v", err)
	}

	<-done // Wait for first request to complete
}
