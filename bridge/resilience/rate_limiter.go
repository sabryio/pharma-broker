package resilience

import (
	"context"
	"pharma-bridge/ports"
	"sync"
	"sync/atomic"
	"time"
)

// RateLimitWaitTimeout is the max wait time for rate limit.
const RateLimitWaitTimeout = 30 * time.Second

// RateLimiterStats tracks rate limiter statistics.
type RateLimiterStats struct {
	TotalRequests   atomic.Int64 // Total send requests
	TotalAllowed    atomic.Int64 // Requests allowed immediately
	TotalWaited     atomic.Int64 // Requests that had to wait
	TotalDropped    atomic.Int64 // Requests dropped due to timeout
	TotalWaitTimeMs atomic.Int64 // Cumulative wait time in milliseconds
}

// RateLimiterConfig holds configuration for the rate limiter.
type RateLimiterConfig struct {
	RatePerMinute float64 // Messages allowed per minute
	BurstSize     int     // Maximum burst size
	Enabled       bool    // Whether rate limiting is enabled
}

// RateLimiter controls the rate of outgoing messages to prevent WhatsApp bans.
// Uses a token bucket algorithm with configurable rate and burst size.
type RateLimiter struct {
	// Configuration
	ratePerMinute float64
	burstSize     int
	enabled       atomic.Bool

	// Token bucket state
	tokens     float64
	lastRefill time.Time
	mu         sync.Mutex

	// Statistics
	stats RateLimiterStats
}

// NewRateLimiter creates a new rate limiter with the given configuration.
func NewRateLimiter(cfg RateLimiterConfig) *RateLimiter {
	rl := &RateLimiter{
		ratePerMinute: cfg.RatePerMinute,
		burstSize:     cfg.BurstSize,
		tokens:        float64(cfg.BurstSize), // Start with full bucket
		lastRefill:    time.Now(),
	}
	rl.enabled.Store(cfg.Enabled)

	return rl
}

// Wait blocks until a token is available or the context is cancelled.
// Returns nil if a token was acquired, or an error if the wait was cancelled/timed out.
func (rl *RateLimiter) Wait(ctx context.Context) error {
	rl.stats.TotalRequests.Add(1)

	// If disabled, allow immediately
	if !rl.enabled.Load() {
		rl.stats.TotalAllowed.Add(1)
		return nil
	}

	startWait := time.Now()

	for {
		// Try to acquire a token
		if rl.tryAcquire() {
			waitTime := time.Since(startWait)
			if waitTime > time.Millisecond {
				rl.stats.TotalWaited.Add(1)
				rl.stats.TotalWaitTimeMs.Add(waitTime.Milliseconds())
			} else {
				rl.stats.TotalAllowed.Add(1)
			}
			return nil
		}

		// Calculate time until next token
		waitDuration := rl.timeUntilNextToken()

		// Check context before waiting
		select {
		case <-ctx.Done():
			rl.stats.TotalDropped.Add(1)
			return ctx.Err()
		case <-time.After(waitDuration):
			// Continue loop to try acquiring again
		}
	}
}

// Allow checks if a message can be sent immediately without waiting.
// Returns true if allowed, false if rate limited.
func (rl *RateLimiter) Allow() bool {
	rl.stats.TotalRequests.Add(1)

	if !rl.enabled.Load() {
		rl.stats.TotalAllowed.Add(1)
		return true
	}

	if rl.tryAcquire() {
		rl.stats.TotalAllowed.Add(1)
		return true
	}

	rl.stats.TotalDropped.Add(1)
	return false
}

// tryAcquire attempts to acquire a token from the bucket.
// Returns true if successful, false if no tokens available.
func (rl *RateLimiter) tryAcquire() bool {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	rl.refillTokens()

	if rl.tokens >= 1.0 {
		rl.tokens -= 1.0
		return true
	}

	return false
}

// refillTokens adds tokens based on elapsed time since last refill.
// Must be called with mutex held.
func (rl *RateLimiter) refillTokens() {
	now := time.Now()
	elapsed := now.Sub(rl.lastRefill)
	rl.lastRefill = now

	// Calculate tokens to add: (elapsed_minutes * rate_per_minute)
	tokensToAdd := elapsed.Minutes() * rl.ratePerMinute

	// Add tokens, capped at burst size
	rl.tokens = min(rl.tokens+tokensToAdd, float64(rl.burstSize))
}

// timeUntilNextToken calculates how long until the next token is available.
func (rl *RateLimiter) timeUntilNextToken() time.Duration {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if rl.tokens >= 1.0 {
		return 0
	}

	// Time for one token: 60 seconds / rate_per_minute
	tokenInterval := time.Duration(60.0/rl.ratePerMinute*1000) * time.Millisecond
	tokensNeeded := 1.0 - rl.tokens

	return time.Duration(float64(tokenInterval) * tokensNeeded)
}

// SetEnabled enables or disables the rate limiter.
func (rl *RateLimiter) SetEnabled(enabled bool) {
	rl.enabled.Store(enabled)
}

// IsEnabled returns whether the rate limiter is enabled.
func (rl *RateLimiter) IsEnabled() bool {
	return rl.enabled.Load()
}

// SetRate updates the rate limit configuration.
func (rl *RateLimiter) SetRate(ratePerMinute float64, burstSize int) {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	if ratePerMinute > 0 {
		rl.ratePerMinute = ratePerMinute
	}
	if burstSize > 0 {
		rl.burstSize = burstSize
		// Cap current tokens at new burst size
		rl.tokens = min(rl.tokens, float64(burstSize))
	}
}

// GetStats returns a snapshot of rate limiter statistics.
func (rl *RateLimiter) GetStats() map[string]int64 {
	return map[string]int64{
		"total_requests":     rl.stats.TotalRequests.Load(),
		"total_allowed":      rl.stats.TotalAllowed.Load(),
		"total_waited":       rl.stats.TotalWaited.Load(),
		"total_dropped":      rl.stats.TotalDropped.Load(),
		"total_wait_time_ms": rl.stats.TotalWaitTimeMs.Load(),
	}
}

// GetCurrentTokens returns the current number of available tokens.
func (rl *RateLimiter) GetCurrentTokens() float64 {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	rl.refillTokens()
	return rl.tokens
}

// Reset resets the rate limiter to its initial state.
func (rl *RateLimiter) Reset() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	rl.tokens = float64(rl.burstSize)
	rl.lastRefill = time.Now()
}

// Ensure RateLimiter implements the interface
var _ ports.RateLimiter = (*RateLimiter)(nil)
