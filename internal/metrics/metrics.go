package metrics

import (
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

var (
	// Counters
	MessagesProcessed = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_messages_processed_total",
		Help: "The total number of messages processed by the parser",
	})

	OffersCreated = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_offers_created_total",
		Help: "The total number of offers extracted and created",
	})

	RequestsCreated = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_requests_created_total",
		Help: "The total number of requests extracted and created",
	})

	MatchesFound = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_matches_found_total",
		Help: "The total number of matches found between offers and requests",
	})

	SystemErrors = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_system_errors_total",
		Help: "The total number of system errors (AI failures, DB errors)",
	})

	// Circuit Breaker Metrics
	CircuitBreakerState = promauto.NewGaugeVec(prometheus.GaugeOpts{
		Name: "pharma_circuit_breaker_state",
		Help: "Current state of circuit breakers (0=closed, 1=open, 2=half-open)",
	}, []string{"name"})

	CircuitBreakerFailures = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_circuit_breaker_failures_total",
		Help: "Total number of failures recorded by circuit breakers",
	}, []string{"name"})

	// Match Queue Metrics
	MatchQueueDepth = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_match_queue_depth",
		Help: "Current number of pending match jobs in the queue",
	})

	MatchJobsProcessed = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_match_jobs_processed_total",
		Help: "Total number of match jobs processed",
	})

	MatchProcessingDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_processing_duration_seconds",
		Help:    "Duration of match job processing",
		Buckets: prometheus.DefBuckets,
	})

	// Goroutine Metrics
	ActiveWorkers = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_active_workers",
		Help: "Number of active worker goroutines",
	})

	// Histograms
	AIRequestDuration = promauto.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "pharma_ai_request_duration_seconds",
		Help:    "Duration of AI provider requests",
		Buckets: prometheus.DefBuckets,
	}, []string{"status"})

	AITokensUsed = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_ai_tokens_used",
		Help:    "Number of tokens used per AI request (estimated)",
		Buckets: []float64{100, 500, 1000, 2000, 5000, 10000, 20000},
	})

	MessageProcessingDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_message_processing_duration_seconds",
		Help:    "Time taken to process a batch of messages",
		Buckets: prometheus.DefBuckets,
	})
)
