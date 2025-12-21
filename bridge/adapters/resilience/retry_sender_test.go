package resilienceadapter

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/rs/zerolog"

	"pharma-bridge/domain"
)

type mockSink struct {
	messages []domain.Message
	mu       sync.Mutex
	err      error
	failN    int
	calls    int
}

func (m *mockSink) Send(_ context.Context, msg domain.Message) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.calls++
	if m.failN > 0 && m.calls <= m.failN {
		return m.err
	}
	if m.err != nil && m.failN == 0 {
		return m.err
	}
	m.messages = append(m.messages, msg)
	return nil
}

func (m *mockSink) Close() error { return nil }

func (m *mockSink) GetMessages() []domain.Message {
	m.mu.Lock()
	defer m.mu.Unlock()
	result := make([]domain.Message, len(m.messages))
	copy(result, m.messages)
	return result
}

func TestNewRetrySender(t *testing.T) {
	sender := NewRetrySender(&mockSink{}, DefaultRetrySenderConfig(), zerolog.Nop())

	if sender == nil {
		t.Fatal("Expected sender to be created")
	}
	if sender.maxSize != 1000 {
		t.Errorf("Expected maxSize 1000, got %d", sender.maxSize)
	}
}

func TestRetrySender_SendSuccess(t *testing.T) {
	inner := &mockSink{}
	sender := NewRetrySender(inner, DefaultRetrySenderConfig(), zerolog.Nop())

	msg := domain.Message{ID: "msg1", Content: "Hello"}

	err := sender.Send(context.Background(), msg)
	if err != nil {
		t.Errorf("Expected no error, got %v", err)
	}

	if len(inner.GetMessages()) != 1 {
		t.Errorf("Expected 1 message, got %d", len(inner.GetMessages()))
	}
}

func TestRetrySender_SendFailure_BuffersMessage(t *testing.T) {
	inner := &mockSink{err: errors.New("connection failed")}
	sender := NewRetrySender(inner, DefaultRetrySenderConfig(), zerolog.Nop())

	msg := domain.Message{ID: "msg1", Content: "Hello"}

	err := sender.Send(context.Background(), msg)
	if err == nil {
		t.Error("Expected error, got nil")
	}

	if sender.Size() != 1 {
		t.Errorf("Expected buffer size 1, got %d", sender.Size())
	}
}

func TestRetrySender_FlushRetries(t *testing.T) {
	inner := &mockSink{
		err:   errors.New("connection failed"),
		failN: 1,
	}
	cfg := RetrySenderConfig{
		MaxSize:       100,
		FlushInterval: 50 * time.Millisecond,
	}

	sender := NewRetrySender(inner, cfg, zerolog.Nop())
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sender.Start(ctx, cfg)

	msg := domain.Message{ID: "msg1", Content: "Hello"}

	err := sender.Send(context.Background(), msg)
	if err == nil {
		t.Error("Expected error on first send")
	}

	time.Sleep(100 * time.Millisecond)

	if len(inner.GetMessages()) != 1 {
		t.Errorf("Expected 1 message after retry, got %d", len(inner.GetMessages()))
	}

	if sender.Size() != 0 {
		t.Errorf("Expected buffer size 0 after flush, got %d", sender.Size())
	}
}

func TestRetrySender_BufferSizeLimit(t *testing.T) {
	inner := &mockSink{err: errors.New("connection failed")}
	cfg := RetrySenderConfig{
		MaxSize:       3,
		FlushInterval: time.Hour,
	}

	sender := NewRetrySender(inner, cfg, zerolog.Nop())

	for i := 0; i < 5; i++ {
		msg := domain.Message{
			ID:      domain.MessageID("msg" + string(rune('0'+i))),
			Content: "Hello",
		}
		sender.Send(context.Background(), msg)
	}

	if sender.Size() != 3 {
		t.Errorf("Expected buffer size 3 (max), got %d", sender.Size())
	}
}

func TestDefaultRetrySenderConfig(t *testing.T) {
	cfg := DefaultRetrySenderConfig()

	if cfg.MaxSize != 1000 {
		t.Errorf("Expected MaxSize 1000, got %d", cfg.MaxSize)
	}
	if cfg.FlushInterval != 10*time.Second {
		t.Errorf("Expected FlushInterval 10s, got %v", cfg.FlushInterval)
	}
}
