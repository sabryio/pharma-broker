package queue

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"

	"pharmabroker/pkg/metrics"
)

type QueueHealthStatus string

const (
	QueueHealthStatusHealthy      QueueHealthStatus = "HEALTHY"
	QueueHealthStatusWarning      QueueHealthStatus = "WARNING"
	QueueHealthStatusDegraded     QueueHealthStatus = "DEGRADED"
	QueueHealthStatusStopped      QueueHealthStatus = "STOPPED"
	QueueHealthStatusUnhealthy    QueueHealthStatus = "UNHEALTHY"
	QueueHealthStatusDisconnected QueueHealthStatus = "DISCONNECTED"
)

// QueueConfig holds configuration for the message queue.
type QueueConfig struct {
	// BufferSize is the main queue capacity.
	BufferSize int
	// DeadLetterSize is the overflow queue capacity.
	DeadLetterSize int
	// WorkerCount is the number of concurrent workers.
	WorkerCount int
	// ProcessTimeout is the max time for processing a single message.
	ProcessTimeout time.Duration
}

// DefaultQueueConfig returns sensible default configuration.
func DefaultQueueConfig() QueueConfig {
	return QueueConfig{
		BufferSize:     1000,
		DeadLetterSize: 500,
		WorkerCount:    3,
		ProcessTimeout: 30 * time.Second,
	}
}

// Identifiable is the constraint required by Queue.
// Any message type must implement these accessors.
type Identifiable interface {
	GetID() string
}

// MessageHandler processes a message of any type T.
type MessageHandler[T Identifiable] func(ctx context.Context, msg T) error

// Queue is a production-ready message queue with overflow handling and metrics.
type Queue[T Identifiable] struct {
	cfg    QueueConfig
	log    zerolog.Logger
	mu     sync.RWMutex
	wg     sync.WaitGroup
	closed atomic.Bool

	// Main message channel
	messages chan T
	// Dead letter queue for overflow
	deadLetter chan T
	// Stop signal
	done chan struct{}

	// Handler for processing messages
	handler MessageHandler[T]

	// Metrics
	received  atomic.Int64
	processed atomic.Int64
	overflow  atomic.Int64
	dropped   atomic.Int64
	inFlight  atomic.Int32
	dlqSize   atomic.Int32
	startTime time.Time
}

// NewQueue creates a new message queue with the given configuration.
func NewQueue[T Identifiable](cfg QueueConfig, log zerolog.Logger) *Queue[T] {
	if cfg.BufferSize <= 0 {
		cfg.BufferSize = 1000
	}
	if cfg.DeadLetterSize <= 0 {
		cfg.DeadLetterSize = 500
	}
	if cfg.WorkerCount <= 0 {
		cfg.WorkerCount = 3
	}
	if cfg.ProcessTimeout <= 0 {
		cfg.ProcessTimeout = 30 * time.Second
	}

	return &Queue[T]{
		cfg:        cfg,
		log:        log.With().Str("component", "message-queue").Logger(),
		messages:   make(chan T, cfg.BufferSize),
		deadLetter: make(chan T, cfg.DeadLetterSize),
		done:       make(chan struct{}),
		startTime:  time.Now(),
	}
}

// SetHandler sets the message processing handler.
func (q *Queue[T]) SetHandler(handler MessageHandler[T]) {
	q.mu.Lock()
	defer q.mu.Unlock()
	q.handler = handler
}

// Start begins the worker pool for processing messages.
func (q *Queue[T]) Start() {
	q.log.Info().
		Int("workers", q.cfg.WorkerCount).
		Int("buffer_size", q.cfg.BufferSize).
		Int("dlq_size", q.cfg.DeadLetterSize).
		Msg("Starting message queue workers")

	for i := 0; i < q.cfg.WorkerCount; i++ {
		q.wg.Add(1)
		go q.worker(i)
	}

	// Start DLQ processor (single worker for retry)
	q.wg.Add(1)
	go q.dlqWorker()

	// Start metrics updater
	q.wg.Add(1)
	go q.metricsUpdater()
}

// Stop gracefully shuts down the queue.
func (q *Queue[T]) Stop(ctx context.Context) error {
	if q.closed.Swap(true) {
		return nil // Already closed
	}

	q.log.Info().Msg("Stopping message queue...")
	close(q.done)

	// Wait for workers with timeout
	done := make(chan struct{})
	go func() {
		q.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		q.log.Info().
			Int64("processed", q.processed.Load()).
			Int64("overflow", q.overflow.Load()).
			Int64("dropped", q.dropped.Load()).
			Msg("Message queue stopped gracefully")
		return nil
	case <-ctx.Done():
		q.log.Warn().Msg("Message queue stop timed out")
		return ctx.Err()
	}
}

// Enqueue adds a message to the queue.
// Returns true if enqueued successfully, false if dropped.
func (q *Queue[T]) Enqueue(msg *T) bool {
	if q.closed.Load() {
		q.dropped.Add(1)
		metrics.MessagesDropped.Inc()
		return false
	}

	q.received.Add(1)
	metrics.MessagesReceived.Inc()

	// Try main queue first (non-blocking)
	select {
	case q.messages <- *msg:
		q.log.Debug().
			Str("msg_id", (*msg).GetID()).
			Int("queue_size", len(q.messages)).
			Msg("Message enqueued")
		return true
	default:
		// Main queue full, try dead letter
		return q.enqueueDeadLetter(msg)
	}
}

// enqueueDeadLetter adds a message to the dead letter queue.
func (q *Queue[T]) enqueueDeadLetter(msg *T) bool {
	q.overflow.Add(1)
	metrics.MessagesOverflow.Inc()

	select {
	case q.deadLetter <- *msg:
		q.dlqSize.Add(1)
		q.log.Warn().
			Str("msg_id", (*msg).GetID()).
			Int("dlq_size", len(q.deadLetter)).
			Msg("Message moved to dead letter queue")
		return true
	default:
		// Both queues full, message dropped
		q.dropped.Add(1)
		metrics.MessagesDropped.Inc()
		q.log.Error().
			Str("msg_id", (*msg).GetID()).
			Msg("Message dropped - all queues full")
		return false
	}
}

// worker processes messages from the main queue.
func (q *Queue[T]) worker(id int) {
	defer q.wg.Done()

	log := q.log.With().Int("worker_id", id).Logger()
	log.Debug().Msg("Worker started")

	for {
		select {
		case <-q.done:
			log.Debug().Msg("Worker stopping")
			return
		case msg, ok := <-q.messages:
			if !ok {
				return
			}
			q.processMessage(log, &msg)
		}
	}
}

// dlqWorker processes messages from the dead letter queue.
func (q *Queue[T]) dlqWorker() {
	defer q.wg.Done()

	log := q.log.With().Str("worker", "dlq").Logger()
	log.Debug().Msg("DLQ worker started")

	// Process DLQ with rate limiting (1 per second)
	ticker := time.NewTicker(time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-q.done:
			log.Debug().Msg("DLQ worker stopping")
			return
		case <-ticker.C:
			select {
			case msg, ok := <-q.deadLetter:
				if !ok {
					return
				}
				q.dlqSize.Add(-1)
				q.processMessage(log, &msg)
			default:
				// No messages in DLQ
			}
		}
	}
}

// processMessage handles a single message.
func (q *Queue[T]) processMessage(log zerolog.Logger, msg *T) {
	q.mu.RLock()
	handler := q.handler
	q.mu.RUnlock()

	if handler == nil {
		log.Warn().Str("msg_id", (*msg).GetID()).Msg("No handler set, skipping message")
		return
	}

	q.inFlight.Add(1)
	metrics.MessageQueueInFlight.Inc()
	defer func() {
		q.inFlight.Add(-1)
		metrics.MessageQueueInFlight.Dec()
	}()

	ctx, cancel := context.WithTimeout(context.Background(), q.cfg.ProcessTimeout)
	defer cancel()

	start := time.Now()
	err := handler(ctx, *msg)
	duration := time.Since(start)

	if err != nil {
		log.Error().
			Err(err).
			Str("msg_id", (*msg).GetID()).
			Dur("duration", duration).
			Msg("Failed to process message")
		metrics.MessagesProcessedStatus.WithLabelValues("error").Inc()
	} else {
		q.processed.Add(1)
		log.Debug().
			Str("msg_id", (*msg).GetID()).
			Dur("duration", duration).
			Msg("Message processed successfully")
		metrics.MessagesProcessedStatus.WithLabelValues("success").Inc()
	}

	metrics.MessageProcessingLatency.Observe(duration.Seconds())
}

// metricsUpdater periodically updates gauge metrics.
func (q *Queue[T]) metricsUpdater() {
	defer q.wg.Done()

	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-q.done:
			return
		case <-ticker.C:
			metrics.MessageQueueSize.Set(float64(len(q.messages)))
			metrics.MessageQueueDLQSize.Set(float64(len(q.deadLetter)))
			metrics.MessageQueueWorkers.Set(float64(q.cfg.WorkerCount))
		}
	}
}

// Stats returns current queue statistics.
func (q *Queue[T]) Stats() QueueStats {
	return QueueStats{
		Received:    q.received.Load(),
		Processed:   q.processed.Load(),
		Overflow:    q.overflow.Load(),
		Dropped:     q.dropped.Load(),
		InFlight:    int(q.inFlight.Load()),
		QueueSize:   len(q.messages),
		DLQSize:     len(q.deadLetter),
		WorkerCount: q.cfg.WorkerCount,
		Uptime:      time.Since(q.startTime),
	}
}

// QueueStats holds queue statistics.
type QueueStats struct {
	Received    int64         `json:"received"`
	Processed   int64         `json:"processed"`
	Overflow    int64         `json:"overflow"`
	Dropped     int64         `json:"dropped"`
	InFlight    int           `json:"in_flight"`
	QueueSize   int           `json:"queue_size"`
	DLQSize     int           `json:"dlq_size"`
	WorkerCount int           `json:"worker_count"`
	Uptime      time.Duration `json:"uptime"`
}

// HealthStatus returns health information for the queue.
func (q *Queue[T]) HealthStatus() QueueHealth {
	stats := q.Stats()

	status := QueueHealthStatusHealthy
	if q.closed.Load() {
		status = QueueHealthStatusStopped
	} else if stats.DLQSize > q.cfg.DeadLetterSize/2 {
		status = QueueHealthStatusDegraded
	} else if stats.QueueSize > q.cfg.BufferSize*80/100 {
		status = QueueHealthStatusWarning
	}

	return QueueHealth{
		Status:       status,
		QueueUsage:   float64(stats.QueueSize) / float64(q.cfg.BufferSize) * 100,
		DLQUsage:     float64(stats.DLQSize) / float64(q.cfg.DeadLetterSize) * 100,
		ProcessedPct: q.calculateProcessedPct(stats),
		Stats:        stats,
	}
}

func (q *Queue[T]) calculateProcessedPct(stats QueueStats) float64 {
	if stats.Received == 0 {
		return 100.0
	}
	return float64(stats.Processed) / float64(stats.Received) * 100
}

// QueueHealth represents queue health status.
type QueueHealth struct {
	Status       QueueHealthStatus `json:"status"`
	QueueUsage   float64           `json:"queue_usage_pct"`
	DLQUsage     float64           `json:"dlq_usage_pct"`
	ProcessedPct float64           `json:"processed_pct"`
	Stats        QueueStats        `json:"stats"`
}

// Size returns the current number of messages in the main queue.
func (q *Queue[T]) Size() int {
	return len(q.messages)
}

// DLQSize returns the current number of messages in the dead letter queue.
func (q *Queue[T]) DLQSize() int {
	return len(q.deadLetter)
}

// IsClosed returns true if the queue has been stopped.
func (q *Queue[T]) IsClosed() bool {
	return q.closed.Load()
}
