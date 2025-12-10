package whatsapp

import (
	"context"
	"testing"
	"time"

	"pharmabroker/messaging/reconnector"
)

func TestConnectionState_String(t *testing.T) {
	tests := []struct {
		state    ConnectionState
		expected string
	}{
		{StateDisconnected, "DISCONNECTED"},
		{StateConnecting, "CONNECTING"},
		{StateConnected, "CONNECTED"},
		{StateReconnecting, "RECONNECTING"},
		{StateFailed, "FAILED"},
		{ConnectionState(99), "UNKNOWN"},
	}

	for _, tt := range tests {
		if got := tt.state.String(); got != tt.expected {
			t.Errorf("ConnectionState(%d).String() = %s, want %s", tt.state, got, tt.expected)
		}
	}
}

func TestDefaultReconnectorConfig(t *testing.T) {
	cfg := reconnector.DefaultReconnectorConfig()

	if cfg.MaxRetries != 0 {
		t.Errorf("MaxRetries = %d, want 0 (infinite)", cfg.MaxRetries)
	}
	if cfg.InitialInterval != 5*time.Second {
		t.Errorf("InitialInterval = %v, want 5s", cfg.InitialInterval)
	}
	if cfg.MaxInterval != 5*time.Minute {
		t.Errorf("MaxInterval = %v, want 5m", cfg.MaxInterval)
	}
	if cfg.RandomizationFactor != 0.1 {
		t.Errorf("RandomizationFactor = %f, want 0.1", cfg.RandomizationFactor)
	}
	if cfg.Multiplier != 2.0 {
		t.Errorf("Multiplier = %f, want 2.0", cfg.Multiplier)
	}
}

func TestConnectionStatus(t *testing.T) {
	status := ConnectionStatus{
		State:           StateConnected,
		ReconnectCount:  3,
		LastConnectedAt: time.Now(),
		UptimeSeconds:   3600,
	}

	if status.State != StateConnected {
		t.Errorf("State = %v, want CONNECTED", status.State)
	}
	if status.ReconnectCount != 3 {
		t.Errorf("ReconnectCount = %d, want 3", status.ReconnectCount)
	}
	if status.UptimeSeconds != 3600 {
		t.Errorf("UptimeSeconds = %d, want 3600", status.UptimeSeconds)
	}
}

type mockAlerter struct {
	called   bool
	severity string
	title    string
	message  string
}

func (m *mockAlerter) SendAlert(_ context.Context, severity, title, message string) error {
	m.called = true
	m.severity = severity
	m.title = title
	m.message = message
	return nil
}

func TestAlerterInterface(t *testing.T) {
	alerter := &mockAlerter{}

	// Verify it implements the interface
	var _ AlertNotifier = alerter

	// Test calling
	ctx := context.Background()
	err := alerter.SendAlert(ctx, "critical", "Test", "Test message")
	if err != nil {
		t.Errorf("SendAlert returned error: %v", err)
	}
	if !alerter.called {
		t.Error("SendAlert was not called")
	}
	if alerter.severity != "critical" {
		t.Errorf("severity = %s, want critical", alerter.severity)
	}
}
