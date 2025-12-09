// Package parsing provides message parsing service interfaces and types.
package parsing

import (
	"context"

	"pharmabroker/domain/entity"
)

// Service defines the message parsing service interface
type Service interface {
	// ProcessMessage queues a message for async processing
	ProcessMessage(ctx context.Context, msg *entity.RawMessage)

	// ParseBatch synchronously parses a batch of messages
	ParseBatch(ctx context.Context, messages []*entity.RawMessage) ([]*Result, error)

	// Start begins the background processing loop
	Start(ctx context.Context)

	// Stop stops the parser
	Stop()
}

// Result represents the result of parsing a single message
type Result struct {
	MessageID string
	Items     []ParsedItem
	Error     string
	Pass      int // Which parsing pass produced this result
}

// ParsedItem represents a single extracted item from a message
type ParsedItem struct {
	Type            ItemType
	Medication      string
	MedicationRaw   string
	Quantity        float64
	Unit            *string
	Price           float64
	MaxPrice        float64
	Currency        string
	Urgent          bool
	Notes           string
	MatchConfidence string  // EXACT, FUZZY, VECTOR, TRANSLITERATED
	AIConfidence    float64 // 0.0 - 1.0
}

// ItemType represents the type of parsed item
type ItemType string

const (
	ItemTypeOffer   ItemType = "OFFER"
	ItemTypeRequest ItemType = "REQUEST"
	ItemTypeBoth    ItemType = "BOTH"
)

// Config holds parsing configuration
type Config struct {
	BatchSize           int
	BatchTimeout        int // seconds
	WorkerCount         int
	RetryAttempts       int
	ConfidenceThreshold float64 // minimum confidence to accept
}

// MultiPassConfig configures multi-pass parsing behavior
type MultiPassConfig struct {
	// StrictMinConfidence is the minimum average confidence to accept Pass 1 results
	StrictMinConfidence float64 // Default: 0.7

	// RelaxedMinConfidence is the minimum confidence to accept Pass 2 results
	RelaxedMinConfidence float64 // Default: 0.4

	// EnablePass2 enables the relaxed fallback pass
	EnablePass2 bool // Default: true

	// EnableReviewQueue enables queuing low-confidence results for review
	EnableReviewQueue bool // Default: true
}

// ErrorNotifier interface for reporting system errors
type ErrorNotifier interface {
	NotifyError(err error)
}

// SSEBroadcaster interface for real-time updates
type SSEBroadcaster interface {
	BroadcastNewOffer(offerID string, medication string)
	BroadcastNewRequest(requestID string, medication string)
	BroadcastNewMatch(matchID string, score float64)
}

// Compile-time check
var _ = entity.RawMessage{}
