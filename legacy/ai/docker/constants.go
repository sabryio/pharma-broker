package ai

import "time"

// Constants for configuration defaults and magic numbers.
const (
	// Message processing defaults
	DefaultVectorTopK       = 10
	DefaultMaxMessageLines  = 20
	DefaultConcurrencyLimit = 5
	DefaultMaxBatchSize     = 1

	// Matching defaults
	MaxFuzzyDistance  = 2
	MaxLogTruncateLen = 500

	// Model defaults
	DefaultEmbeddingModel = "ai/embeddinggemma"

	// Circuit breaker defaults
	DefaultCBMaxRequests  = 3                // Failure threshold before opening
	DefaultCBTimeout      = 30 * time.Second // Time before transitioning to half-open
	DefaultCBFailureRatio = 0.6              // Failure ratio threshold (0-1)
	DefaultCBMinRequests  = 5                // Minimum requests before ratio is evaluated
)
