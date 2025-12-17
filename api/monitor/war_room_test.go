package monitor

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

// Mock implementations
type mockSender struct {
	mu       sync.Mutex
	messages []string
	err      error
}

func (m *mockSender) SendMessage(ctx context.Context, jid, msg string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.err != nil {
		return m.err
	}
	m.messages = append(m.messages, msg)
	return nil
}

func (m *mockSender) getMessages() []string {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]string{}, m.messages...)
}

type mockConfigProvider struct {
	config *entity.AppConfig
	err    error
}

func (m *mockConfigProvider) GetAll(ctx context.Context) (*entity.AppConfig, error) {
	if m.err != nil {
		return nil, m.err
	}
	return m.config, nil
}

func TestWarRoom_NotifyError_BelowThreshold(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: "123456789"}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 5,
		ErrorWindow:    time.Minute,
		AlertCooldown:  time.Minute,
	})

	// Report 4 errors (below threshold)
	for i := 0; i < 4; i++ {
		wr.NotifyError(context.Background(), errors.New("test error"))
	}

	// No alert should be sent
	if len(sender.getMessages()) != 0 {
		t.Errorf("expected no messages, got %d", len(sender.getMessages()))
	}
}

func TestWarRoom_NotifyError_AtThreshold(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: "123456789"}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 5,
		ErrorWindow:    time.Minute,
		AlertCooldown:  time.Minute,
	})

	// Report 5 errors (at threshold)
	for range 5 {
		wr.NotifyError(context.Background(), errors.New("test error"))
	}

	// Alert should be sent
	msgs := sender.getMessages()
	if len(msgs) != 1 {
		t.Errorf("expected 1 message, got %d", len(msgs))
	}
}

func TestWarRoom_AlertCooldown(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: "123456789"}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 2,
		ErrorWindow:    time.Minute,
		AlertCooldown:  time.Hour, // Long cooldown
	})

	// Trigger first alert
	wr.NotifyError(context.Background(), errors.New("error 1"))
	wr.NotifyError(context.Background(), errors.New("error 2"))

	// Try to trigger second alert (should be blocked by cooldown)
	wr.NotifyError(context.Background(), errors.New("error 3"))
	wr.NotifyError(context.Background(), errors.New("error 4"))

	// Only one alert should be sent
	msgs := sender.getMessages()
	if len(msgs) != 1 {
		t.Errorf("expected 1 message (cooldown should block second), got %d", len(msgs))
	}
}

func TestWarRoom_ErrorWindowCleanup(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: "123456789"}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 3,
		ErrorWindow:    50 * time.Millisecond, // Very short window
		AlertCooldown:  time.Millisecond,
	})

	// Report 2 errors
	wr.NotifyError(context.Background(), errors.New("error 1"))
	wr.NotifyError(context.Background(), errors.New("error 2"))

	// Wait for window to expire
	time.Sleep(100 * time.Millisecond)

	// Report 2 more errors (old ones should be cleaned up)
	wr.NotifyError(context.Background(), errors.New("error 3"))
	wr.NotifyError(context.Background(), errors.New("error 4"))

	// No alert should be sent (only 2 errors in window)
	msgs := sender.getMessages()
	if len(msgs) != 0 {
		t.Errorf("expected no messages (errors expired), got %d", len(msgs))
	}
}

func TestWarRoom_Metrics(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: "123456789"}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 2,
		ErrorWindow:    time.Minute,
		AlertCooldown:  time.Millisecond,
	})

	// Report errors
	wr.NotifyError(context.Background(), errors.New("error 1"))
	wr.NotifyError(context.Background(), errors.New("error 2"))
	wr.NotifyError(context.Background(), errors.New("error 3"))

	alerts, logged := wr.Metrics()
	if logged != 3 {
		t.Errorf("expected 3 errors logged, got %d", logged)
	}
	if alerts != 1 {
		t.Errorf("expected 1 alert sent, got %d", alerts)
	}
}

func TestWarRoom_NoAdminPhone(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{AdminPhone: ""}}
	log := zerolog.Nop()

	wr := NewWarRoomWithConfig(sender, cfg, log, WarRoomConfig{
		ErrorThreshold: 1,
		ErrorWindow:    time.Minute,
		AlertCooldown:  time.Minute,
	})

	wr.NotifyError(context.Background(), errors.New("error"))

	// No message should be sent (no admin phone)
	if len(sender.getMessages()) != 0 {
		t.Errorf("expected no messages when admin phone not configured")
	}
}

func TestFormatJID(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"123456789", "123456789@s.whatsapp.net"},
		{"123456789@s.whatsapp.net", "123456789@s.whatsapp.net"},
		{"group@g.us", "group@g.us"},
	}

	for _, tt := range tests {
		result := formatJID(tt.input)
		if result != tt.expected {
			t.Errorf("formatJID(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestWarRoom_Close(t *testing.T) {
	sender := &mockSender{}
	cfg := &mockConfigProvider{config: &entity.AppConfig{}}
	log := zerolog.Nop()

	wr := NewWarRoom(sender, cfg, log)

	// Close should not panic
	wr.Close()

	// Done channel should be closed
	select {
	case <-wr.Done():
		// Expected
	default:
		t.Error("Done channel should be closed after Close()")
	}
}
