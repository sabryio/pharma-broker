// Package circuitbreaker provides a circuit breaker implementation that wraps
// github.com/sony/gobreaker with enhanced observability and context support.
package circuitbreaker

import (
	"context"
	"errors"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
	"github.com/sony/gobreaker/v2"
)

// Common errors returned by the circuit breaker.
var (
	ErrCircuitOpen     = gobreaker.ErrOpenState
	ErrTooManyRequests = gobreaker.ErrTooManyRequests
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

// fromGobreakerState converts gobreaker.State to our State type.
func fromGobreakerState(gs gobreaker.State) State {
	switch gs {
	case gobreaker.StateClosed:
		return StateClosed
	case gobreaker.StateOpen:
		return StateOpen
	case gobreaker.StateHalfOpen:
		return StateHalfOpen
	default:
		return StateClosed
	}
}

// Config holds the configuration for a circuit breaker.
type Config struct {
	// Name identifies this circuit breaker instance.
	Name string

	// FailureThreshold is the number of consecutive failures before opening.
	// Used when FailureRatio is not set or MinRequests not met.
	FailureThreshold int

	// SuccessThreshold is the number of consecutive successes needed to close from half-open.
	SuccessThreshold int

	// Timeout is the duration the circuit stays open before transitioning to half-open.
	Timeout time.Duration

	// MaxHalfOpenRequests limits concurrent requests in half-open state.
	MaxHalfOpenRequests int

	// FailureRatio triggers opening when failures/total >= this ratio (0-1).
	// Takes precedence over FailureThreshold when MinRequests is met.
	FailureRatio float64

	// MinRequests is the minimum number of requests before FailureRatio is evaluated.
	MinRequests int

	// Interval is the cyclic period of the closed state for the circuit breaker
	// to clear the internal counts. If Interval is 0, the counts are never cleared.
	Interval time.Duration

	// IsSuccessful is a function to determine if an error should count as a failure.
	// Return true if the error should be treated as a success (e.g., expected errors).
	// If nil, all non-nil errors are counted as failures.
	IsSuccessful func(err error) bool

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
		Interval:            60 * time.Second,
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

// fromGobreakerCounts converts gobreaker.Counts to our Counts type.
func fromGobreakerCounts(gc gobreaker.Counts) Counts {
	return Counts{
		Requests:             int(gc.Requests),
		TotalSuccesses:       int(gc.TotalSuccesses),
		TotalFailures:        int(gc.TotalFailures),
		ConsecutiveSuccesses: int(gc.ConsecutiveSuccesses),
		ConsecutiveFailures:  int(gc.ConsecutiveFailures),
	}
}

// Breaker wraps gobreaker.CircuitBreaker with enhanced observability and context support.
// It is safe for concurrent use.
type Breaker struct {
	cb              *gobreaker.CircuitBreaker[any]
	cfg             Config
	log             zerolog.Logger
	lastStateChange atomic.Value // time.Time
}

// New creates a new circuit breaker with the given configuration.
func New(cfg Config, log zerolog.Logger) *Breaker {
	// Apply defaults
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

	b := &Breaker{
		cfg: cfg,
		log: log.With().Str("component", "circuit-breaker").Str("name", cfg.Name).Logger(),
	}
	b.lastStateChange.Store(time.Now())

	// Build gobreaker settings
	settings := gobreaker.Settings{
		Name:        cfg.Name,
		MaxRequests: uint32(cfg.MaxHalfOpenRequests),
		Interval:    cfg.Interval,
		Timeout:     cfg.Timeout,

		// ReadyToTrip is called with the current counts whenever a request fails.
		ReadyToTrip: func(counts gobreaker.Counts) bool {
			// Check failure ratio if configured and minimum requests met
			if cfg.FailureRatio > 0 && counts.Requests >= uint32(cfg.MinRequests) {
				ratio := float64(counts.TotalFailures) / float64(counts.Requests)
				if ratio >= cfg.FailureRatio {
					return true
				}
			}
			// Check consecutive failure threshold
			return counts.ConsecutiveFailures >= uint32(cfg.FailureThreshold)
		},

		// OnStateChange is called when the state changes.
		OnStateChange: func(name string, from, to gobreaker.State) {
			fromState := fromGobreakerState(from)
			toState := fromGobreakerState(to)

			b.lastStateChange.Store(time.Now())

			b.log.Info().
				Str("from", fromState.String()).
				Str("to", toState.String()).
				Msg("Circuit breaker state changed")

			if cfg.OnStateChange != nil {
				// Call in goroutine to avoid blocking
				go cfg.OnStateChange(name, fromState, toState)
			}
		},

		// IsSuccessful determines whether the error should be considered a failure.
		IsSuccessful: func(err error) bool {
			if err == nil {
				return true
			}
			// Context cancellation/deadline should not trip the circuit
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return true
			}
			// Use custom success checker if provided
			if cfg.IsSuccessful != nil {
				return cfg.IsSuccessful(err)
			}
			return false
		},
	}

	b.cb = gobreaker.NewCircuitBreaker[any](settings)
	return b
}

// Execute runs the given function if the circuit allows it.
// It automatically records success or failure based on the returned error.
// This is the primary method for using the circuit breaker.
func (b *Breaker) Execute(fn func() (any, error)) (any, error) {
	result, err := b.cb.Execute(fn)
	if err != nil {
		b.logExecutionError(err)
	}
	return result, err
}

// ExecuteWithContext runs the given function with context support.
// It respects context cancellation and records the result appropriately.
func (b *Breaker) ExecuteWithContext(ctx context.Context, fn func(context.Context) (any, error)) (any, error) {
	// Check context before executing
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	default:
	}

	result, err := b.cb.Execute(func() (any, error) {
		// Check context again inside the execution
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		default:
		}
		return fn(ctx)
	})

	if err != nil {
		b.logExecutionError(err)
	}
	return result, err
}

// logExecutionError logs execution errors at appropriate levels.
func (b *Breaker) logExecutionError(err error) {
	switch {
	case errors.Is(err, ErrCircuitOpen):
		b.log.Warn().Msg("Request rejected: circuit is open")
	case errors.Is(err, ErrTooManyRequests):
		b.log.Warn().Msg("Request rejected: too many requests in half-open state")
	case errors.Is(err, context.Canceled):
		b.log.Debug().Msg("Request cancelled by context")
	case errors.Is(err, context.DeadlineExceeded):
		b.log.Debug().Msg("Request deadline exceeded")
	}
}

// State returns the current circuit state.
func (b *Breaker) State() State {
	return fromGobreakerState(b.cb.State())
}

// Name returns the circuit breaker name.
func (b *Breaker) Name() string {
	return b.cfg.Name
}

// Stats returns current circuit breaker statistics.
func (b *Breaker) Stats() (state State, counts Counts, lastChange time.Time) {
	gbState, gbCounts := b.cb.State(), b.cb.Counts()
	lastChange, _ = b.lastStateChange.Load().(time.Time)
	return fromGobreakerState(gbState), fromGobreakerCounts(gbCounts), lastChange
}

// IsOpen returns true if the circuit is currently open.
func (b *Breaker) IsOpen() bool {
	return b.cb.State() == gobreaker.StateOpen
}

// IsClosed returns true if the circuit is currently closed.
func (b *Breaker) IsClosed() bool {
	return b.cb.State() == gobreaker.StateClosed
}

// IsHalfOpen returns true if the circuit is currently half-open.
func (b *Breaker) IsHalfOpen() bool {
	return b.cb.State() == gobreaker.StateHalfOpen
}

// Reset forces the circuit breaker back to closed state.
// Note: gobreaker v2 doesn't expose a public reset method, so we recreate the breaker.
// Use with caution - typically for administrative purposes.
func (b *Breaker) Reset() {
	oldState := b.State()

	// Recreate the circuit breaker with the same settings
	*b = *New(b.cfg, b.log)

	b.log.Warn().
		Str("from", oldState.String()).
		Msg("Circuit breaker manually reset")
}

// ---- Two-Phase Circuit Breaker (Advanced Pattern) ----

// TwoPhaseBreaker provides separate circuit breakers for different failure modes.
// This is useful when you want different thresholds for transient vs persistent failures.
type TwoPhaseBreaker struct {
	primary   *Breaker // For transient failures (network, timeout)
	secondary *Breaker // For persistent failures (5xx, validation)
	log       zerolog.Logger
}

// TwoPhaseConfig configures the two-phase circuit breaker.
type TwoPhaseConfig struct {
	Name string

	// Primary handles transient failures (shorter timeout, quick recovery)
	PrimaryFailureThreshold    int
	PrimaryTimeout             time.Duration
	PrimaryMaxHalfOpenRequests int

	// Secondary handles persistent failures (longer timeout, stricter)
	SecondaryFailureThreshold    int
	SecondaryTimeout             time.Duration
	SecondaryMaxHalfOpenRequests int

	// IsTransient determines if an error is transient (goes to primary)
	// If nil, all errors go to primary.
	IsTransient func(err error) bool

	OnStateChange func(name string, phase string, from, to State)
}

// NewTwoPhaseBreaker creates a two-phase circuit breaker.
func NewTwoPhaseBreaker(cfg TwoPhaseConfig, log zerolog.Logger) *TwoPhaseBreaker {
	// Primary: fast recovery for transient issues
	primaryCfg := Config{
		Name:                cfg.Name + "-primary",
		FailureThreshold:    cfg.PrimaryFailureThreshold,
		SuccessThreshold:    1,
		Timeout:             cfg.PrimaryTimeout,
		MaxHalfOpenRequests: cfg.PrimaryMaxHalfOpenRequests,
		MinRequests:         3,
	}
	if primaryCfg.FailureThreshold <= 0 {
		primaryCfg.FailureThreshold = 3
	}
	if primaryCfg.Timeout <= 0 {
		primaryCfg.Timeout = 10 * time.Second
	}
	if primaryCfg.MaxHalfOpenRequests <= 0 {
		primaryCfg.MaxHalfOpenRequests = 2
	}
	if cfg.OnStateChange != nil {
		primaryCfg.OnStateChange = func(name string, from, to State) {
			cfg.OnStateChange(cfg.Name, "primary", from, to)
		}
	}

	// Secondary: stricter for persistent issues
	secondaryCfg := Config{
		Name:                cfg.Name + "-secondary",
		FailureThreshold:    cfg.SecondaryFailureThreshold,
		SuccessThreshold:    3,
		Timeout:             cfg.SecondaryTimeout,
		MaxHalfOpenRequests: cfg.SecondaryMaxHalfOpenRequests,
		MinRequests:         5,
	}
	if secondaryCfg.FailureThreshold <= 0 {
		secondaryCfg.FailureThreshold = 5
	}
	if secondaryCfg.Timeout <= 0 {
		secondaryCfg.Timeout = 60 * time.Second
	}
	if secondaryCfg.MaxHalfOpenRequests <= 0 {
		secondaryCfg.MaxHalfOpenRequests = 1
	}
	if cfg.OnStateChange != nil {
		secondaryCfg.OnStateChange = func(name string, from, to State) {
			cfg.OnStateChange(cfg.Name, "secondary", from, to)
		}
	}

	return &TwoPhaseBreaker{
		primary:   New(primaryCfg, log),
		secondary: New(secondaryCfg, log),
		log:       log.With().Str("component", "two-phase-breaker").Str("name", cfg.Name).Logger(),
	}
}

// Execute runs the function through both circuit breakers.
// Primary is checked first (for transient failures), then secondary (for persistent).
func (tb *TwoPhaseBreaker) Execute(fn func() (any, error)) (any, error) {
	// Check primary breaker first
	if tb.primary.IsOpen() {
		return nil, ErrCircuitOpen
	}

	// Check secondary breaker
	if tb.secondary.IsOpen() {
		return nil, ErrCircuitOpen
	}

	// Execute through primary (it will record the result)
	result, err := tb.primary.Execute(fn)

	// If there was an error, also record it in secondary for persistent tracking
	if err != nil && !errors.Is(err, ErrCircuitOpen) && !errors.Is(err, ErrTooManyRequests) {
		// Run a no-op through secondary to record the failure
		_, _ = tb.secondary.Execute(func() (any, error) { return nil, err })
	}

	return result, err
}

// State returns the combined state (open if either is open).
func (tb *TwoPhaseBreaker) State() State {
	if tb.primary.IsOpen() || tb.secondary.IsOpen() {
		return StateOpen
	}
	if tb.primary.IsHalfOpen() || tb.secondary.IsHalfOpen() {
		return StateHalfOpen
	}
	return StateClosed
}

// PrimaryState returns the primary circuit breaker state.
func (tb *TwoPhaseBreaker) PrimaryState() State {
	return tb.primary.State()
}

// SecondaryState returns the secondary circuit breaker state.
func (tb *TwoPhaseBreaker) SecondaryState() State {
	return tb.secondary.State()
}
