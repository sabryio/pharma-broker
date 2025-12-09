// Package circuitbreaker provides a generic circuit breaker implementation
// for protecting external service calls from cascading failures.
package circuitbreaker

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/rs/zerolog"
)

// Common errors returned by the circuit breaker.
var (
	ErrCircuitOpen     = errors.New("circuit breaker is open")
	ErrTooManyRequests = errors.New("too many requests in half-open state")
)

// State represents the state of the circuit breaker.
type State int

const (
	// StateClosed allows requests through normally.
	StateClosed State = iota
	// StateOpen blocks all requests.
	StateOpen
	// StateHalfOpen allows limited test requests.
	StateHalfOpen
)

// String returns the string representation of the circuit state.
func (s State) String() string {
	switch s {
	case StateClosed:
		return "CLOSED"
	case StateOpen:
		return "OPEN"
	case StateHalfOpen:
		return "HALF_OPEN"
	default:
		return "UNKNOWN"
	}
}

// Config holds the configuration for a circuit breaker.
type Config struct {
	// Name identifies this circuit breaker instance.
	Name string
	// FailureThreshold is the number of failures before opening the circuit.
	FailureThreshold int
	// SuccessThreshold is the number of successes needed to close from half-open.
	SuccessThreshold int
	// Timeout is the duration the circuit stays open before transitioning to half-open.
	Timeout time.Duration
	// MaxHalfOpenRequests limits concurrent requests in half-open state.
	MaxHalfOpenRequests int
	// FailureRatio triggers opening when failures/total >= this ratio (0-1).
	// If set, takes precedence over FailureThreshold when MinRequests is met.
	FailureRatio float64
	// MinRequests is the minimum number of requests before FailureRatio is evaluated.
	MinRequests int
	// OnStateChange is called when the circuit state changes.
	OnStateChange func(name string, from, to State)
}

// DefaultConfig returns a Config with sensible defaults.
func DefaultConfig(name string) Config {
	return Config{
		Name:                name,
		FailureThreshold:    5,
		SuccessThreshold:    2,
		Timeout:             30 * time.Second,
		MaxHalfOpenRequests: 1,
		FailureRatio:        0.6,
		MinRequests:         5,
	}
}

// Counts holds the request statistics for the circuit breaker.
type Counts struct {
	Requests             int
	TotalSuccesses       int
	TotalFailures        int
	ConsecutiveSuccesses int
	ConsecutiveFailures  int
}

// Breaker implements the circuit breaker pattern for external service calls.
// It is safe for concurrent use.
type Breaker struct {
	mu              sync.RWMutex
	cfg             Config
	state           State
	counts          Counts
	lastStateChange time.Time
	halfOpenCount   int // Current requests in half-open state
	log             zerolog.Logger
}

// New creates a new circuit breaker with the given configuration.
func New(cfg Config, log zerolog.Logger) *Breaker {
	if cfg.FailureThreshold <= 0 {
		cfg.FailureThreshold = 5
	}
	if cfg.SuccessThreshold <= 0 {
		cfg.SuccessThreshold = 2
	}
	if cfg.Timeout <= 0 {
		cfg.Timeout = 30 * time.Second
	}
	if cfg.MaxHalfOpenRequests <= 0 {
		cfg.MaxHalfOpenRequests = 1
	}
	if cfg.MinRequests <= 0 {
		cfg.MinRequests = 5
	}

	return &Breaker{
		cfg:             cfg,
		state:           StateClosed,
		lastStateChange: time.Now(),
		log:             log.With().Str("component", "circuit-breaker").Str("name", cfg.Name).Logger(),
	}
}

// Execute runs the given function if the circuit allows it.
// It automatically records success or failure based on the returned error.
// This is the primary method for using the circuit breaker.
func (b *Breaker) Execute(fn func() (any, error)) (any, error) {
	if err := b.beforeRequest(); err != nil {
		return nil, err
	}

	result, err := fn()
	b.afterRequest(err == nil)

	return result, err
}

// ExecuteWithContext runs the given function with context support.
// It respects context cancellation and records the result appropriately.
func (b *Breaker) ExecuteWithContext(ctx context.Context, fn func(context.Context) (any, error)) (any, error) {
	if err := b.beforeRequest(); err != nil {
		return nil, err
	}

	// Check context before executing
	select {
	case <-ctx.Done():
		b.afterRequest(false)
		return nil, ctx.Err()
	default:
	}

	result, err := fn(ctx)

	// Don't count context cancellation as a circuit failure
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		// Don't record - let the circuit state remain unchanged
		b.releaseHalfOpen()
		return nil, err
	}

	b.afterRequest(err == nil)
	return result, err
}

// beforeRequest checks if a request should be allowed and updates state if needed.
func (b *Breaker) beforeRequest() error {
	b.mu.Lock()
	defer b.mu.Unlock()

	now := time.Now()

	switch b.state {
	case StateOpen:
		// Check if timeout has passed to transition to half-open
		if now.Sub(b.lastStateChange) >= b.cfg.Timeout {
			b.setState(StateHalfOpen)
			b.halfOpenCount = 1
			return nil
		}
		return ErrCircuitOpen

	case StateHalfOpen:
		// Limit concurrent requests in half-open state
		if b.halfOpenCount >= b.cfg.MaxHalfOpenRequests {
			return ErrTooManyRequests
		}
		b.halfOpenCount++
		return nil

	default: // StateClosed
		return nil
	}
}

// afterRequest records the result of a request and updates circuit state.
func (b *Breaker) afterRequest(success bool) {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.counts.Requests++

	if success {
		b.onSuccess()
	} else {
		b.onFailure()
	}
}

// onSuccess handles a successful request.
func (b *Breaker) onSuccess() {
	b.counts.TotalSuccesses++
	b.counts.ConsecutiveSuccesses++
	b.counts.ConsecutiveFailures = 0

	switch b.state {
	case StateHalfOpen:
		b.halfOpenCount--
		if b.counts.ConsecutiveSuccesses >= b.cfg.SuccessThreshold {
			b.setState(StateClosed)
			b.resetCounts()
		}
	case StateClosed:
		// Reset failure count on success in closed state
		b.counts.ConsecutiveFailures = 0
	}
}

// onFailure handles a failed request.
func (b *Breaker) onFailure() {
	b.counts.TotalFailures++
	b.counts.ConsecutiveFailures++
	b.counts.ConsecutiveSuccesses = 0

	switch b.state {
	case StateHalfOpen:
		// Immediately open on any failure in half-open state
		b.halfOpenCount--
		b.setState(StateOpen)

	case StateClosed:
		if b.shouldTrip() {
			b.setState(StateOpen)
		}
	}
}

// shouldTrip determines if the circuit should open based on failure conditions.
func (b *Breaker) shouldTrip() bool {
	// Check failure ratio if configured and minimum requests met
	if b.cfg.FailureRatio > 0 && b.counts.Requests >= b.cfg.MinRequests {
		ratio := float64(b.counts.TotalFailures) / float64(b.counts.Requests)
		if ratio >= b.cfg.FailureRatio {
			return true
		}
	}

	// Check consecutive failure threshold
	return b.counts.ConsecutiveFailures >= b.cfg.FailureThreshold
}

// setState transitions to a new state and invokes the callback if configured.
func (b *Breaker) setState(newState State) {
	if b.state == newState {
		return
	}

	oldState := b.state
	b.state = newState
	b.lastStateChange = time.Now()

	b.log.Info().
		Str("from", oldState.String()).
		Str("to", newState.String()).
		Msg("Circuit breaker state changed")

	if b.cfg.OnStateChange != nil {
		// Call in goroutine to avoid blocking
		go b.cfg.OnStateChange(b.cfg.Name, oldState, newState)
	}
}

// resetCounts resets all counters (typically when closing the circuit).
func (b *Breaker) resetCounts() {
	b.counts = Counts{}
	b.halfOpenCount = 0
}

// releaseHalfOpen decrements the half-open counter without recording success/failure.
func (b *Breaker) releaseHalfOpen() {
	b.mu.Lock()
	defer b.mu.Unlock()

	if b.state == StateHalfOpen && b.halfOpenCount > 0 {
		b.halfOpenCount--
	}
}

// State returns the current circuit state.
func (b *Breaker) State() State {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.state
}

// Name returns the circuit breaker name.
func (b *Breaker) Name() string {
	return b.cfg.Name
}

// Stats returns current circuit breaker statistics.
func (b *Breaker) Stats() (state State, counts Counts, lastChange time.Time) {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.state, b.counts, b.lastStateChange
}

// IsOpen returns true if the circuit is currently open.
func (b *Breaker) IsOpen() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.state == StateOpen
}

// IsClosed returns true if the circuit is currently closed.
func (b *Breaker) IsClosed() bool {
	b.mu.RLock()
	defer b.mu.RUnlock()
	return b.state == StateClosed
}

// Reset forces the circuit breaker back to closed state.
// Use with caution - typically for administrative purposes.
func (b *Breaker) Reset() {
	b.mu.Lock()
	defer b.mu.Unlock()

	oldState := b.state
	b.state = StateClosed
	b.resetCounts()
	b.lastStateChange = time.Now()

	b.log.Warn().
		Str("from", oldState.String()).
		Msg("Circuit breaker manually reset")
}
