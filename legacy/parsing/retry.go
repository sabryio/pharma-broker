package parsing

import (
	"context"
	"errors"
	"fmt"
	"math"
	"math/rand/v2"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// AI Retry Configuration and Types
// =============================================================================

// RetryConfig holds configuration for the retry mechanism.
type RetryConfig struct {
	MaxRetries int           // Maximum number of retry attempts (0 = no retries)
	BaseDelay  time.Duration // Initial delay before first retry
	MaxDelay   time.Duration // Maximum delay between retries
	Multiplier float64       // Exponential backoff multiplier
	Jitter     float64       // Jitter factor (0.0-1.0) for randomization
}

// DefaultRetryConfig returns sensible default retry configuration.
func DefaultRetryConfig() RetryConfig {
	return RetryConfig{
		MaxRetries: DefaultMaxRetries,
		BaseDelay:  DefaultRetryBaseDelay,
		MaxDelay:   DefaultRetryMaxDelay,
		Multiplier: DefaultRetryMultiplier,
		Jitter:     DefaultRetryJitter,
	}
}

// RetryStats tracks retry statistics for monitoring.
type RetryStats struct {
	TotalAttempts   atomic.Int64 // Total AI call attempts
	TotalSuccesses  atomic.Int64 // Successful calls (including retries)
	TotalFailures   atomic.Int64 // Final failures after all retries
	TotalRetries    atomic.Int64 // Number of retry attempts made
	TotalWaitTimeMs atomic.Int64 // Cumulative wait time in milliseconds
}

// GetStats returns a snapshot of retry statistics.
func (s *RetryStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_attempts":     s.TotalAttempts.Load(),
		"total_successes":    s.TotalSuccesses.Load(),
		"total_failures":     s.TotalFailures.Load(),
		"total_retries":      s.TotalRetries.Load(),
		"total_wait_time_ms": s.TotalWaitTimeMs.Load(),
	}
}

// =============================================================================
// Retryable Errors
// =============================================================================

// RetryableError wraps an error and indicates whether it should be retried.
type RetryableError struct {
	Err       error
	Retryable bool
	Reason    string
}

func (e *RetryableError) Error() string {
	if e.Reason != "" {
		return fmt.Sprintf("%s: %v", e.Reason, e.Err)
	}
	return e.Err.Error()
}

func (e *RetryableError) Unwrap() error {
	return e.Err
}

// IsRetryable checks if an error should be retried.
func IsRetryable(err error) bool {
	if err == nil {
		return false
	}

	// Check for RetryableError wrapper
	var retryableErr *RetryableError
	if errors.As(err, &retryableErr) {
		return retryableErr.Retryable
	}

	// Check for context errors (not retryable)
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return false
	}

	// Default: assume transient errors are retryable
	// This includes network errors, timeouts, rate limits, etc.
	errStr := err.Error()
	retryablePatterns := []string{
		"timeout",
		"connection refused",
		"connection reset",
		"temporary failure",
		"rate limit",
		"429",
		"503",
		"502",
		"500",
		"EOF",
		"broken pipe",
	}

	for _, pattern := range retryablePatterns {
		if containsIgnoreCase(errStr, pattern) {
			return true
		}
	}

	return true // Default to retryable for unknown errors
}

// containsIgnoreCase checks if s contains substr (case-insensitive).
func containsIgnoreCase(s, substr string) bool {
	sLower := toLower(s)
	substrLower := toLower(substr)
	return contains(sLower, substrLower)
}

// toLower converts string to lowercase (simple ASCII version).
func toLower(s string) string {
	b := make([]byte, len(s))
	for i := range s {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			c += 'a' - 'A'
		}
		b[i] = c
	}
	return string(b)
}

// contains checks if s contains substr.
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 || findSubstring(s, substr) >= 0)
}

// findSubstring finds the index of substr in s, or -1 if not found.
func findSubstring(s, substr string) int {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}

// =============================================================================
// Retry Executor
// =============================================================================

// AIRetryExecutor handles retry logic for AI operations.
type AIRetryExecutor struct {
	config RetryConfig
	stats  RetryStats
	log    zerolog.Logger
}

// NewAIRetryExecutor creates a new retry executor with the given configuration.
func NewAIRetryExecutor(cfg RetryConfig, log zerolog.Logger) *AIRetryExecutor {
	if cfg.MaxRetries < 0 {
		cfg.MaxRetries = 0
	}
	if cfg.BaseDelay <= 0 {
		cfg.BaseDelay = DefaultRetryBaseDelay
	}
	if cfg.MaxDelay <= 0 {
		cfg.MaxDelay = DefaultRetryMaxDelay
	}
	if cfg.Multiplier <= 0 {
		cfg.Multiplier = DefaultRetryMultiplier
	}
	if cfg.Jitter < 0 || cfg.Jitter > 1 {
		cfg.Jitter = DefaultRetryJitter
	}

	return &AIRetryExecutor{
		config: cfg,
		log:    log.With().Str("component", "ai-retry").Logger(),
	}
}

// Execute runs the given function with retry logic.
// Returns the result and any error after all retries are exhausted.
func (r *AIRetryExecutor) Execute(
	ctx context.Context,
	operation string,
	fn func(ctx context.Context) (any, error),
) (any, error) {
	r.stats.TotalAttempts.Add(1)

	var lastErr error
	for attempt := 0; attempt <= r.config.MaxRetries; attempt++ {
		// Check context before each attempt
		select {
		case <-ctx.Done():
			r.stats.TotalFailures.Add(1)
			return nil, ctx.Err()
		default:
		}

		// Execute the operation
		result, err := fn(ctx)
		if err == nil {
			r.stats.TotalSuccesses.Add(1)
			if attempt > 0 {
				r.log.Info().
					Str("operation", operation).
					Int("attempt", attempt+1).
					Msg("Operation succeeded after retry")
			}
			return result, nil
		}

		lastErr = err

		// Check if we should retry
		if attempt >= r.config.MaxRetries {
			break
		}

		if !IsRetryable(err) {
			r.log.Warn().
				Err(err).
				Str("operation", operation).
				Int("attempt", attempt+1).
				Msg("Non-retryable error, giving up")
			break
		}

		// Calculate delay with exponential backoff and jitter
		delay := r.calculateDelay(attempt)
		r.stats.TotalRetries.Add(1)
		r.stats.TotalWaitTimeMs.Add(delay.Milliseconds())

		r.log.Warn().
			Err(err).
			Str("operation", operation).
			Int("attempt", attempt+1).
			Int("max_attempts", r.config.MaxRetries+1).
			Dur("retry_delay", delay).
			Msg("Retrying after transient error")

		// Wait before retry
		select {
		case <-ctx.Done():
			r.stats.TotalFailures.Add(1)
			return nil, ctx.Err()
		case <-time.After(delay):
			// Continue to next attempt
		}
	}

	r.stats.TotalFailures.Add(1)
	return nil, fmt.Errorf("operation %s failed after %d attempts: %w",
		operation, r.config.MaxRetries+1, lastErr)
}

// calculateDelay computes the delay for a given attempt using exponential backoff with jitter.
func (r *AIRetryExecutor) calculateDelay(attempt int) time.Duration {
	// Exponential backoff: baseDelay * multiplier^attempt
	delay := float64(r.config.BaseDelay) * math.Pow(r.config.Multiplier, float64(attempt))

	// Cap at max delay
	if delay > float64(r.config.MaxDelay) {
		delay = float64(r.config.MaxDelay)
	}

	// Add jitter: delay * (1 ± jitter)
	if r.config.Jitter > 0 {
		jitterRange := delay * r.config.Jitter
		jitter := (rand.Float64()*2 - 1) * jitterRange // Random value in [-jitterRange, +jitterRange]
		delay += jitter
	}

	// Ensure non-negative
	if delay < 0 {
		delay = float64(r.config.BaseDelay)
	}

	return time.Duration(delay)
}

// GetStats returns the current retry statistics.
func (r *AIRetryExecutor) GetStats() map[string]int64 {
	return r.stats.GetStats()
}

// GetConfig returns the current retry configuration.
func (r *AIRetryExecutor) GetConfig() RetryConfig {
	return r.config
}

// SetConfig updates the retry configuration.
func (r *AIRetryExecutor) SetConfig(cfg RetryConfig) {
	r.config = cfg
	r.log.Info().
		Int("max_retries", cfg.MaxRetries).
		Dur("base_delay", cfg.BaseDelay).
		Dur("max_delay", cfg.MaxDelay).
		Float64("multiplier", cfg.Multiplier).
		Float64("jitter", cfg.Jitter).
		Msg("Retry configuration updated")
}
