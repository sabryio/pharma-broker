package whatsapp

import (
	"context"
	"testing"
	"time"
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

func TestDefaultReconnectConfig(t *testing.T) {
	cfg := DefaultReconnectConfig()

	if cfg.MaxAttempts != 0 {
		t.Errorf("MaxAttempts = %d, want 0 (infinite)", cfg.MaxAttempts)
	}
	if cfg.BaseDelay != 5*time.Second {
		t.Errorf("BaseDelay = %v, want 5s", cfg.BaseDelay)
	}
	if cfg.MaxDelay != 5*time.Minute {
		t.Errorf("MaxDelay = %v, want 5m", cfg.MaxDelay)
	}
	if cfg.JitterFactor != 0.1 {
		t.Errorf("JitterFactor = %f, want 0.1", cfg.JitterFactor)
	}
}

func TestCalculateBackoffDelay(t *testing.T) {
	// Create a minimal manager for testing
	m := &Manager{}

	tests := []struct {
		attempt      int
		baseDelay    time.Duration
		maxDelay     time.Duration
		jitterFactor float64
		minExpected  time.Duration
		maxExpected  time.Duration
	}{
		// No jitter tests
		{1, time.Second, time.Minute, 0, time.Second, time.Second},           // 1s * 2^0 = 1s
		{2, time.Second, time.Minute, 0, 2 * time.Second, 2 * time.Second},   // 1s * 2^1 = 2s
		{3, time.Second, time.Minute, 0, 4 * time.Second, 4 * time.Second},   // 1s * 2^2 = 4s
		{4, time.Second, time.Minute, 0, 8 * time.Second, 8 * time.Second},   // 1s * 2^3 = 8s
		{5, time.Second, time.Minute, 0, 16 * time.Second, 16 * time.Second}, // 1s * 2^4 = 16s

		// Cap at maxDelay
		{10, time.Second, 30 * time.Second, 0, 30 * time.Second, 30 * time.Second},
	}

	for _, tt := range tests {
		got := m.calculateBackoffDelay(tt.attempt, tt.baseDelay, tt.maxDelay, tt.jitterFactor)
		if got < tt.minExpected || got > tt.maxExpected {
			t.Errorf("attempt=%d: calculateBackoffDelay() = %v, want between %v and %v",
				tt.attempt, got, tt.minExpected, tt.maxExpected)
		}
	}
}

func TestCalculateBackoffDelay_WithJitter(t *testing.T) {
	m := &Manager{}

	baseDelay := 10 * time.Second
	maxDelay := time.Minute
	jitterFactor := 0.1 // 10%

	// Run multiple times to check jitter is applied
	results := make(map[time.Duration]bool)
	for i := 0; i < 50; i++ {
		delay := m.calculateBackoffDelay(1, baseDelay, maxDelay, jitterFactor)
		results[delay] = true

		// With 10% jitter, delay should be between 9s and 11s
		minExpected := time.Duration(float64(baseDelay) * 0.9)
		maxExpected := time.Duration(float64(baseDelay) * 1.1)

		if delay < minExpected || delay > maxExpected {
			t.Errorf("delay = %v, want between %v and %v", delay, minExpected, maxExpected)
		}
	}

	// With jitter, we should get some variation
	if len(results) < 2 {
		t.Logf("Warning: Expected some jitter variation, got %d unique values", len(results))
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

func TestReconnectConfig_Callbacks(t *testing.T) {
	var stateChangeCalled bool
	var maxAttemptsCalled bool

	cfg := ReconnectConfig{
		MaxAttempts:  5,
		BaseDelay:    100 * time.Millisecond,
		MaxDelay:     time.Second,
		JitterFactor: 0,
		OnStateChange: func(from, to ConnectionState) {
			stateChangeCalled = true
		},
		OnMaxAttempts: func() {
			maxAttemptsCalled = true
		},
	}

	// Test callbacks are set
	if cfg.OnStateChange == nil {
		t.Error("OnStateChange callback not set")
	}
	if cfg.OnMaxAttempts == nil {
		t.Error("OnMaxAttempts callback not set")
	}

	// Call callbacks
	cfg.OnStateChange(StateDisconnected, StateConnecting)
	cfg.OnMaxAttempts()

	if !stateChangeCalled {
		t.Error("OnStateChange was not called")
	}
	if !maxAttemptsCalled {
		t.Error("OnMaxAttempts was not called")
	}
}

func TestReconnectConfig_InfiniteRetry(t *testing.T) {
	cfg := DefaultReconnectConfig()

	// MaxAttempts = 0 means infinite retry
	if cfg.MaxAttempts != 0 {
		t.Errorf("Default config should have MaxAttempts=0 for infinite retry, got %d", cfg.MaxAttempts)
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
