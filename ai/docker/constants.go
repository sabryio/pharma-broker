package ai

import "time"

// Constants for configuration defaults and magic numbers
const (
	DefaultVectorTopK       = 10
	DefaultMaxMessageLines  = 20
	DefaultConcurrencyLimit = 5
	DefaultMaxBatchSize     = 1
	MaxFuzzyDistance        = 2
	MaxLogTruncateLen       = 500
	DefaultEmbeddingModel   = "ai/embeddinggemma"

	// Circuit breaker defaults
	DefaultCBMaxRequests  = 3
	DefaultCBInterval     = 60 * time.Second
	DefaultCBTimeout      = 30 * time.Second
	DefaultCBFailureRatio = 0.6
	DefaultCBMinRequests  = 5
)
