package sse

import (
	"testing"

	"github.com/rs/zerolog"
)

// =============================================================================
// EventLog Tests
// =============================================================================

func TestNewEventLog(t *testing.T) {
	log := NewEventLog(100)
	if log == nil {
		t.Fatal("NewEventLog returned nil")
	}
	if log.capacity != 100 {
		t.Errorf("capacity = %d, want 100", log.capacity)
	}
}

func TestNewEventLog_DefaultCapacity(t *testing.T) {
	log := NewEventLog(0)
	if log.capacity != 1000 {
		t.Errorf("capacity = %d, want 1000 (default)", log.capacity)
	}
}

func TestEventLog_Push(t *testing.T) {
	log := NewEventLog(10)

	log.Push(SequencedEvent{Sequence: 1, Type: "test"})
	log.Push(SequencedEvent{Sequence: 2, Type: "test"})

	if log.Count() != 2 {
		t.Errorf("Count() = %d, want 2", log.Count())
	}
}

func TestEventLog_Push_CircularBuffer(t *testing.T) {
	log := NewEventLog(5)

	// Push more than capacity
	for i := uint64(1); i <= 10; i++ {
		log.Push(SequencedEvent{Sequence: i, Type: "test"})
	}

	// Should only have 5 events
	if log.Count() != 5 {
		t.Errorf("Count() = %d, want 5", log.Count())
	}

	// Latest sequence should be 10
	if log.GetLatestSequence() != 10 {
		t.Errorf("GetLatestSequence() = %d, want 10", log.GetLatestSequence())
	}
}

func TestEventLog_GetSince(t *testing.T) {
	log := NewEventLog(100)

	for i := uint64(1); i <= 10; i++ {
		log.Push(SequencedEvent{Sequence: i, Type: "test"})
	}

	// Get events since sequence 5
	events := log.GetSince(5)
	if len(events) != 5 {
		t.Errorf("GetSince(5) returned %d events, want 5", len(events))
	}

	// First event should be sequence 6
	if events[0].Sequence != 6 {
		t.Errorf("First event sequence = %d, want 6", events[0].Sequence)
	}
}

func TestEventLog_GetSince_Empty(t *testing.T) {
	log := NewEventLog(100)

	events := log.GetSince(0)
	if len(events) != 0 {
		t.Errorf("GetSince on empty log returned %d events, want 0", len(events))
	}
}

func TestEventLog_GetLatestSequence_Empty(t *testing.T) {
	log := NewEventLog(100)

	if log.GetLatestSequence() != 0 {
		t.Errorf("GetLatestSequence on empty log = %d, want 0", log.GetLatestSequence())
	}
}

// =============================================================================
// SequencedSSEHub Tests
// =============================================================================

func TestNewSequencedSSEHub(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)

	if hub == nil {
		t.Fatal("NewSequencedSSEHub returned nil")
	}

	// Cleanup
	hub.Shutdown()
}

func TestSequencedSSEHub_BroadcastSequenced(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	seq1 := hub.BroadcastSequenced("test", map[string]string{"key": "value1"})
	seq2 := hub.BroadcastSequenced("test", map[string]string{"key": "value2"})

	if seq1 != 1 {
		t.Errorf("First sequence = %d, want 1", seq1)
	}
	if seq2 != 2 {
		t.Errorf("Second sequence = %d, want 2", seq2)
	}
}

func TestSequencedSSEHub_ReplayFrom(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	// Broadcast some events
	for i := 0; i < 10; i++ {
		hub.BroadcastSequenced("test", i)
	}

	// Replay from sequence 5
	events := hub.ReplayFrom(5)
	if len(events) != 5 {
		t.Errorf("ReplayFrom(5) returned %d events, want 5", len(events))
	}
}

func TestSequencedSSEHub_GetLatestSequence(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	if hub.GetLatestSequence() != 0 {
		t.Errorf("Initial sequence = %d, want 0", hub.GetLatestSequence())
	}

	hub.BroadcastSequenced("test", nil)
	hub.BroadcastSequenced("test", nil)

	if hub.GetLatestSequence() != 2 {
		t.Errorf("After 2 broadcasts, sequence = %d, want 2", hub.GetLatestSequence())
	}
}

func TestSequencedSSEHub_BroadcastNewOfferSequenced(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	seq := hub.BroadcastNewOfferSequenced("offer-1", "Aspirin")
	if seq != 1 {
		t.Errorf("BroadcastNewOfferSequenced sequence = %d, want 1", seq)
	}
}

func TestSequencedSSEHub_BroadcastNewRequestSequenced(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	seq := hub.BroadcastNewRequestSequenced("request-1", "Paracetamol")
	if seq != 1 {
		t.Errorf("BroadcastNewRequestSequenced sequence = %d, want 1", seq)
	}
}

func TestSequencedSSEHub_BroadcastNewMatchSequenced(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	seq := hub.BroadcastNewMatchSequenced("match-1", 0.95)
	if seq != 1 {
		t.Errorf("BroadcastNewMatchSequenced sequence = %d, want 1", seq)
	}
}

func TestSequencedSSEHub_GetEventLogCount(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSequencedSSEHub(100, 1000, log)
	defer hub.Shutdown()

	if hub.GetEventLogCount() != 0 {
		t.Errorf("Initial event log count = %d, want 0", hub.GetEventLogCount())
	}

	hub.BroadcastSequenced("test", nil)
	hub.BroadcastSequenced("test", nil)

	if hub.GetEventLogCount() != 2 {
		t.Errorf("After 2 broadcasts, event log count = %d, want 2", hub.GetEventLogCount())
	}
}
