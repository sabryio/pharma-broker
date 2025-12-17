package parsing

import "time"

// Default configuration constants
const (
	// Worker pool configuration
	DefaultWorkerCount      = 10
	DefaultMatchPoolSize    = 5
	DefaultInputChannelSize = 100

	// Batch processing
	DefaultBatchSize     = 10
	DefaultBatchInterval = 5 * time.Second

	// FTS and fuzzy matching
	MaxFTSTokens          = 50
	MaxFuzzyQueries       = 20
	MinWordLengthForFuzzy = 4
	MaxQueryLength        = 10000 // Prevent massive FTS queries

	// Levenshtein optimization
	MaxLevenshteinLength = 100

	// Match thresholds
	DefaultMatchThreshold    = 0.5
	DefaultMessageBufferSize = 1000

	// Circuit breaker
	DefaultCircuitBreakerThreshold = 5
	DefaultCircuitBreakerTimeout   = 30 * time.Second

	// Embedding refresh
	EmbeddingRefreshTimeout = 2 * time.Minute

	// AI Retry configuration
	DefaultMaxRetries      = 3                // Maximum retry attempts
	DefaultRetryBaseDelay  = 1 * time.Second  // Initial delay before first retry
	DefaultRetryMaxDelay   = 30 * time.Second // Maximum delay between retries
	DefaultRetryMultiplier = 2.0              // Exponential backoff multiplier
	DefaultRetryJitter     = 0.1              // Jitter factor (10% randomization)

	// Token-Aware Batching configuration
	DefaultMaxTokensPerBatch = 6000 // Max tokens per AI batch (leaves room for response)
	DefaultPromptOverhead    = 2000 // Estimated tokens for system prompt + mappings
	DefaultTokensPerMessage  = 50   // Overhead per message structure (labels, formatting)

	// Dynamic Confidence Thresholds
	DefaultStrictConfidence           = 0.7  // Minimum confidence for strict pass
	DefaultRelaxedConfidence          = 0.4  // Minimum confidence for relaxed pass
	DefaultConfidenceAdjustmentStep   = 0.02 // Step size for adaptive adjustment
	DefaultMinConfidenceThreshold     = 0.3  // Minimum allowed threshold
	DefaultMaxConfidenceThreshold     = 0.95 // Maximum allowed threshold
	DefaultConfidenceEvaluationWindow = 100  // Results to evaluate before adjusting
	DefaultTargetAcceptRate           = 0.85 // Target acceptance rate
	DefaultAcceptRateTolerance        = 0.05 // Tolerance around target rate
)

// Match filtering constants (not in const block due to time.Duration)
var (
	DefaultMaxOfferAge = 7 * 24 * time.Hour // 7 days - offers older than this are considered stale
)

// Auto-action constants
const (
	DefaultAutoConfirmThreshold = 0.9 // Minimum score for auto-confirmation
)
