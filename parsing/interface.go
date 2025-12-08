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
	MaxPasses           int
	ConfidenceThreshold float64
	EnableReviewQueue   bool
}

// ErrorNotifier receives error notifications
type ErrorNotifier interface {
	NotifyError(err error)
}

// SSEBroadcaster sends real-time updates
type SSEBroadcaster interface {
	BroadcastNewOffer(offerID, medication string)
	BroadcastNewRequest(requestID, medication string)
	BroadcastNewMatch(matchID string, score float64)
}

// Compile-time check
var _ = entity.RawMessage{}
