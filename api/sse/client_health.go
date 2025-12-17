package sse

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// Client Health Configuration
// =============================================================================

// ClientHealthConfig configures client health monitoring.
type ClientHealthConfig struct {
	// Maximum missed events before disconnecting client
	MaxMissedEvents int32

	// Check interval for slow clients
	CheckInterval time.Duration

	// Inactivity timeout (no events sent)
	InactivityTimeout time.Duration

	// Enable health monitoring
	Enabled bool
}

// DefaultClientHealthConfig returns sensible defaults.
func DefaultClientHealthConfig() ClientHealthConfig {
	return ClientHealthConfig{
		MaxMissedEvents:   50,
		CheckInterval:     10 * time.Second,
		InactivityTimeout: 5 * time.Minute,
		Enabled:           true,
	}
}

// =============================================================================
// Client Health Tracker
// =============================================================================

// ClientHealth tracks health metrics for a single client.
type ClientHealth struct {
	MissedEvents atomic.Int32
	LastActivity time.Time
	ConnectedAt  time.Time
	EventsSent   atomic.Int64
	ClientID     string
	mu           sync.RWMutex
}

// NewClientHealth creates a new client health tracker.
func NewClientHealth(clientID string) *ClientHealth {
	now := time.Now()
	return &ClientHealth{
		LastActivity: now,
		ConnectedAt:  now,
		ClientID:     clientID,
	}
}

// RecordEvent records a successfully sent event.
func (h *ClientHealth) RecordEvent() {
	h.mu.Lock()
	h.LastActivity = time.Now()
	h.mu.Unlock()
	h.EventsSent.Add(1)
	h.MissedEvents.Store(0) // Reset missed counter on success
}

// RecordMissed records a missed event.
func (h *ClientHealth) RecordMissed() {
	h.MissedEvents.Add(1)
}

// GetLastActivity returns the last activity time.
func (h *ClientHealth) GetLastActivity() time.Time {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.LastActivity
}

// IsHealthy checks if the client is healthy.
func (h *ClientHealth) IsHealthy(maxMissed int32, inactivityTimeout time.Duration) bool {
	if h.MissedEvents.Load() > maxMissed {
		return false
	}

	h.mu.RLock()
	inactive := time.Since(h.LastActivity) > inactivityTimeout
	h.mu.RUnlock()

	return !inactive
}

// GetStats returns client health statistics.
func (h *ClientHealth) GetStats() map[string]any {
	h.mu.RLock()
	lastActivity := h.LastActivity
	h.mu.RUnlock()

	return map[string]any{
		"client_id":     h.ClientID,
		"connected_at":  h.ConnectedAt,
		"last_activity": lastActivity,
		"events_sent":   h.EventsSent.Load(),
		"missed_events": h.MissedEvents.Load(),
		"uptime_secs":   time.Since(h.ConnectedAt).Seconds(),
	}
}

// =============================================================================
// Client Health Monitor
// =============================================================================

// ClientHealthMonitor monitors all connected clients.
type ClientHealthMonitor struct {
	config  ClientHealthConfig
	clients map[chan SSEEvent]*ClientHealth
	onEvict func(clientChan chan SSEEvent, reason string)
	log     zerolog.Logger
	done    chan struct{}
	mu      sync.RWMutex
}

// NewClientHealthMonitor creates a new client health monitor.
func NewClientHealthMonitor(cfg ClientHealthConfig, log zerolog.Logger) *ClientHealthMonitor {
	return &ClientHealthMonitor{
		config:  cfg,
		clients: make(map[chan SSEEvent]*ClientHealth),
		log:     log.With().Str("component", "client-health").Logger(),
		done:    make(chan struct{}),
	}
}

// Start begins the health monitoring loop.
func (m *ClientHealthMonitor) Start() {
	if !m.config.Enabled {
		return
	}

	go m.monitorLoop()
}

// Stop stops the health monitoring.
func (m *ClientHealthMonitor) Stop() {
	close(m.done)
}

// SetEvictCallback sets the callback for evicting unhealthy clients.
func (m *ClientHealthMonitor) SetEvictCallback(callback func(chan SSEEvent, string)) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.onEvict = callback
}

// RegisterClient registers a new client for monitoring.
func (m *ClientHealthMonitor) RegisterClient(clientChan chan SSEEvent, clientID string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.clients[clientChan] = NewClientHealth(clientID)
	m.log.Debug().Str("client_id", clientID).Msg("Client registered for health monitoring")
}

// UnregisterClient removes a client from monitoring.
func (m *ClientHealthMonitor) UnregisterClient(clientChan chan SSEEvent) {
	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.clients, clientChan)
}

// RecordEvent records a successful event for a client.
func (m *ClientHealthMonitor) RecordEvent(clientChan chan SSEEvent) {
	m.mu.RLock()
	health, ok := m.clients[clientChan]
	m.mu.RUnlock()

	if ok {
		health.RecordEvent()
	}
}

// RecordMissed records a missed event for a client.
func (m *ClientHealthMonitor) RecordMissed(clientChan chan SSEEvent) {
	m.mu.RLock()
	health, ok := m.clients[clientChan]
	m.mu.RUnlock()

	if ok {
		health.RecordMissed()
	}
}

// monitorLoop periodically checks client health.
func (m *ClientHealthMonitor) monitorLoop() {
	ticker := time.NewTicker(m.config.CheckInterval)
	defer ticker.Stop()

	for {
		select {
		case <-m.done:
			return
		case <-ticker.C:
			m.checkClients()
		}
	}
}

// checkClients checks all clients and evicts unhealthy ones.
func (m *ClientHealthMonitor) checkClients() {
	m.mu.Lock()
	defer m.mu.Unlock()

	var toEvict []chan SSEEvent
	var evictReasons []string

	for clientChan, health := range m.clients {
		if !health.IsHealthy(m.config.MaxMissedEvents, m.config.InactivityTimeout) {
			toEvict = append(toEvict, clientChan)

			reason := "unhealthy"
			if health.MissedEvents.Load() > m.config.MaxMissedEvents {
				reason = "too many missed events"
			} else {
				reason = "inactive"
			}
			evictReasons = append(evictReasons, reason)

			m.log.Warn().
				Str("client_id", health.ClientID).
				Int32("missed_events", health.MissedEvents.Load()).
				Str("reason", reason).
				Msg("🚨 Evicting unhealthy client")
		}
	}

	// Evict unhealthy clients
	for i, clientChan := range toEvict {
		delete(m.clients, clientChan)
		if m.onEvict != nil {
			m.onEvict(clientChan, evictReasons[i])
		}
	}
}

// GetClientCount returns the number of monitored clients.
func (m *ClientHealthMonitor) GetClientCount() int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return len(m.clients)
}

// GetAllStats returns health stats for all clients.
func (m *ClientHealthMonitor) GetAllStats() []map[string]any {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var stats []map[string]any
	for _, health := range m.clients {
		stats = append(stats, health.GetStats())
	}
	return stats
}

// GetConfig returns the current configuration.
func (m *ClientHealthMonitor) GetConfig() ClientHealthConfig {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.config
}

// SetConfig updates the configuration.
func (m *ClientHealthMonitor) SetConfig(cfg ClientHealthConfig) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.config = cfg
}
