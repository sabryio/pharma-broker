package resilience

import (
	"context"
	"sync"
	"time"

	pb "pharma-bridge/proto"
)

// RetryBuffer holds messages that failed to forward due to transient errors.
type RetryBuffer struct {
	mu        sync.Mutex
	messages  []*pb.RawMessage
	maxSize   int
	onFlush   func(context.Context, *pb.RawMessage) error
	flushChan chan struct{}
	done      chan struct{}
}

// NewRetryBuffer creates a new retry buffer with a maximum size.
func NewRetryBuffer(maxSize int, onFlush func(context.Context, *pb.RawMessage) error) *RetryBuffer {
	return &RetryBuffer{
		messages:  make([]*pb.RawMessage, 0, maxSize),
		maxSize:   maxSize,
		onFlush:   onFlush,
		flushChan: make(chan struct{}, 1),
		done:      make(chan struct{}),
	}
}

// Add appends a message to the buffer if there is space.
func (b *RetryBuffer) Add(msg *pb.RawMessage) bool {
	b.mu.Lock()
	defer b.mu.Unlock()

	if len(b.messages) >= b.maxSize {
		return false
	}

	b.messages = append(b.messages, msg)

	// Signal worker to try flushing
	select {
	case b.flushChan <- struct{}{}:
	default:
	}

	return true
}

// Start begins the background flushing worker.
func (b *RetryBuffer) Start(ctx context.Context) {
	go func() {
		ticker := time.NewTicker(10 * time.Second)
		defer ticker.Stop()

		for {
			select {
			case <-b.done:
				return
			case <-ctx.Done():
				return
			case <-b.flushChan:
				b.flush(ctx)
			case <-ticker.C:
				b.flush(ctx)
			}
		}
	}()
}

// Stop stops the background flushing worker.
func (b *RetryBuffer) Stop() {
	close(b.done)
}

// Flush signals the worker to try flushing messages immediately.
func (b *RetryBuffer) Flush() {
	select {
	case b.flushChan <- struct{}{}:
	default:
	}
}

func (b *RetryBuffer) flush(ctx context.Context) {
	b.mu.Lock()
	if len(b.messages) == 0 {
		b.mu.Unlock()
		return
	}

	// Copy messages to avoid holding lock during network calls
	msgs := make([]*pb.RawMessage, len(b.messages))
	copy(msgs, b.messages)
	b.messages = b.messages[:0]
	b.mu.Unlock()

	failed := make([]*pb.RawMessage, 0)

	for _, msg := range msgs {
		if err := b.onFlush(ctx, msg); err != nil {
			failed = append(failed, msg)
		}
	}

	if len(failed) > 0 {
		b.mu.Lock()
		// Put failed ones back at the beginning (if space allows)
		remaining := b.maxSize - len(b.messages)
		if remaining > 0 {
			toAdd := failed
			if len(toAdd) > remaining {
				toAdd = toAdd[:remaining]
			}
			b.messages = append(toAdd, b.messages...)
		}
		b.mu.Unlock()
	}
}

// Size returns the current number of messages in the buffer.
func (b *RetryBuffer) Size() int {
	b.mu.Lock()
	defer b.mu.Unlock()
	return len(b.messages)
}
