package resilience

import (
	"sync"
	"time"
)

type State int

const (
	StateClosed State = iota
	StateOpen
	StateHalfOpen
)

// CircuitBreaker prevents cascading failures by stopping calls after consecutive errors.
type CircuitBreaker struct {
	mu            sync.RWMutex
	state         State
	failureCount  int
	maxFailures   int
	resetTimeout  time.Duration
	lastFailure   time.Time
	onStateChange func(State)
}

// NewCircuitBreaker creates a new circuit breaker.
func NewCircuitBreaker(maxFailures int, resetTimeout time.Duration) *CircuitBreaker {
	return &CircuitBreaker{
		state:        StateClosed,
		maxFailures:  maxFailures,
		resetTimeout: resetTimeout,
	}
}

// SetOnStateChange sets a callback for state changes.
func (cb *CircuitBreaker) SetOnStateChange(fn func(State)) {
	cb.mu.Lock()
	defer cb.mu.Unlock()
	cb.onStateChange = fn
}

// Allow returns true if the call is permitted.
func (cb *CircuitBreaker) Allow() bool {
	cb.mu.Lock()
	defer cb.mu.Unlock()

	if cb.state == StateClosed {
		return true
	}

	if cb.state == StateOpen {
		if time.Since(cb.lastFailure) > cb.resetTimeout {
			cb.setState(StateHalfOpen)
			return true
		}
		return false
	}

	// HalfOpen: allow one call
	return true
}

// RecordSuccess records a successful call and potentially closes the circuit.
func (cb *CircuitBreaker) RecordSuccess() {
	cb.mu.Lock()
	defer cb.mu.Unlock()

	if cb.state == StateHalfOpen {
		cb.setState(StateClosed)
	}
	cb.failureCount = 0
}

// RecordFailure records a failed call and potentially opens the circuit.
func (cb *CircuitBreaker) RecordFailure() {
	cb.mu.Lock()
	defer cb.mu.Unlock()

	cb.failureCount++
	cb.lastFailure = time.Now()

	if cb.state == StateClosed && cb.failureCount >= cb.maxFailures {
		cb.setState(StateOpen)
	} else if cb.state == StateHalfOpen {
		cb.setState(StateOpen)
	}
}

func (cb *CircuitBreaker) setState(s State) {
	if cb.state != s {
		cb.state = s
		if cb.onStateChange != nil {
			cb.onStateChange(s)
		}
	}
}

// State returns the current state of the circuit breaker.
func (cb *CircuitBreaker) State() State {
	cb.mu.RLock()
	defer cb.mu.RUnlock()
	return cb.state
}
