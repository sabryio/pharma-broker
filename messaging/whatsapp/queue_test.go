package whatsapp

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/rs/zerolog"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"pharmabroker/domain/entity"
)

// testLogger creates a silent logger for tests.
func testLogger() zerolog.Logger {
	return zerolog.Nop()
}

// newTestMessage creates a test raw message with unique ID.
func newTestMessage(id string) *entity.RawMessage {
	return &entity.RawMessage{
		ID:        id,
		GroupJID:  "group@s.whatsapp.net",
		GroupName: "Test Group",
		SenderJID: "sender@s.whatsapp.net",
		Content:   "Test message content",
		Timestamp: time.Now(),
	}
}

func TestNewQueue(t *testing.T) {
	tests := []struct {
		name     string
		cfg      QueueConfig
		wantBuf  int
		wantDLQ  int
		wantWork int
	}{
		{
			name:     "default config",
			cfg:      DefaultQueueConfig(),
			wantBuf:  1000,
			wantDLQ:  500,
			wantWork: 3,
		},
		{
			name:     "custom config",
			cfg:      QueueConfig{BufferSize: 100, DeadLetterSize: 50, WorkerCount: 5},
			wantBuf:  100,
			wantDLQ:  50,
			wantWork: 5,
		},
		{
			name:     "zero values use defaults",
			cfg:      QueueConfig{},
			wantBuf:  1000,
			wantDLQ:  500,
			wantWork: 3,
		},
		{
			name:     "negative values use defaults",
			cfg:      QueueConfig{BufferSize: -10, DeadLetterSize: -5, WorkerCount: -2},
			wantBuf:  1000,
			wantDLQ:  500,
			wantWork: 3,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			q := NewQueue(tt.cfg, testLogger())
			require.NotNil(t, q)
			assert.Equal(t, tt.wantBuf, cap(q.messages))
			assert.Equal(t, tt.wantDLQ, cap(q.deadLetter))
			assert.Equal(t, tt.wantWork, q.cfg.WorkerCount)
			assert.False(t, q.IsClosed())
		})
	}
}

func TestQueue_EnqueueBasic(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	msg := newTestMessage("msg-1")
	ok := q.Enqueue(msg)

	assert.True(t, ok)
	assert.Equal(t, 1, q.Size())
	assert.Equal(t, int64(1), q.Stats().Received)
}

func TestQueue_EnqueueMultiple(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	for i := 0; i < 5; i++ {
		msg := newTestMessage("msg-" + string(rune('a'+i)))
		ok := q.Enqueue(msg)
		assert.True(t, ok)
	}

	assert.Equal(t, 5, q.Size())
	assert.Equal(t, int64(5), q.Stats().Received)
	assert.Equal(t, int64(0), q.Stats().Overflow)
}

func TestQueue_EnqueueOverflowToDeadLetter(t *testing.T) {
	cfg := QueueConfig{BufferSize: 3, DeadLetterSize: 2, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	// Fill main queue
	for i := 0; i < 3; i++ {
		msg := newTestMessage("main-" + string(rune('a'+i)))
		ok := q.Enqueue(msg)
		assert.True(t, ok)
	}

	assert.Equal(t, 3, q.Size())
	assert.Equal(t, 0, q.DLQSize())

	// Next messages should go to DLQ
	for i := 0; i < 2; i++ {
		msg := newTestMessage("dlq-" + string(rune('a'+i)))
		ok := q.Enqueue(msg)
		assert.True(t, ok)
	}

	assert.Equal(t, 3, q.Size())
	assert.Equal(t, 2, q.DLQSize())
	assert.Equal(t, int64(5), q.Stats().Received)
	assert.Equal(t, int64(2), q.Stats().Overflow)
}

func TestQueue_EnqueueDroppedWhenFull(t *testing.T) {
	cfg := QueueConfig{BufferSize: 2, DeadLetterSize: 1, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	// Fill both queues
	for i := 0; i < 3; i++ {
		q.Enqueue(newTestMessage("fill-" + string(rune('a'+i))))
	}

	assert.Equal(t, 2, q.Size())
	assert.Equal(t, 1, q.DLQSize())

	// Next message should be dropped
	msg := newTestMessage("dropped")
	ok := q.Enqueue(msg)

	assert.False(t, ok)
	assert.Equal(t, int64(1), q.Stats().Dropped)
}

func TestQueue_EnqueueAfterClose(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())
	q.Start()

	// Close queue
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	require.NoError(t, q.Stop(ctx))

	// Try to enqueue after close
	msg := newTestMessage("after-close")
	ok := q.Enqueue(msg)

	assert.False(t, ok)
	assert.True(t, q.IsClosed())
	assert.Equal(t, int64(1), q.Stats().Dropped)
}

func TestQueue_StartAndStop(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 2}
	q := NewQueue(cfg, testLogger())

	assert.False(t, q.IsClosed())

	q.Start()
	// Give workers time to start
	time.Sleep(50 * time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	err := q.Stop(ctx)

	assert.NoError(t, err)
	assert.True(t, q.IsClosed())
}

func TestQueue_StopIdempotent(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())
	q.Start()

	ctx := context.Background()

	// Stop multiple times
	err1 := q.Stop(ctx)
	err2 := q.Stop(ctx)
	err3 := q.Stop(ctx)

	assert.NoError(t, err1)
	assert.NoError(t, err2)
	assert.NoError(t, err3)
}

func TestQueue_ProcessMessages(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 2, ProcessTimeout: time.Second}
	q := NewQueue(cfg, testLogger())

	var processed atomic.Int32
	q.SetHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		processed.Add(1)
		return nil
	})

	q.Start()

	// Enqueue messages
	for i := 0; i < 5; i++ {
		q.Enqueue(newTestMessage("msg-" + string(rune('a'+i))))
	}

	// Wait for processing
	time.Sleep(500 * time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	assert.Equal(t, int32(5), processed.Load())
	assert.Equal(t, int64(5), q.Stats().Processed)
}

func TestQueue_ProcessMessagesFromDLQ(t *testing.T) {
	cfg := QueueConfig{BufferSize: 2, DeadLetterSize: 3, WorkerCount: 2, ProcessTimeout: time.Second}
	q := NewQueue(cfg, testLogger())

	var processed atomic.Int32
	q.SetHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		processed.Add(1)
		return nil
	})

	// Enqueue to fill main queue BEFORE starting (so messages sit in queue)
	q.Enqueue(newTestMessage("main-a"))
	q.Enqueue(newTestMessage("main-b"))
	// These go to DLQ
	q.Enqueue(newTestMessage("dlq-a"))
	q.Enqueue(newTestMessage("dlq-b"))

	assert.Equal(t, 2, q.Size())
	assert.Equal(t, 2, q.DLQSize())
	assert.Equal(t, int64(2), q.Stats().Overflow)

	// Now start processing
	q.Start()

	// Wait for main queue + DLQ processing (DLQ has 1/sec rate limit)
	time.Sleep(3 * time.Second)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	// All messages should be processed
	assert.Equal(t, int32(4), processed.Load())
}

func TestQueue_HandlerError(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1, ProcessTimeout: time.Second}
	q := NewQueue(cfg, testLogger())

	var called atomic.Int32
	q.SetHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		called.Add(1)
		return errors.New("processing error")
	})

	q.Start()
	q.Enqueue(newTestMessage("error-msg"))

	// Wait for processing attempt
	time.Sleep(200 * time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	// Handler was called but message not counted as processed
	assert.Equal(t, int32(1), called.Load())
	assert.Equal(t, int64(0), q.Stats().Processed)
}

func TestQueue_NoHandler(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	// Don't set handler
	q.Start()
	q.Enqueue(newTestMessage("no-handler"))

	time.Sleep(200 * time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	// Message received but not processed
	assert.Equal(t, int64(1), q.Stats().Received)
	assert.Equal(t, int64(0), q.Stats().Processed)
}

func TestQueue_ConcurrentEnqueue(t *testing.T) {
	cfg := QueueConfig{BufferSize: 100, DeadLetterSize: 50, WorkerCount: 5}
	q := NewQueue(cfg, testLogger())

	var processed atomic.Int32
	q.SetHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		processed.Add(1)
		time.Sleep(10 * time.Millisecond) // Simulate work
		return nil
	})

	q.Start()

	// Concurrent enqueues
	var wg sync.WaitGroup
	for i := 0; i < 50; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			q.Enqueue(newTestMessage("concurrent-" + string(rune('a'+idx%26))))
		}(i)
	}
	wg.Wait()

	// Wait for processing
	time.Sleep(2 * time.Second)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	assert.Equal(t, int64(50), q.Stats().Received)
	assert.Equal(t, int32(50), processed.Load())
}

func TestQueueStats(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 3}
	q := NewQueue(cfg, testLogger())

	q.Enqueue(newTestMessage("a"))
	q.Enqueue(newTestMessage("b"))

	stats := q.Stats()

	assert.Equal(t, int64(2), stats.Received)
	assert.Equal(t, int64(0), stats.Processed)
	assert.Equal(t, int64(0), stats.Overflow)
	assert.Equal(t, int64(0), stats.Dropped)
	assert.Equal(t, 2, stats.QueueSize)
	assert.Equal(t, 0, stats.DLQSize)
	assert.Equal(t, 3, stats.WorkerCount)
	assert.GreaterOrEqual(t, stats.Uptime, time.Duration(0))
}

func TestQueueHealth_Healthy(t *testing.T) {
	cfg := QueueConfig{BufferSize: 100, DeadLetterSize: 50, WorkerCount: 3}
	q := NewQueue(cfg, testLogger())

	q.Enqueue(newTestMessage("a"))

	health := q.HealthStatus()

	assert.Equal(t, "healthy", health.Status)
	assert.Equal(t, 1.0, health.QueueUsage)
	assert.Equal(t, 0.0, health.DLQUsage)
}

func TestQueueHealth_Warning(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 10, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	// Fill to 80%+
	for i := 0; i < 9; i++ {
		q.Enqueue(newTestMessage("warn-" + string(rune('a'+i))))
	}

	health := q.HealthStatus()

	assert.Equal(t, "warning", health.Status)
	assert.Equal(t, 90.0, health.QueueUsage)
}

func TestQueueHealth_Degraded(t *testing.T) {
	cfg := QueueConfig{BufferSize: 2, DeadLetterSize: 4, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())

	// Fill main queue
	q.Enqueue(newTestMessage("main-a"))
	q.Enqueue(newTestMessage("main-b"))
	// Fill DLQ > 50%
	q.Enqueue(newTestMessage("dlq-a"))
	q.Enqueue(newTestMessage("dlq-b"))
	q.Enqueue(newTestMessage("dlq-c"))

	health := q.HealthStatus()

	assert.Equal(t, "degraded", health.Status)
	assert.Equal(t, 75.0, health.DLQUsage)
}

func TestQueueHealth_Stopped(t *testing.T) {
	cfg := QueueConfig{BufferSize: 10, DeadLetterSize: 5, WorkerCount: 1}
	q := NewQueue(cfg, testLogger())
	q.Start()

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	health := q.HealthStatus()

	assert.Equal(t, "stopped", health.Status)
}

func TestQueue_ProcessTimeout(t *testing.T) {
	cfg := QueueConfig{
		BufferSize:     10,
		DeadLetterSize: 5,
		WorkerCount:    1,
		ProcessTimeout: 100 * time.Millisecond,
	}
	q := NewQueue(cfg, testLogger())

	var timedOut atomic.Bool
	q.SetHandler(func(ctx context.Context, msg *entity.RawMessage) error {
		select {
		case <-ctx.Done():
			timedOut.Store(true)
			return ctx.Err()
		case <-time.After(500 * time.Millisecond):
			return nil
		}
	})

	q.Start()
	q.Enqueue(newTestMessage("timeout-test"))

	time.Sleep(300 * time.Millisecond)

	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	q.Stop(ctx)

	assert.True(t, timedOut.Load())
}

func TestDefaultQueueConfig(t *testing.T) {
	cfg := DefaultQueueConfig()

	assert.Equal(t, 1000, cfg.BufferSize)
	assert.Equal(t, 500, cfg.DeadLetterSize)
	assert.Equal(t, 3, cfg.WorkerCount)
	assert.Equal(t, 30*time.Second, cfg.ProcessTimeout)
}
