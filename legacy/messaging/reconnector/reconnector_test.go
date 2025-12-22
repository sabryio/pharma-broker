package reconnector

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

	"github.com/rs/zerolog"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDefaultReconnectorConfig(t *testing.T) {
	cfg := DefaultReconnectorConfig()

	assert.Equal(t, 5*time.Second, cfg.InitialInterval)
	assert.Equal(t, 5*time.Minute, cfg.MaxInterval)
	assert.Equal(t, 2.0, cfg.Multiplier)
	assert.Equal(t, 0.1, cfg.RandomizationFactor)
	assert.Equal(t, time.Duration(0), cfg.MaxElapsedTime)
	assert.Equal(t, uint64(0), cfg.MaxRetries)
}

func TestNewReconnector(t *testing.T) {
	cfg := DefaultReconnectorConfig()
	r := NewReconnector(cfg, zerolog.Nop())

	require.NotNil(t, r)
	assert.False(t, r.IsRunning())
	assert.Equal(t, 0, r.Attempts())
}

func TestReconnector_ImmediateSuccess(t *testing.T) {
	cfg := DefaultReconnectorConfig()
	cfg.InitialInterval = 10 * time.Millisecond
	r := NewReconnector(cfg, zerolog.Nop())

	var successCalled bool
	r.SetOnSuccess(func(attempt int, elapsed time.Duration) {
		successCalled = true
		assert.Equal(t, 1, attempt)
	})

	// Connect succeeds immediately
	err := r.Run(context.Background(), func(ctx context.Context) error {
		return nil
	})

	assert.NoError(t, err)
	assert.True(t, successCalled)
	assert.Equal(t, 1, r.Attempts())
	assert.False(t, r.IsRunning())
}

func TestReconnector_SuccessAfterRetries(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         50 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          5,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	var attempts atomic.Int32
	var retryCount atomic.Int32

	r.SetOnRetry(func(attempt int, delay time.Duration, err error) {
		retryCount.Add(1)
	})

	// Fail twice, then succeed
	err := r.Run(context.Background(), func(ctx context.Context) error {
		a := attempts.Add(1)
		if a < 3 {
			return errors.New("connection failed")
		}
		return nil
	})

	assert.NoError(t, err)
	assert.Equal(t, int32(3), attempts.Load())
	assert.Equal(t, int32(2), retryCount.Load()) // 2 retries before success
}

func TestReconnector_MaxRetriesExceeded(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     5 * time.Millisecond,
		MaxInterval:         10 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          3,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	var failureCalled bool
	r.SetOnFailure(func(attempt int, elapsed time.Duration, err error) {
		failureCalled = true
		assert.Equal(t, 4, attempt) // 1 initial + 3 retries
	})

	// Always fail
	err := r.Run(context.Background(), func(ctx context.Context) error {
		return errors.New("always fails")
	})

	assert.Error(t, err)
	assert.True(t, failureCalled)
	assert.Equal(t, 4, r.Attempts())
}

func TestReconnector_ContextCancellation(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     100 * time.Millisecond,
		MaxInterval:         1 * time.Second,
		Multiplier:          2.0,
		RandomizationFactor: 0,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	ctx, cancel := context.WithCancel(context.Background())

	var attempts atomic.Int32

	go func() {
		// Cancel after first attempt starts
		time.Sleep(50 * time.Millisecond)
		cancel()
	}()

	err := r.Run(ctx, func(ctx context.Context) error {
		attempts.Add(1)
		return errors.New("connection failed")
	})

	assert.Error(t, err)
	assert.True(t, errors.Is(err, context.Canceled))
}

func TestReconnector_Stop(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     100 * time.Millisecond,
		MaxInterval:         1 * time.Second,
		Multiplier:          2.0,
		RandomizationFactor: 0,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	done := make(chan struct{})

	go func() {
		err := r.Run(context.Background(), func(ctx context.Context) error {
			return errors.New("connection failed")
		})
		assert.Error(t, err) // Cancelled
		close(done)
	}()

	// Wait for reconnector to start
	time.Sleep(50 * time.Millisecond)
	assert.True(t, r.IsRunning())

	// Stop it
	r.Stop()

	select {
	case <-done:
		// Success
	case <-time.After(500 * time.Millisecond):
		t.Fatal("Reconnector did not stop in time")
	}

	assert.False(t, r.IsRunning())
}

func TestReconnector_Stats(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         50 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          2,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	// Before running
	stats := r.Stats()
	assert.False(t, stats.Running)
	assert.Equal(t, 0, stats.Attempts)

	var attempts atomic.Int32
	r.Run(context.Background(), func(ctx context.Context) error {
		a := attempts.Add(1)
		if a < 2 {
			return errors.New("fail")
		}
		return nil
	})

	// After running
	stats = r.Stats()
	assert.False(t, stats.Running)
	assert.Equal(t, 2, stats.Attempts)
	assert.True(t, stats.Elapsed > 0)
}

func TestReconnector_Reset(t *testing.T) {
	cfg := DefaultReconnectorConfig()
	cfg.InitialInterval = 10 * time.Millisecond
	cfg.MaxRetries = 1
	r := NewReconnector(cfg, zerolog.Nop())

	// Run once (will fail after max retries)
	r.Run(context.Background(), func(ctx context.Context) error {
		return errors.New("fail")
	})

	assert.Greater(t, r.Attempts(), 0)

	// Reset
	r.Reset()

	assert.Equal(t, 0, r.Attempts())
	assert.Equal(t, time.Duration(0), r.Elapsed())
}

func TestReconnector_ConcurrentRuns(t *testing.T) {
	cfg := DefaultReconnectorConfig()
	cfg.InitialInterval = 50 * time.Millisecond
	r := NewReconnector(cfg, zerolog.Nop())

	done1 := make(chan error)
	done2 := make(chan error)

	// Start first run
	go func() {
		done1 <- r.Run(context.Background(), func(ctx context.Context) error {
			time.Sleep(100 * time.Millisecond)
			return nil
		})
	}()

	// Wait for it to start
	time.Sleep(20 * time.Millisecond)

	// Try to start second run (should return immediately)
	go func() {
		done2 <- r.Run(context.Background(), func(ctx context.Context) error {
			return nil
		})
	}()

	// Second run should return quickly (nil because already running)
	select {
	case err := <-done2:
		assert.NoError(t, err)
	case <-time.After(50 * time.Millisecond):
		t.Fatal("Second run blocked unexpectedly")
	}

	// First run completes
	err := <-done1
	assert.NoError(t, err)
}

func TestReconnector_ExponentialBackoff(t *testing.T) {
	cfg := ReconnectorConfig{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         100 * time.Millisecond,
		Multiplier:          2.0,
		RandomizationFactor: 0, // No jitter for predictable test
		MaxRetries:          4,
	}
	r := NewReconnector(cfg, zerolog.Nop())

	var delays []time.Duration
	r.SetOnRetry(func(attempt int, delay time.Duration, err error) {
		delays = append(delays, delay)
	})

	r.Run(context.Background(), func(ctx context.Context) error {
		return errors.New("fail")
	})

	// Should have 4 delays recorded (4 retries)
	assert.Len(t, delays, 4)

	// Verify exponential increase (with some tolerance for timing)
	// 10ms -> 20ms -> 40ms -> 80ms (capped at 100ms)
	assert.True(t, delays[0] >= 10*time.Millisecond)
	assert.True(t, delays[1] >= 20*time.Millisecond)
	assert.True(t, delays[2] >= 40*time.Millisecond)
	assert.True(t, delays[3] <= 100*time.Millisecond) // Capped
}
