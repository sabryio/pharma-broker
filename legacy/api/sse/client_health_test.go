package sse

import (
	"testing"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// ClientHealth Tests
// =============================================================================

func TestNewClientHealth(t *testing.T) {
	health := NewClientHealth("client-1")

	if health.ClientID != "client-1" {
		t.Errorf("ClientID = %s, want client-1", health.ClientID)
	}
	if health.EventsSent.Load() != 0 {
		t.Errorf("EventsSent = %d, want 0", health.EventsSent.Load())
	}
	if health.MissedEvents.Load() != 0 {
		t.Errorf("MissedEvents = %d, want 0", health.MissedEvents.Load())
	}
}

func TestClientHealth_RecordEvent(t *testing.T) {
	health := NewClientHealth("client-1")

	// Record some missed events first
	health.RecordMissed()
	health.RecordMissed()

	if health.MissedEvents.Load() != 2 {
		t.Errorf("MissedEvents = %d, want 2", health.MissedEvents.Load())
	}

	// Recording an event should reset missed counter
	health.RecordEvent()

	if health.EventsSent.Load() != 1 {
		t.Errorf("EventsSent = %d, want 1", health.EventsSent.Load())
	}
	if health.MissedEvents.Load() != 0 {
		t.Errorf("MissedEvents after RecordEvent = %d, want 0", health.MissedEvents.Load())
	}
}

func TestClientHealth_RecordMissed(t *testing.T) {
	health := NewClientHealth("client-1")

	health.RecordMissed()
	health.RecordMissed()
	health.RecordMissed()

	if health.MissedEvents.Load() != 3 {
		t.Errorf("MissedEvents = %d, want 3", health.MissedEvents.Load())
	}
}

func TestClientHealth_IsHealthy(t *testing.T) {
	health := NewClientHealth("client-1")

	// Should be healthy initially
	if !health.IsHealthy(50, 5*time.Minute) {
		t.Error("New client should be healthy")
	}

	// Too many missed events
	for range 60 {
		health.RecordMissed()
	}
	if health.IsHealthy(50, 5*time.Minute) {
		t.Error("Client with 60 missed events should not be healthy (max 50)")
	}
}

func TestClientHealth_GetStats(t *testing.T) {
	health := NewClientHealth("client-1")
	health.RecordEvent()
	health.RecordEvent()

	stats := health.GetStats()

	if stats["client_id"] != "client-1" {
		t.Errorf("client_id = %v, want client-1", stats["client_id"])
	}
	if stats["events_sent"] != int64(2) {
		t.Errorf("events_sent = %v, want 2", stats["events_sent"])
	}
}

// =============================================================================
// ClientHealthConfig Tests
// =============================================================================

func TestDefaultClientHealthConfig(t *testing.T) {
	cfg := DefaultClientHealthConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.MaxMissedEvents != 50 {
		t.Errorf("MaxMissedEvents = %d, want 50", cfg.MaxMissedEvents)
	}
	if cfg.CheckInterval != 10*time.Second {
		t.Errorf("CheckInterval = %v, want 10s", cfg.CheckInterval)
	}
}

// =============================================================================
// ClientHealthMonitor Tests
// =============================================================================

func TestNewClientHealthMonitor(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	if monitor == nil {
		t.Fatal("NewClientHealthMonitor returned nil")
	}
}

func TestClientHealthMonitor_RegisterClient(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	clientChan := make(chan SSEEvent, 10)
	monitor.RegisterClient(clientChan, "client-1")

	if monitor.GetClientCount() != 1 {
		t.Errorf("GetClientCount() = %d, want 1", monitor.GetClientCount())
	}
}

func TestClientHealthMonitor_UnregisterClient(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	clientChan := make(chan SSEEvent, 10)
	monitor.RegisterClient(clientChan, "client-1")
	monitor.UnregisterClient(clientChan)

	if monitor.GetClientCount() != 0 {
		t.Errorf("GetClientCount() after unregister = %d, want 0", monitor.GetClientCount())
	}
}

func TestClientHealthMonitor_RecordEvent(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	clientChan := make(chan SSEEvent, 10)
	monitor.RegisterClient(clientChan, "client-1")
	monitor.RecordEvent(clientChan)

	stats := monitor.GetAllStats()
	if len(stats) != 1 {
		t.Fatalf("GetAllStats() returned %d stats, want 1", len(stats))
	}
	if stats[0]["events_sent"] != int64(1) {
		t.Errorf("events_sent = %v, want 1", stats[0]["events_sent"])
	}
}

func TestClientHealthMonitor_RecordMissed(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	clientChan := make(chan SSEEvent, 10)
	monitor.RegisterClient(clientChan, "client-1")
	monitor.RecordMissed(clientChan)
	monitor.RecordMissed(clientChan)

	stats := monitor.GetAllStats()
	if stats[0]["missed_events"] != int32(2) {
		t.Errorf("missed_events = %v, want 2", stats[0]["missed_events"])
	}
}

func TestClientHealthMonitor_SetEvictCallback(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	evicted := false
	monitor.SetEvictCallback(func(ch chan SSEEvent, reason string) {
		evicted = true
	})

	// Manually trigger eviction check with unhealthy client
	clientChan := make(chan SSEEvent, 10)
	monitor.RegisterClient(clientChan, "client-1")

	// Make client unhealthy
	for range 60 {
		monitor.RecordMissed(clientChan)
	}

	monitor.checkClients()

	if !evicted {
		t.Error("Evict callback should have been called")
	}
}

func TestClientHealthMonitor_GetConfig(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultClientHealthConfig()
	monitor := NewClientHealthMonitor(cfg, log)

	gotCfg := monitor.GetConfig()
	if gotCfg.MaxMissedEvents != cfg.MaxMissedEvents {
		t.Errorf("MaxMissedEvents = %d, want %d", gotCfg.MaxMissedEvents, cfg.MaxMissedEvents)
	}
}

func TestClientHealthMonitor_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	monitor := NewClientHealthMonitor(DefaultClientHealthConfig(), log)

	newCfg := ClientHealthConfig{
		MaxMissedEvents: 100,
		CheckInterval:   20 * time.Second,
		Enabled:         true,
	}
	monitor.SetConfig(newCfg)

	gotCfg := monitor.GetConfig()
	if gotCfg.MaxMissedEvents != 100 {
		t.Errorf("MaxMissedEvents = %d, want 100", gotCfg.MaxMissedEvents)
	}
}
