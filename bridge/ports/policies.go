package ports

import (
	"time"

	"pharma-bridge/domain"
)

// MessageFilter determines if a message should be processed.
// Implementations: GroupFilter, MonitoredFilter, etc.
type MessageFilter interface {
	// Allow returns true if the message should be processed.
	Allow(msg domain.Message) bool
}

// Deduplicator detects and filters duplicate messages.
type Deduplicator interface {
	// IsDuplicate returns true if the message is a duplicate.
	IsDuplicate(groupJID domain.JID, senderJID domain.JID, content string, timestamp time.Time) bool
	// Record stores the message for future deduplication checks.
	Record(groupJID domain.JID, senderJID domain.JID, content string, timestamp time.Time)
	// Close releases resources.
	Close()
}

// RateLimiter controls the rate of outgoing messages.
type RateLimiter interface {
	// Allow returns true if a message can be sent immediately.
	Allow() bool
}

// GroupCache provides fast lookup for monitored groups.
type GroupCache interface {
	// IsMonitored returns true if the group is being monitored.
	IsMonitored(jid domain.JID) bool
	// Update sets the monitored JIDs.
	Update(jids []domain.JID)
}

// CircuitBreaker prevents cascading failures.
type CircuitBreaker interface {
	// Allow returns true if the call is permitted.
	Allow() bool
	// RecordSuccess records a successful call.
	RecordSuccess()
	// RecordFailure records a failed call.
	RecordFailure()
}

// RetryBuffer holds messages that failed to forward.
type RetryBuffer interface {
	// Add appends a message to the buffer.
	Add(msg domain.Message) bool
	// Size returns the current buffer size.
	Size() int
	// Start begins background flushing.
	Start()
	// Stop stops background flushing.
	Stop()
}
