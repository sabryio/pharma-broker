package resilience

import (
	"context"
	"testing"
	"time"
)

func TestNewRateLimiter(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 20,
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	if rl == nil {
		t.Fatal("NewRateLimiter returned nil")
	}

	if !rl.IsEnabled() {
		t.Error("Rate limiter should be enabled")
	}

	if rl.ratePerMinute != 20 {
		t.Errorf("Expected rate 20, got %v", rl.ratePerMinute)
	}

	if rl.burstSize != 5 {
		t.Errorf("Expected burst size 5, got %v", rl.burstSize)
	}
}

func TestRateLimiter_Allow(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 60, // 1 per second
		BurstSize:     3,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	// Should allow burst size requests immediately
	for i := 0; i < 3; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed (within burst)", i+1)
		}
	}

	// Next request should be denied (no tokens left)
	if rl.Allow() {
		t.Error("Request should be denied (burst exhausted)")
	}

	stats := rl.GetStats()
	if stats["total_requests"] != 4 {
		t.Errorf("Expected 4 total requests, got %d", stats["total_requests"])
	}
	if stats["total_allowed"] != 3 {
		t.Errorf("Expected 3 allowed, got %d", stats["total_allowed"])
	}
	if stats["total_dropped"] != 1 {
		t.Errorf("Expected 1 dropped, got %d", stats["total_dropped"])
	}
}

func TestRateLimiter_Wait(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 600, // 10 per second for faster test
		BurstSize:     2,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	ctx := context.Background()

	// Exhaust burst
	for i := 0; i < 2; i++ {
		if err := rl.Wait(ctx); err != nil {
			t.Errorf("Wait %d should succeed: %v", i+1, err)
		}
	}

	// Next wait should block briefly then succeed
	start := time.Now()
	if err := rl.Wait(ctx); err != nil {
		t.Errorf("Wait should eventually succeed: %v", err)
	}
	elapsed := time.Since(start)

	// Should have waited at least some time (100ms / 10 per second = ~100ms per token)
	if elapsed < 50*time.Millisecond {
		t.Logf("Wait time was %v (expected some delay)", elapsed)
	}
}

func TestRateLimiter_WaitCancelled(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 1, // Very slow
		BurstSize:     1,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	// Exhaust the single token
	rl.Allow()

	// Create a context that cancels quickly
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()

	err := rl.Wait(ctx)
	if err == nil {
		t.Error("Wait should return error when context is cancelled")
	}

	stats := rl.GetStats()
	if stats["total_dropped"] != 1 {
		t.Errorf("Expected 1 dropped, got %d", stats["total_dropped"])
	}
}

func TestRateLimiter_Disabled(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 1,
		BurstSize:     1,
		Enabled:       false,
	}
	rl := NewRateLimiter(cfg)

	// Should allow all requests when disabled
	for i := 0; i < 100; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed when disabled", i+1)
		}
	}
}

func TestRateLimiter_SetEnabled(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 1,
		BurstSize:     1,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	// Exhaust token
	rl.Allow()

	// Should be denied
	if rl.Allow() {
		t.Error("Should be denied when enabled")
	}

	// Disable
	rl.SetEnabled(false)

	// Should now be allowed
	if !rl.Allow() {
		t.Error("Should be allowed when disabled")
	}
}

func TestRateLimiter_SetRate(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 20,
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	rl.SetRate(120, 10)

	if rl.ratePerMinute != 120 {
		t.Errorf("Expected rate 120, got %v", rl.ratePerMinute)
	}
	if rl.burstSize != 10 {
		t.Errorf("Expected burst size 10, got %v", rl.burstSize)
	}
}

func TestRateLimiter_Reset(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 60,
		BurstSize:     3,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	// Exhaust all tokens
	for i := 0; i < 3; i++ {
		rl.Allow()
	}

	// Should be denied
	if rl.Allow() {
		t.Error("Should be denied after exhausting tokens")
	}

	// Reset
	rl.Reset()

	// Should have full burst again
	for i := 0; i < 3; i++ {
		if !rl.Allow() {
			t.Errorf("Request %d should be allowed after reset", i+1)
		}
	}
}

func TestRateLimiter_GetCurrentTokens(t *testing.T) {
	cfg := RateLimiterConfig{
		RatePerMinute: 60,
		BurstSize:     5,
		Enabled:       true,
	}
	rl := NewRateLimiter(cfg)

	tokens := rl.GetCurrentTokens()
	if tokens != 5.0 {
		t.Errorf("Expected 5 tokens, got %v", tokens)
	}

	// Use 2 tokens
	rl.Allow()
	rl.Allow()

	tokens = rl.GetCurrentTokens()
	if tokens < 2.9 || tokens > 3.1 {
		t.Errorf("Expected ~3 tokens, got %v", tokens)
	}
}
