// Package reconnector provides resilient reconnection with exponential backoff.
// Ported from legacy/messaging/reconnector for the minimal bridge.
package reconnector

import (
	"context"
	"sync"
	"time"

	"github.com/cenkalti/backoff/v4"
	"github.com/rs/zerolog"
)

// Config configures the reconnection behavior.
type Config struct {
	// InitialInterval is the starting delay between reconnection attempts.
	InitialInterval time.Duration
	// MaxInterval is the maximum delay between reconnection attempts.
	MaxInterval time.Duration
	// Multiplier is the factor by which the interval increases.
	Multiplier float64
	// RandomizationFactor adds jitter to prevent thundering herd (0.0-1.0).
	RandomizationFactor float64
	// MaxElapsedTime is the maximum total time to attempt reconnection. 0 = infinite.
	MaxElapsedTime time.Duration
	// MaxRetries is the maximum number of retry attempts. 0 = infinite.
	MaxRetries uint64
}

// DefaultConfig returns production-ready defaults.
func DefaultConfig() Config {
	return Config{
		InitialInterval:     5 * time.Second,
		MaxInterval:         5 * time.Minute,
		Multiplier:          2.0,
		RandomizationFactor: 0.1, // 10% jitter
		MaxElapsedTime:      0,   // Infinite
		MaxRetries:          0,   // Infinite
	}
}

// ConnectFunc is the operation to retry (returns error if needs retry).
type ConnectFunc func(ctx context.Context) error

// Reconnector manages connection retries with exponential backoff.
// Thread-safe for concurrent use.
type Reconnector struct {
	cfg    Config
	log    zerolog.Logger
	mu     sync.RWMutex
	cancel context.CancelFunc

	// Callbacks
	onRetry   func(attempt int, delay time.Duration, err error)
	onSuccess func(attempt int, elapsed time.Duration)
	onFailure func(attempt int, elapsed time.Duration, err error)

	// State
	attempts  int
	startTime time.Time
	running   bool
}

// New creates a new reconnector with the given configuration.
func New(cfg Config, log zerolog.Logger) *Reconnector {
	return &Reconnector{
		cfg: cfg,
		log: log.With().Str("component", "reconnector").Logger(),
	}
}

// SetOnRetry sets the callback invoked on each retry attempt.
func (r *Reconnector) SetOnRetry(fn func(attempt int, delay time.Duration, err error)) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.onRetry = fn
}

// SetOnSuccess sets the callback invoked when reconnection succeeds.
func (r *Reconnector) SetOnSuccess(fn func(attempt int, elapsed time.Duration)) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.onSuccess = fn
}

// SetOnFailure sets the callback invoked when reconnection fails permanently.
func (r *Reconnector) SetOnFailure(fn func(attempt int, elapsed time.Duration, err error)) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.onFailure = fn
}

// Run starts the reconnection loop with the given connect function.
// Blocks until successful, cancelled, or max retries exceeded.
func (r *Reconnector) Run(ctx context.Context, connect ConnectFunc) error {
	r.mu.Lock()
	if r.running {
		r.mu.Unlock()
		return nil // Already running
	}
	r.running = true
	r.attempts = 0
	r.startTime = time.Now()

	// Create cancellable context
	ctx, r.cancel = context.WithCancel(ctx)
	r.mu.Unlock()

	defer func() {
		r.mu.Lock()
		r.running = false
		r.mu.Unlock()
	}()

	// Create exponential backoff
	b := r.createBackoff()
	bWithContext := backoff.WithContext(b, ctx)

	operation := func() error {
		r.mu.Lock()
		r.attempts++
		r.mu.Unlock()

		err := connect(ctx)
		if err != nil {
			return err // Retry
		}
		return nil // Success
	}

	notify := func(err error, delay time.Duration) {
		r.mu.RLock()
		attempt := r.attempts
		onRetry := r.onRetry
		r.mu.RUnlock()

		r.log.Warn().
			Err(err).
			Int("attempt", attempt).
			Dur("next_delay", delay).
			Msg("Reconnection attempt failed, retrying")

		if onRetry != nil {
			onRetry(attempt, delay, err)
		}
	}

	err := backoff.RetryNotify(operation, bWithContext, notify)

	r.mu.RLock()
	attempt := r.attempts
	elapsed := time.Since(r.startTime)
	onSuccess := r.onSuccess
	onFailure := r.onFailure
	r.mu.RUnlock()

	if err != nil {
		// Failed permanently (cancelled or max retries)
		r.log.Error().
			Err(err).
			Int("attempts", attempt).
			Dur("elapsed", elapsed).
			Msg("Reconnection failed permanently")

		if onFailure != nil {
			onFailure(attempt, elapsed, err)
		}
		return err
	}

	// Success
	r.log.Info().
		Int("attempts", attempt).
		Dur("elapsed", elapsed).
		Msg("Reconnection successful")

	if onSuccess != nil {
		onSuccess(attempt, elapsed)
	}

	return nil
}

// Stop cancels any in-progress reconnection attempt.
func (r *Reconnector) Stop() {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.cancel != nil {
		r.cancel()
		r.cancel = nil
	}
	r.running = false
}

// IsRunning returns whether a reconnection is in progress.
func (r *Reconnector) IsRunning() bool {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.running
}

// Attempts returns the current retry attempt count.
func (r *Reconnector) Attempts() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.attempts
}

// createBackoff creates the backoff strategy from config.
func (r *Reconnector) createBackoff() backoff.BackOff {
	b := backoff.NewExponentialBackOff()
	b.InitialInterval = r.cfg.InitialInterval
	b.MaxInterval = r.cfg.MaxInterval
	b.Multiplier = r.cfg.Multiplier
	b.RandomizationFactor = r.cfg.RandomizationFactor

	if r.cfg.MaxElapsedTime > 0 {
		b.MaxElapsedTime = r.cfg.MaxElapsedTime
	} else {
		b.MaxElapsedTime = 0 // Infinite
	}

	// Wrap with max retries if configured
	if r.cfg.MaxRetries > 0 {
		return backoff.WithMaxRetries(b, r.cfg.MaxRetries)
	}

	return b
}

// Reset resets the reconnector state for a fresh start.
func (r *Reconnector) Reset() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.attempts = 0
	r.startTime = time.Time{}
}
