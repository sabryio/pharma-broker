package resilience

import (
	"context"
	"errors"
	"testing"
	"time"

	pb "pharma-bridge/proto"
)

func TestCircuitBreaker(t *testing.T) {
	cb := NewCircuitBreaker(2, 100*time.Millisecond)

	// Closed -> Open
	if !cb.Allow() {
		t.Error("Should allow when closed")
	}
	cb.RecordFailure()
	if !cb.Allow() {
		t.Error("Should allow after 1 failure")
	}
	cb.RecordFailure()
	if cb.Allow() {
		t.Error("Should not allow after 2 failures (Open)")
	}

	// Open -> Half-Open
	time.Sleep(150 * time.Millisecond)
	if !cb.Allow() {
		t.Error("Should allow after timeout (Half-Open)")
	}
	if cb.State() != StateHalfOpen {
		t.Errorf("Expected HalfOpen, got %v", cb.State())
	}

	// Half-Open -> Open (Failure again)
	cb.RecordFailure()
	if cb.State() != StateOpen {
		t.Errorf("Expected Open after failure in HalfOpen, got %v", cb.State())
	}

	// Open -> Half-Open -> Closed (Success)
	time.Sleep(150 * time.Millisecond)
	cb.Allow()
	cb.RecordSuccess()
	if cb.State() != StateClosed {
		t.Errorf("Expected Closed after success in HalfOpen, got %v", cb.State())
	}
	if !cb.Allow() {
		t.Error("Should allow after closing")
	}
}

func TestRetryBuffer(t *testing.T) {
	count := 0
	flushErr := errors.New("failed")

	onFlush := func(ctx context.Context, msg *pb.RawMessage) error {
		count++
		return flushErr
	}

	buffer := NewRetryBuffer(2, onFlush)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	buffer.Start(ctx)

	msg := &pb.RawMessage{Id: "1"}
	if !buffer.Add(msg) {
		t.Error("Should add message 1")
	}
	if !buffer.Add(&pb.RawMessage{Id: "2"}) {
		t.Error("Should add message 2")
	}
	if buffer.Add(&pb.RawMessage{Id: "3"}) {
		t.Error("Should not add message 3 (Full)")
	}

	// Wait for background flush
	time.Sleep(50 * time.Millisecond)
	if count == 0 {
		t.Error("Should have tried to flush")
	}

	// Now make flush succeed
	flushErr = nil
	buffer.Flush()

	// Buffer should eventually clear
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if buffer.Size() == 0 {
			break
		}
		time.Sleep(100 * time.Millisecond)
	}

	if buffer.Size() != 0 {
		t.Errorf("Buffer should be empty, got %d", buffer.Size())
	}
}
