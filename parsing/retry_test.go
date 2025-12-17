package parsing

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// RetryConfig Tests
// =============================================================================

func TestDefaultRetryConfig(t *testing.T) {
	cfg := DefaultRetryConfig()

	if cfg.MaxRetries != DefaultMaxRetries {
		t.Errorf("MaxRetries = %d, want %d", cfg.MaxRetries, DefaultMaxRetries)
	}
	if cfg.BaseDelay != DefaultRetryBaseDelay {
		t.Errorf("BaseDelay = %v, want %v", cfg.BaseDelay, DefaultRetryBaseDelay)
	}
	if cfg.MaxDelay != DefaultRetryMaxDelay {
		t.Errorf("MaxDelay = %v, want %v", cfg.MaxDelay, DefaultRetryMaxDelay)
	}
	if cfg.Multiplier != DefaultRetryMultiplier {
		t.Errorf("Multiplier = %f, want %f", cfg.Multiplier, DefaultRetryMultiplier)
	}
	if cfg.Jitter != DefaultRetryJitter {
		t.Errorf("Jitter = %f, want %f", cfg.Jitter, DefaultRetryJitter)
	}
}

// =============================================================================
// RetryStats Tests
// =============================================================================

func TestRetryStats_GetStats(t *testing.T) {
	stats := &RetryStats{}
	stats.TotalAttempts.Store(10)
	stats.TotalSuccesses.Store(8)
	stats.TotalFailures.Store(2)
	stats.TotalRetries.Store(5)
	stats.TotalWaitTimeMs.Store(1500)

	result := stats.GetStats()

	if result["total_attempts"] != 10 {
		t.Errorf("total_attempts = %d, want 10", result["total_attempts"])
	}
	if result["total_successes"] != 8 {
		t.Errorf("total_successes = %d, want 8", result["total_successes"])
	}
	if result["total_failures"] != 2 {
		t.Errorf("total_failures = %d, want 2", result["total_failures"])
	}
	if result["total_retries"] != 5 {
		t.Errorf("total_retries = %d, want 5", result["total_retries"])
	}
	if result["total_wait_time_ms"] != 1500 {
		t.Errorf("total_wait_time_ms = %d, want 1500", result["total_wait_time_ms"])
	}
}

func TestRetryStats_Atomicity(t *testing.T) {
	stats := &RetryStats{}
	const goroutines = 100
	const iterations = 100

	done := make(chan struct{})
	for i := 0; i < goroutines; i++ {
		go func() {
			for j := 0; j < iterations; j++ {
				stats.TotalAttempts.Add(1)
				stats.TotalSuccesses.Add(1)
				stats.TotalRetries.Add(1)
			}
			done <- struct{}{}
		}()
	}

	for i := 0; i < goroutines; i++ {
		<-done
	}

	expected := int64(goroutines * iterations)
	if stats.TotalAttempts.Load() != expected {
		t.Errorf("TotalAttempts = %d, want %d", stats.TotalAttempts.Load(), expected)
	}
}

// =============================================================================
// RetryableError Tests
// =============================================================================

func TestRetryableError_Error(t *testing.T) {
	tests := []struct {
		name     string
		err      *RetryableError
		expected string
	}{
		{
			name: "with reason",
			err: &RetryableError{
				Err:       errors.New("connection failed"),
				Retryable: true,
				Reason:    "network error",
			},
			expected: "network error: connection failed",
		},
		{
			name: "without reason",
			err: &RetryableError{
				Err:       errors.New("connection failed"),
				Retryable: true,
				Reason:    "",
			},
			expected: "connection failed",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.Error(); got != tt.expected {
				t.Errorf("Error() = %q, want %q", got, tt.expected)
			}
		})
	}
}

func TestRetryableError_Unwrap(t *testing.T) {
	originalErr := errors.New("original error")
	retryErr := &RetryableError{Err: originalErr, Retryable: true}

	if unwrapped := retryErr.Unwrap(); unwrapped != originalErr {
		t.Errorf("Unwrap() = %v, want %v", unwrapped, originalErr)
	}
}

// =============================================================================
// IsRetryable Tests
// =============================================================================

func TestIsRetryable(t *testing.T) {
	tests := []struct {
		name     string
		err      error
		expected bool
	}{
		{
			name:     "nil error",
			err:      nil,
			expected: false,
		},
		{
			name:     "context canceled",
			err:      context.Canceled,
			expected: false,
		},
		{
			name:     "context deadline exceeded",
			err:      context.DeadlineExceeded,
			expected: false,
		},
		{
			name:     "retryable error wrapper - true",
			err:      &RetryableError{Err: errors.New("test"), Retryable: true},
			expected: true,
		},
		{
			name:     "retryable error wrapper - false",
			err:      &RetryableError{Err: errors.New("test"), Retryable: false},
			expected: false,
		},
		{
			name:     "timeout error",
			err:      errors.New("connection timeout"),
			expected: true,
		},
		{
			name:     "connection refused",
			err:      errors.New("dial tcp: connection refused"),
			expected: true,
		},
		{
			name:     "rate limit 429",
			err:      errors.New("HTTP 429 Too Many Requests"),
			expected: true,
		},
		{
			name:     "server error 500",
			err:      errors.New("HTTP 500 Internal Server Error"),
			expected: true,
		},
		{
			name:     "server error 502",
			err:      errors.New("HTTP 502 Bad Gateway"),
			expected: true,
		},
		{
			name:     "server error 503",
			err:      errors.New("HTTP 503 Service Unavailable"),
			expected: true,
		},
		{
			name:     "EOF error",
			err:      errors.New("unexpected EOF"),
			expected: true,
		},
		{
			name:     "broken pipe",
			err:      errors.New("write: broken pipe"),
			expected: true,
		},
		{
			name:     "unknown error - defaults to retryable",
			err:      errors.New("some unknown error"),
			expected: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsRetryable(tt.err); got != tt.expected {
				t.Errorf("IsRetryable(%v) = %v, want %v", tt.err, got, tt.expected)
			}
		})
	}
}

// =============================================================================
// AIRetryExecutor Tests
// =============================================================================

func TestNewAIRetryExecutor_DefaultValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{} // All zero values

	executor := NewAIRetryExecutor(cfg, log)

	// MaxRetries=0 is valid (no retries)
	if executor.config.MaxRetries != 0 {
		t.Errorf("MaxRetries = %d, want 0", executor.config.MaxRetries)
	}
	// Zero durations get replaced with defaults
	if executor.config.BaseDelay != DefaultRetryBaseDelay {
		t.Errorf("BaseDelay = %v, want %v", executor.config.BaseDelay, DefaultRetryBaseDelay)
	}
	if executor.config.MaxDelay != DefaultRetryMaxDelay {
		t.Errorf("MaxDelay = %v, want %v", executor.config.MaxDelay, DefaultRetryMaxDelay)
	}
	if executor.config.Multiplier != DefaultRetryMultiplier {
		t.Errorf("Multiplier = %f, want %f", executor.config.Multiplier, DefaultRetryMultiplier)
	}
	// Jitter=0 is valid (no jitter), should NOT be replaced
	if executor.config.Jitter != 0 {
		t.Errorf("Jitter = %f, want 0 (valid value)", executor.config.Jitter)
	}
}

func TestNewAIRetryExecutor_NegativeValues(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: -5,
		BaseDelay:  -1 * time.Second,
		MaxDelay:   -1 * time.Second,
		Multiplier: -2.0,
		Jitter:     -0.5,
	}

	executor := NewAIRetryExecutor(cfg, log)

	if executor.config.MaxRetries != 0 {
		t.Errorf("MaxRetries = %d, want 0 (clamped)", executor.config.MaxRetries)
	}
	if executor.config.BaseDelay != DefaultRetryBaseDelay {
		t.Errorf("BaseDelay should be default when negative")
	}
	if executor.config.MaxDelay != DefaultRetryMaxDelay {
		t.Errorf("MaxDelay should be default when negative")
	}
	if executor.config.Multiplier != DefaultRetryMultiplier {
		t.Errorf("Multiplier should be default when negative")
	}
	if executor.config.Jitter != DefaultRetryJitter {
		t.Errorf("Jitter should be default when out of range")
	}
}

func TestAIRetryExecutor_Execute_Success(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultRetryConfig()
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	expectedResult := "success"

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		return expectedResult, nil
	})

	if err != nil {
		t.Errorf("Execute() error = %v, want nil", err)
	}
	if result != expectedResult {
		t.Errorf("Execute() result = %v, want %v", result, expectedResult)
	}

	stats := executor.GetStats()
	if stats["total_attempts"] != 1 {
		t.Errorf("total_attempts = %d, want 1", stats["total_attempts"])
	}
	if stats["total_successes"] != 1 {
		t.Errorf("total_successes = %d, want 1", stats["total_successes"])
	}
	if stats["total_retries"] != 0 {
		t.Errorf("total_retries = %d, want 0", stats["total_retries"])
	}
}

func TestAIRetryExecutor_Execute_RetryThenSuccess(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 3,
		BaseDelay:  10 * time.Millisecond,
		MaxDelay:   100 * time.Millisecond,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	var attempts int32

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		attempt := atomic.AddInt32(&attempts, 1)
		if attempt < 3 {
			return nil, errors.New("temporary failure")
		}
		return "success", nil
	})

	if err != nil {
		t.Errorf("Execute() error = %v, want nil", err)
	}
	if result != "success" {
		t.Errorf("Execute() result = %v, want success", result)
	}
	if attempts != 3 {
		t.Errorf("attempts = %d, want 3", attempts)
	}

	stats := executor.GetStats()
	if stats["total_successes"] != 1 {
		t.Errorf("total_successes = %d, want 1", stats["total_successes"])
	}
	if stats["total_retries"] != 2 {
		t.Errorf("total_retries = %d, want 2", stats["total_retries"])
	}
}

func TestAIRetryExecutor_Execute_AllRetriesFail(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 2,
		BaseDelay:  5 * time.Millisecond,
		MaxDelay:   50 * time.Millisecond,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	var attempts int32

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, errors.New("persistent failure")
	})

	if err == nil {
		t.Error("Execute() error = nil, want error")
	}
	if result != nil {
		t.Errorf("Execute() result = %v, want nil", result)
	}
	if attempts != 3 { // 1 initial + 2 retries
		t.Errorf("attempts = %d, want 3", attempts)
	}

	stats := executor.GetStats()
	if stats["total_failures"] != 1 {
		t.Errorf("total_failures = %d, want 1", stats["total_failures"])
	}
	if stats["total_retries"] != 2 {
		t.Errorf("total_retries = %d, want 2", stats["total_retries"])
	}
}

func TestAIRetryExecutor_Execute_NonRetryableError(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 3,
		BaseDelay:  10 * time.Millisecond,
		MaxDelay:   100 * time.Millisecond,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	var attempts int32

	nonRetryableErr := &RetryableError{
		Err:       errors.New("validation error"),
		Retryable: false,
		Reason:    "invalid input",
	}

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, nonRetryableErr
	})

	if err == nil {
		t.Error("Execute() error = nil, want error")
	}
	if result != nil {
		t.Errorf("Execute() result = %v, want nil", result)
	}
	if attempts != 1 { // Should not retry
		t.Errorf("attempts = %d, want 1 (no retries for non-retryable)", attempts)
	}
}

func TestAIRetryExecutor_Execute_ContextCanceled(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 3,
		BaseDelay:  100 * time.Millisecond,
		MaxDelay:   1 * time.Second,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx, cancel := context.WithCancel(context.Background())
	var attempts int32

	// Cancel after first attempt
	go func() {
		time.Sleep(20 * time.Millisecond)
		cancel()
	}()

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, errors.New("temporary failure")
	})

	if !errors.Is(err, context.Canceled) {
		t.Errorf("Execute() error = %v, want context.Canceled", err)
	}
	if result != nil {
		t.Errorf("Execute() result = %v, want nil", result)
	}
}

func TestAIRetryExecutor_Execute_ContextCanceledBeforeStart(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultRetryConfig()
	executor := NewAIRetryExecutor(cfg, log)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Cancel immediately

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		t.Error("Operation should not be called when context is already canceled")
		return "should not reach", nil
	})

	if !errors.Is(err, context.Canceled) {
		t.Errorf("Execute() error = %v, want context.Canceled", err)
	}
	if result != nil {
		t.Errorf("Execute() result = %v, want nil", result)
	}
}

func TestAIRetryExecutor_Execute_NoRetries(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 0, // No retries
		BaseDelay:  10 * time.Millisecond,
		MaxDelay:   100 * time.Millisecond,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	var attempts int32

	result, err := executor.Execute(ctx, "test_op", func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, errors.New("failure")
	})

	if err == nil {
		t.Error("Execute() error = nil, want error")
	}
	if result != nil {
		t.Errorf("Execute() result = %v, want nil", result)
	}
	if attempts != 1 {
		t.Errorf("attempts = %d, want 1 (no retries configured)", attempts)
	}
}

// =============================================================================
// calculateDelay Tests
// =============================================================================

func TestAIRetryExecutor_CalculateDelay(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 5,
		BaseDelay:  100 * time.Millisecond,
		MaxDelay:   5 * time.Second,
		Multiplier: 2.0,
		Jitter:     0, // No jitter for predictable testing
	}
	executor := NewAIRetryExecutor(cfg, log)

	tests := []struct {
		attempt  int
		expected time.Duration
	}{
		{0, 100 * time.Millisecond},  // 100ms * 2^0 = 100ms
		{1, 200 * time.Millisecond},  // 100ms * 2^1 = 200ms
		{2, 400 * time.Millisecond},  // 100ms * 2^2 = 400ms
		{3, 800 * time.Millisecond},  // 100ms * 2^3 = 800ms
		{4, 1600 * time.Millisecond}, // 100ms * 2^4 = 1600ms
	}

	for _, tt := range tests {
		t.Run("attempt_"+string(rune('0'+tt.attempt)), func(t *testing.T) {
			delay := executor.calculateDelay(tt.attempt)
			if delay != tt.expected {
				t.Errorf("calculateDelay(%d) = %v, want %v", tt.attempt, delay, tt.expected)
			}
		})
	}
}

func TestAIRetryExecutor_CalculateDelay_MaxDelayCap(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 10,
		BaseDelay:  100 * time.Millisecond,
		MaxDelay:   500 * time.Millisecond, // Low cap
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	// Attempt 5: 100ms * 2^5 = 3200ms, but capped at 500ms
	delay := executor.calculateDelay(5)
	if delay != 500*time.Millisecond {
		t.Errorf("calculateDelay(5) = %v, want 500ms (capped)", delay)
	}
}

func TestAIRetryExecutor_CalculateDelay_WithJitter(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 3,
		BaseDelay:  100 * time.Millisecond,
		MaxDelay:   5 * time.Second,
		Multiplier: 2.0,
		Jitter:     0.5, // 50% jitter
	}
	executor := NewAIRetryExecutor(cfg, log)

	// Run multiple times to verify jitter adds randomness
	baseDelay := 100 * time.Millisecond
	minExpected := time.Duration(float64(baseDelay) * 0.5) // 50ms
	maxExpected := time.Duration(float64(baseDelay) * 1.5) // 150ms

	for i := 0; i < 10; i++ {
		delay := executor.calculateDelay(0)
		if delay < minExpected || delay > maxExpected {
			t.Errorf("calculateDelay(0) with jitter = %v, want between %v and %v", delay, minExpected, maxExpected)
		}
	}
}

// =============================================================================
// GetConfig and SetConfig Tests
// =============================================================================

func TestAIRetryExecutor_GetConfig(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 5,
		BaseDelay:  200 * time.Millisecond,
		MaxDelay:   10 * time.Second,
		Multiplier: 3.0,
		Jitter:     0.2,
	}
	executor := NewAIRetryExecutor(cfg, log)

	got := executor.GetConfig()
	if got.MaxRetries != cfg.MaxRetries {
		t.Errorf("GetConfig().MaxRetries = %d, want %d", got.MaxRetries, cfg.MaxRetries)
	}
	if got.BaseDelay != cfg.BaseDelay {
		t.Errorf("GetConfig().BaseDelay = %v, want %v", got.BaseDelay, cfg.BaseDelay)
	}
}

func TestAIRetryExecutor_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	executor := NewAIRetryExecutor(DefaultRetryConfig(), log)

	newCfg := RetryConfig{
		MaxRetries: 10,
		BaseDelay:  500 * time.Millisecond,
		MaxDelay:   1 * time.Minute,
		Multiplier: 1.5,
		Jitter:     0.3,
	}
	executor.SetConfig(newCfg)

	got := executor.GetConfig()
	if got.MaxRetries != newCfg.MaxRetries {
		t.Errorf("After SetConfig, MaxRetries = %d, want %d", got.MaxRetries, newCfg.MaxRetries)
	}
	if got.BaseDelay != newCfg.BaseDelay {
		t.Errorf("After SetConfig, BaseDelay = %v, want %v", got.BaseDelay, newCfg.BaseDelay)
	}
}

// =============================================================================
// Helper Function Tests
// =============================================================================

func TestContainsIgnoreCase(t *testing.T) {
	tests := []struct {
		s        string
		substr   string
		expected bool
	}{
		{"Hello World", "world", true},
		{"Hello World", "WORLD", true},
		{"Hello World", "foo", false},
		{"timeout error", "TIMEOUT", true},
		{"", "test", false},
		{"test", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.s+"_"+tt.substr, func(t *testing.T) {
			if got := containsIgnoreCase(tt.s, tt.substr); got != tt.expected {
				t.Errorf("containsIgnoreCase(%q, %q) = %v, want %v", tt.s, tt.substr, got, tt.expected)
			}
		})
	}
}

func TestToLower(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"HELLO", "hello"},
		{"Hello World", "hello world"},
		{"already lowercase", "already lowercase"},
		{"MiXeD CaSe", "mixed case"},
		{"", ""},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			if got := toLower(tt.input); got != tt.expected {
				t.Errorf("toLower(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

// =============================================================================
// Concurrent Execution Tests
// =============================================================================

func TestAIRetryExecutor_ConcurrentExecution(t *testing.T) {
	log := zerolog.Nop()
	cfg := RetryConfig{
		MaxRetries: 2,
		BaseDelay:  5 * time.Millisecond,
		MaxDelay:   50 * time.Millisecond,
		Multiplier: 2.0,
		Jitter:     0,
	}
	executor := NewAIRetryExecutor(cfg, log)

	ctx := context.Background()
	const goroutines = 50

	done := make(chan error, goroutines)
	for i := 0; i < goroutines; i++ {
		go func(id int) {
			_, err := executor.Execute(ctx, "concurrent_test", func(ctx context.Context) (any, error) {
				return id, nil
			})
			done <- err
		}(i)
	}

	for i := 0; i < goroutines; i++ {
		if err := <-done; err != nil {
			t.Errorf("Concurrent execution %d failed: %v", i, err)
		}
	}

	stats := executor.GetStats()
	if stats["total_attempts"] != goroutines {
		t.Errorf("total_attempts = %d, want %d", stats["total_attempts"], goroutines)
	}
	if stats["total_successes"] != goroutines {
		t.Errorf("total_successes = %d, want %d", stats["total_successes"], goroutines)
	}
}

// =============================================================================
// Retry Constants Tests
// =============================================================================

func TestRetryConstants(t *testing.T) {
	if DefaultMaxRetries <= 0 {
		t.Error("DefaultMaxRetries should be positive")
	}
	if DefaultRetryBaseDelay <= 0 {
		t.Error("DefaultRetryBaseDelay should be positive")
	}
	if DefaultRetryMaxDelay <= DefaultRetryBaseDelay {
		t.Error("DefaultRetryMaxDelay should be greater than DefaultRetryBaseDelay")
	}
	if DefaultRetryMultiplier <= 1.0 {
		t.Error("DefaultRetryMultiplier should be greater than 1.0")
	}
	if DefaultRetryJitter < 0 || DefaultRetryJitter > 1 {
		t.Error("DefaultRetryJitter should be between 0 and 1")
	}
}
