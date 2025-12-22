// Package resilienceadapter provides resilience decorators for message sending.
package resilienceadapter

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
)

// RetrySender wraps a MessageSink with retry buffer functionality.
// Implements the Decorator pattern (OCP - Open/Closed Principle).
type RetrySender struct {
	inner     ports.MessageSink
	buffer    []domain.Message
	maxSize   int
	mu        sync.Mutex
	flushChan chan struct{}
	done      chan struct{}
	closed    atomic.Bool
	logger    zerolog.Logger
}

// RetrySenderConfig holds configuration for the retry sender.
type RetrySenderConfig struct {
	MaxSize       int
	FlushInterval time.Duration
}

// DefaultRetrySenderConfig returns sensible defaults.
func DefaultRetrySenderConfig() RetrySenderConfig {
	return RetrySenderConfig{
		MaxSize:       1000,
		FlushInterval: 10 * time.Second,
	}
}

// NewRetrySender creates a new retry sender decorator.
func NewRetrySender(inner ports.MessageSink, cfg RetrySenderConfig, logger zerolog.Logger) *RetrySender {
	return &RetrySender{
		inner:     inner,
		buffer:    make([]domain.Message, 0, cfg.MaxSize),
		maxSize:   cfg.MaxSize,
		flushChan: make(chan struct{}, 1),
		done:      make(chan struct{}),
		logger:    logger.With().Str("component", "retry_sender").Logger(),
	}
}

// Send attempts to send a message, buffering on failure.
func (r *RetrySender) Send(ctx context.Context, msg domain.Message) error {
	if err := r.inner.Send(ctx, msg); err != nil {
		r.addToBuffer(msg)
		return err
	}
	return nil
}

// Close releases resources.
func (r *RetrySender) Close() error {
	if r.closed.Swap(true) {
		return nil // Already closed
	}
	close(r.done)
	return r.inner.Close()
}

// Start begins the background flushing worker.
func (r *RetrySender) Start(ctx context.Context, cfg RetrySenderConfig) {
	go func() {
		ticker := time.NewTicker(cfg.FlushInterval)
		defer ticker.Stop()

		for {
			select {
			case <-r.done:
				return
			case <-ctx.Done():
				return
			case <-r.flushChan:
				r.flush(ctx)
			case <-ticker.C:
				r.flush(ctx)
			}
		}
	}()
}

func (r *RetrySender) addToBuffer(msg domain.Message) bool {
	r.mu.Lock()
	defer r.mu.Unlock()

	if len(r.buffer) >= r.maxSize {
		return false
	}

	r.buffer = append(r.buffer, msg)

	// Signal worker to try flushing
	select {
	case r.flushChan <- struct{}{}:
	default:
	}

	return true
}

func (r *RetrySender) flush(ctx context.Context) {
	r.mu.Lock()
	if len(r.buffer) == 0 {
		r.mu.Unlock()
		return
	}

	// Copy messages to avoid holding lock during network calls
	msgs := make([]domain.Message, len(r.buffer))
	copy(msgs, r.buffer)
	r.buffer = r.buffer[:0]
	r.mu.Unlock()

	failed := make([]domain.Message, 0)

	for _, msg := range msgs {
		if err := r.inner.Send(ctx, msg); err != nil {
			failed = append(failed, msg)
		}
	}

	if len(failed) > 0 {
		r.mu.Lock()
		// Put failed ones back at the beginning (if space allows)
		remaining := r.maxSize - len(r.buffer)
		if remaining > 0 {
			toAdd := failed
			if len(toAdd) > remaining {
				toAdd = toAdd[:remaining]
			}
			r.buffer = append(toAdd, r.buffer...)
		}
		r.mu.Unlock()
	}
}

// Size returns the current buffer size.
func (r *RetrySender) Size() int {
	r.mu.Lock()
	defer r.mu.Unlock()
	return len(r.buffer)
}

// Ensure RetrySender implements MessageSink
var _ ports.MessageSink = (*RetrySender)(nil)
