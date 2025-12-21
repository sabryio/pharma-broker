package reconnector

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func TestNew(t *testing.T) {
	logger := zerolog.Nop()
	cfg := DefaultConfig()

	r := New(cfg, logger)
	if r == nil {
		t.Fatal("New returned nil")
	}

	if r.IsRunning() {
		t.Error("Should not be running initially")
	}

	if r.Attempts() != 0 {
		t.Error("Should have 0 attempts initially")
	}
}

func TestReconnector_SuccessOnFirstAttempt(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         100 * time.Millisecond,
		Multiplier:          2.0,
		RandomizationFactor: 0,
		MaxRetries:          5,
	}

	r := New(cfg, logger)

	var successCalled atomic.Bool
	r.SetOnSuccess(func(attempt int, elapsed time.Duration) {
		successCalled.Store(true)
		if attempt != 1 {
			t.Errorf("Expected 1 attempt, got %d", attempt)
		}
	})

	ctx := context.Background()
	err := r.Run(ctx, func(ctx context.Context) error {
		return nil // Success immediately
	})

	if err != nil {
		t.Errorf("Expected no error, got %v", err)
	}

	if !successCalled.Load() {
		t.Error("OnSuccess callback should have been called")
	}

	if r.Attempts() != 1 {
		t.Errorf("Expected 1 attempt, got %d", r.Attempts())
	}
}

func TestReconnector_SuccessAfterRetries(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         50 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          10,
	}

	r := New(cfg, logger)

	var attempts atomic.Int32
	var retryCalled atomic.Int32

	r.SetOnRetry(func(attempt int, delay time.Duration, err error) {
		retryCalled.Add(1)
	})

	r.SetOnSuccess(func(attempt int, elapsed time.Duration) {
		if attempt != 3 {
			t.Errorf("Expected 3 attempts, got %d", attempt)
		}
	})

	ctx := context.Background()
	err := r.Run(ctx, func(ctx context.Context) error {
		count := attempts.Add(1)
		if count < 3 {
			return errors.New("not ready yet")
		}
		return nil // Success on 3rd attempt
	})

	if err != nil {
		t.Errorf("Expected no error, got %v", err)
	}

	if retryCalled.Load() != 2 {
		t.Errorf("Expected 2 retry callbacks, got %d", retryCalled.Load())
	}
}

func TestReconnector_MaxRetriesExceeded(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     5 * time.Millisecond,
		MaxInterval:         10 * time.Millisecond,
		Multiplier:          1.1,
		RandomizationFactor: 0,
		MaxRetries:          3,
	}

	r := New(cfg, logger)

	var failureCalled atomic.Bool
	r.SetOnFailure(func(attempt int, elapsed time.Duration, err error) {
		failureCalled.Store(true)
	})

	ctx := context.Background()
	err := r.Run(ctx, func(ctx context.Context) error {
		return errors.New("always fail")
	})

	if err == nil {
		t.Error("Expected error when max retries exceeded")
	}

	if !failureCalled.Load() {
		t.Error("OnFailure callback should have been called")
	}
}

func TestReconnector_ContextCancellation(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     100 * time.Millisecond,
		MaxInterval:         1 * time.Second,
		Multiplier:          2.0,
		RandomizationFactor: 0,
		MaxRetries:          0, // Infinite
	}

	r := New(cfg, logger)

	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()

	err := r.Run(ctx, func(ctx context.Context) error {
		return errors.New("always fail")
	})

	if err == nil {
		t.Error("Expected error when context cancelled")
	}
}

func TestReconnector_Stop(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     100 * time.Millisecond,
		MaxInterval:         1 * time.Second,
		Multiplier:          2.0,
		RandomizationFactor: 0,
		MaxRetries:          0, // Infinite
	}

	r := New(cfg, logger)

	// Start in goroutine
	done := make(chan error, 1)
	go func() {
		done <- r.Run(context.Background(), func(ctx context.Context) error {
			return errors.New("always fail")
		})
	}()

	// Wait a bit then stop
	time.Sleep(50 * time.Millisecond)
	r.Stop()

	select {
	case err := <-done:
		if err == nil {
			t.Error("Expected error when stopped")
		}
	case <-time.After(500 * time.Millisecond):
		t.Error("Run should have returned after Stop")
	}
}

func TestReconnector_IsRunning(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     50 * time.Millisecond,
		MaxInterval:         100 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          0,
	}

	r := New(cfg, logger)

	if r.IsRunning() {
		t.Error("Should not be running initially")
	}

	started := make(chan struct{})
	done := make(chan struct{})

	go func() {
		close(started)
		r.Run(context.Background(), func(ctx context.Context) error {
			time.Sleep(100 * time.Millisecond)
			return nil
		})
		close(done)
	}()

	<-started
	time.Sleep(10 * time.Millisecond)

	if !r.IsRunning() {
		t.Error("Should be running during Run")
	}

	<-done

	if r.IsRunning() {
		t.Error("Should not be running after completion")
	}
}

func TestReconnector_Reset(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         50 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          10,
	}

	r := New(cfg, logger)

	// Run with some failures
	var attempts atomic.Int32
	r.Run(context.Background(), func(ctx context.Context) error {
		if attempts.Add(1) < 3 {
			return errors.New("fail")
		}
		return nil
	})

	if r.Attempts() != 3 {
		t.Errorf("Expected 3 attempts, got %d", r.Attempts())
	}

	// Reset
	r.Reset()

	if r.Attempts() != 0 {
		t.Errorf("Expected 0 attempts after reset, got %d", r.Attempts())
	}
}

func TestReconnector_ConcurrentRun(t *testing.T) {
	logger := zerolog.Nop()
	cfg := Config{
		InitialInterval:     10 * time.Millisecond,
		MaxInterval:         50 * time.Millisecond,
		Multiplier:          1.5,
		RandomizationFactor: 0,
		MaxRetries:          5,
	}

	r := New(cfg, logger)

	// Start first run
	done1 := make(chan error, 1)
	go func() {
		done1 <- r.Run(context.Background(), func(ctx context.Context) error {
			time.Sleep(100 * time.Millisecond)
			return nil
		})
	}()

	time.Sleep(10 * time.Millisecond)

	// Second run should return immediately (already running)
	done2 := make(chan error, 1)
	go func() {
		done2 <- r.Run(context.Background(), func(ctx context.Context) error {
			return nil
		})
	}()

	// Second should complete quickly
	select {
	case err := <-done2:
		if err != nil {
			t.Errorf("Second run should return nil, got %v", err)
		}
	case <-time.After(50 * time.Millisecond):
		t.Error("Second run should return immediately")
	}

	// Wait for first to complete
	<-done1
}

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()

	if cfg.InitialInterval != 5*time.Second {
		t.Errorf("Expected initial interval 5s, got %v", cfg.InitialInterval)
	}
	if cfg.MaxInterval != 5*time.Minute {
		t.Errorf("Expected max interval 5m, got %v", cfg.MaxInterval)
	}
	if cfg.Multiplier != 2.0 {
		t.Errorf("Expected multiplier 2.0, got %v", cfg.Multiplier)
	}
	if cfg.RandomizationFactor != 0.1 {
		t.Errorf("Expected randomization factor 0.1, got %v", cfg.RandomizationFactor)
	}
	if cfg.MaxElapsedTime != 0 {
		t.Errorf("Expected max elapsed time 0, got %v", cfg.MaxElapsedTime)
	}
	if cfg.MaxRetries != 0 {
		t.Errorf("Expected max retries 0, got %v", cfg.MaxRetries)
	}
}
