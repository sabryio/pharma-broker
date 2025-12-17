package sse

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// Sequenced Event
// =============================================================================

// SequencedEvent wraps an SSE event with sequence number and timestamp.
type SequencedEvent struct {
	Sequence  uint64    `json:"seq"`
	Type      string    `json:"type"`
	Data      any       `json:"data"`
	Timestamp time.Time `json:"timestamp"`
}

// =============================================================================
// Event Log (Ring Buffer)
// =============================================================================

// EventLog stores recent events for replay on reconnect.
type EventLog struct {
	events   []SequencedEvent
	capacity int
	head     int
	count    int
	mu       sync.RWMutex
}

// NewEventLog creates a new event log with the given capacity.
func NewEventLog(capacity int) *EventLog {
	if capacity <= 0 {
		capacity = 1000
	}
	return &EventLog{
		events:   make([]SequencedEvent, capacity),
		capacity: capacity,
	}
}

// Push adds an event to the log.
func (l *EventLog) Push(event SequencedEvent) {
	l.mu.Lock()
	defer l.mu.Unlock()

	l.events[l.head] = event
	l.head = (l.head + 1) % l.capacity
	if l.count < l.capacity {
		l.count++
	}
}

// GetSince returns all events with sequence > sinceSeq.
func (l *EventLog) GetSince(sinceSeq uint64) []SequencedEvent {
	l.mu.RLock()
	defer l.mu.RUnlock()

	var result []SequencedEvent

	// Calculate start position
	start := 0
	if l.count == l.capacity {
		start = l.head
	}

	for i := 0; i < l.count; i++ {
		idx := (start + i) % l.capacity
		if l.events[idx].Sequence > sinceSeq {
			result = append(result, l.events[idx])
		}
	}

	return result
}

// GetLatestSequence returns the latest sequence number.
func (l *EventLog) GetLatestSequence() uint64 {
	l.mu.RLock()
	defer l.mu.RUnlock()

	if l.count == 0 {
		return 0
	}

	// Latest is at head-1
	idx := (l.head - 1 + l.capacity) % l.capacity
	return l.events[idx].Sequence
}

// Count returns the number of events in the log.
func (l *EventLog) Count() int {
	l.mu.RLock()
	defer l.mu.RUnlock()
	return l.count
}

// =============================================================================
// Sequenced SSE Hub
// =============================================================================

// SequencedSSEHub extends SSEHub with event sequencing and replay.
type SequencedSSEHub struct {
	*SSEHub
	sequence atomic.Uint64
	eventLog *EventLog
	log      zerolog.Logger
}

// NewSequencedSSEHub creates a new sequenced SSE hub.
func NewSequencedSSEHub(maxClients int, logCapacity int, log zerolog.Logger) *SequencedSSEHub {
	return &SequencedSSEHub{
		SSEHub:   NewSSEHubWithLimit(maxClients),
		eventLog: NewEventLog(logCapacity),
		log:      log.With().Str("component", "sse-sequenced").Logger(),
	}
}

// BroadcastSequenced sends a sequenced event to all clients.
func (h *SequencedSSEHub) BroadcastSequenced(eventType string, data any) uint64 {
	seq := h.sequence.Add(1)

	seqEvent := SequencedEvent{
		Sequence:  seq,
		Type:      eventType,
		Data:      data,
		Timestamp: time.Now(),
	}

	// Store in event log for replay
	h.eventLog.Push(seqEvent)

	// Broadcast to clients (wrap in SSEEvent format)
	h.SSEHub.Broadcast(SSEEvent{
		Type: eventType,
		Data: map[string]any{
			"seq":       seq,
			"data":      data,
			"timestamp": seqEvent.Timestamp.Unix(),
		},
	})

	return seq
}

// ReplayFrom returns all events since the given sequence number.
func (h *SequencedSSEHub) ReplayFrom(sinceSeq uint64) []SequencedEvent {
	return h.eventLog.GetSince(sinceSeq)
}

// GetLatestSequence returns the current sequence number.
func (h *SequencedSSEHub) GetLatestSequence() uint64 {
	return h.sequence.Load()
}

// GetEventLogCount returns the number of events in the log.
func (h *SequencedSSEHub) GetEventLogCount() int {
	return h.eventLog.Count()
}

// BroadcastNewOfferSequenced sends a sequenced new offer event.
func (h *SequencedSSEHub) BroadcastNewOfferSequenced(offerID, medication string) uint64 {
	return h.BroadcastSequenced("new_offer", map[string]string{
		"id":         offerID,
		"medication": medication,
	})
}

// BroadcastNewRequestSequenced sends a sequenced new request event.
func (h *SequencedSSEHub) BroadcastNewRequestSequenced(requestID, medication string) uint64 {
	return h.BroadcastSequenced("new_request", map[string]string{
		"id":         requestID,
		"medication": medication,
	})
}

// BroadcastNewMatchSequenced sends a sequenced new match event.
func (h *SequencedSSEHub) BroadcastNewMatchSequenced(matchID string, score float64) uint64 {
	return h.BroadcastSequenced("new_match", map[string]any{
		"id":    matchID,
		"score": score,
	})
}
