package whatsapp

import (
	"time"
)

// HealthStatus represents the overall health of the WhatsApp ingestion system.
type HealthStatus struct {
	// Connection health
	Connection ConnectionHealth `json:"connection"`
	// Queue health
	Queue QueueHealth `json:"queue,omitempty"`
	// Overall status
	Status    string    `json:"status"`
	Timestamp time.Time `json:"timestamp"`
}

// ConnectionHealth represents WhatsApp connection status.
type ConnectionHealth struct {
	State           string        `json:"state"`
	Connected       bool          `json:"connected"`
	Uptime          time.Duration `json:"uptime"`
	ReconnectCount  int           `json:"reconnect_count"`
	LastConnectedAt time.Time     `json:"last_connected_at,omitempty"`
}

// HealthChecker provides health check capabilities.
type HealthChecker interface {
	HealthStatus() HealthStatus
}

// GetConnectionHealth returns current connection health from Manager.
func (m *Manager) GetConnectionHealth() ConnectionHealth {
	state := m.State()
	lastConnected := time.Unix(m.lastConnectedAt.Load(), 0)

	var uptime time.Duration
	if state == StateConnected && !lastConnected.IsZero() {
		uptime = time.Since(lastConnected)
	}

	return ConnectionHealth{
		State:           state.String(),
		Connected:       state == StateConnected,
		Uptime:          uptime,
		ReconnectCount:  int(m.reconnectCount.Load()),
		LastConnectedAt: lastConnected,
	}
}

// HealthStatus returns the overall health status of the WhatsApp system.
func (m *Manager) HealthStatus() HealthStatus {
	connHealth := m.GetConnectionHealth()

	status := "healthy"
	if !connHealth.Connected {
		switch m.State() {
		case StateReconnecting:
			status = "degraded"
		case StateFailed:
			status = "unhealthy"
		default:
			status = "disconnected"
		}
	}

	return HealthStatus{
		Connection: connHealth,
		Status:     status,
		Timestamp:  time.Now(),
	}
}

// HealthStatusWithQueue returns health status including queue health.
func (m *Manager) HealthStatusWithQueue(q *Queue) HealthStatus {
	health := m.HealthStatus()

	if q != nil {
		health.Queue = q.HealthStatus()

		// Adjust overall status based on queue health
		if health.Status == "healthy" {
			switch health.Queue.Status {
			case "degraded":
				health.Status = "degraded"
			case "warning":
				health.Status = "warning"
			}
		}
	}

	return health
}

// IsHealthy returns true if the system is operational.
func (h HealthStatus) IsHealthy() bool {
	return h.Status == "healthy" || h.Status == "warning"
}

// IsDegraded returns true if the system is running but with issues.
func (h HealthStatus) IsDegraded() bool {
	return h.Status == "degraded"
}

// IsUnhealthy returns true if the system is not operational.
func (h HealthStatus) IsUnhealthy() bool {
	return h.Status == "unhealthy" || h.Status == "disconnected"
}
