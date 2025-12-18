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

	// Match Escalation Metrics
	MatchesEscalated = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_matches_escalated_total",
		Help: "Total number of matches escalated due to age",
	})

	// ========== Match Scoring Metrics ==========

	// Score distribution histogram - tracks overall match score distribution
	MatchScoreDistribution = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score",
		Help:    "Distribution of match scores (0.0 to 1.0)",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
	})

	// Scores by confidence band - count of matches per band
	MatchesByConfidenceBand = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_matches_by_confidence_total",
		Help: "Total matches categorized by confidence band",
	}, []string{"band"}) // band: AUTO, SUGGEST, REVIEW, NONE

	// Component score breakdown histograms
	MatchScoreMedication = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score_medication",
		Help:    "Distribution of medication matching scores",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
	})

	MatchScoreDosage = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score_dosage",
		Help:    "Distribution of dosage matching scores",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
	})

	MatchScoreQuantity = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score_quantity",
		Help:    "Distribution of quantity matching scores",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
	})

	MatchScorePrice = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score_price",
		Help:    "Distribution of price matching scores",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
	})

	MatchScoreRecency = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_match_score_recency",
		Help:    "Distribution of recency scores",
		Buckets: []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0},
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

	// ========== Adaptive Learning Metrics ==========

	// Learning Job Metrics
	LearningJobsTotal = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_learning_jobs_total",
		Help: "Total number of learning jobs executed",
	}, []string{"status"}) // status: success, failed, skipped, recommended

	LearningJobDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_learning_job_duration_seconds",
		Help:    "Duration of learning job execution",
		Buckets: []float64{1, 5, 10, 30, 60, 120, 300},
	})

	// Weight Application Metrics
	WeightsAppliedTotal = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_weights_applied_total",
		Help: "Total number of weight updates applied",
	}, []string{"source"}) // source: auto_learned, manual, rollback

	// Feedback Analysis Metrics
	FeedbackSamplesAnalyzed = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_feedback_samples_analyzed",
		Help: "Number of feedback samples in last learning run",
	})

	ConfirmationRate = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_confirmation_rate",
		Help: "Current match confirmation rate (0-1)",
	})

	ScoreSeparation = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_score_separation",
		Help: "Difference between avg confirmed and rejected scores",
	})

	// Current Weight Gauges (for monitoring weight evolution)
	CurrentWeights = promauto.NewGaugeVec(prometheus.GaugeOpts{
		Name: "pharma_current_weight",
		Help: "Current scoring weight value",
	}, []string{"factor"}) // factor: medication, dosage, quantity, price, recency

	// Pending Weights Indicator
	PendingWeightsAvailable = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_pending_weights_available",
		Help: "1 if there are pending weights awaiting approval, 0 otherwise",
	})

	// Learning Scheduler State
	LearningSchedulerEnabled = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_learning_scheduler_enabled",
		Help: "1 if adaptive learning scheduler is enabled, 0 otherwise",
	})

	LastLearningJobTimestamp = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_last_learning_job_timestamp",
		Help: "Unix timestamp of last learning job execution",
	})

	// ========== Cronjob Scheduler Metrics ==========

	CronJobSuccess = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_cronjob_success_total",
		Help: "Total number of successful cronjob executions",
	}, []string{"job"})

	CronJobFailed = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_cronjob_failed_total",
		Help: "Total number of failed cronjob executions",
	}, []string{"job"})

	CronJobDuration = promauto.NewHistogramVec(prometheus.HistogramOpts{
		Name:    "pharma_cronjob_duration_seconds",
		Help:    "Duration of cronjob executions",
		Buckets: prometheus.DefBuckets,
	}, []string{"job"})

	// ========== WhatsApp Connection Metrics ==========

	WhatsAppConnectionState = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_whatsapp_connection_state",
		Help: "WhatsApp connection state (0=disconnected, 1=connecting, 2=connected, 3=reconnecting, 4=failed)",
	})

	WhatsAppReconnectAttempts = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_whatsapp_reconnect_attempts_total",
		Help: "Total number of WhatsApp reconnection attempts",
	})

	WhatsAppReconnectFailures = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_whatsapp_reconnect_failures_total",
		Help: "Total number of failed WhatsApp reconnection cycles (max attempts reached)",
	})

	// Group sync metrics
	WhatsAppGroupSyncSuccess = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_whatsapp_group_sync_success_total",
		Help: "Total successful WhatsApp group syncs",
	})

	WhatsAppGroupSyncFailure = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_whatsapp_group_sync_failure_total",
		Help: "Total failed WhatsApp group syncs",
	}, []string{"reason"}) // reason: transient, max_retries, cancelled

	WhatsAppGroupSyncDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_whatsapp_group_sync_duration_seconds",
		Help:    "Duration of WhatsApp group sync operations",
		Buckets: []float64{0.5, 1, 2, 5, 10, 30},
	})

	WhatsAppGroupsSynced = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_whatsapp_groups_synced",
		Help: "Number of groups synced in last successful sync",
	})

	// ========== Message Queue Metrics ==========

	MessageQueueSize = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_message_queue_size",
		Help: "Current number of messages in the main queue",
	})

	MessageQueueDLQSize = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_message_queue_dlq_size",
		Help: "Current number of messages in the dead letter queue",
	})

	MessageQueueWorkers = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_message_queue_workers",
		Help: "Number of active message queue workers",
	})

	MessageQueueInFlight = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "pharma_message_queue_in_flight",
		Help: "Number of messages currently being processed",
	})

	MessagesReceived = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_messages_received_total",
		Help: "Total number of messages received from WhatsApp",
	})

	MessagesOverflow = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_messages_overflow_total",
		Help: "Total number of messages sent to dead letter queue",
	})

	MessagesDropped = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_messages_dropped_total",
		Help: "Total number of messages dropped due to queue overflow",
	})

	MessagesProcessedStatus = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "pharma_messages_processed_status_total",
		Help: "Total number of messages processed by status",
	}, []string{"status"}) // status: success, error

	MessageProcessingLatency = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "pharma_message_processing_latency_seconds",
		Help:    "Latency of individual message processing",
		Buckets: []float64{0.1, 0.5, 1, 2, 5, 10, 30},
	})

	// ========== Deduplication Metrics ==========

	DeduplicatorHits = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_deduplicator_hits_total",
		Help: "Total number of duplicate messages detected",
	})

	DeduplicatorMisses = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_deduplicator_misses_total",
		Help: "Total number of unique messages (not duplicates)",
	})

	// Cross-post deduplication (offer-level)
	DuplicatesSkipped = promauto.NewCounter(prometheus.CounterOpts{
		Name: "pharma_duplicates_skipped_total",
		Help: "Total number of duplicate offers skipped (cross-post detection)",
	})
)
