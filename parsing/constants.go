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
)
