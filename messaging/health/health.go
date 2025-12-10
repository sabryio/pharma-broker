package health

import (
	"time"

	"pharmabroker/messaging/queue"
	"pharmabroker/messaging/whatsapp"
)

// HealthStatus represents the overall health of the WhatsApp ingestion system.
type HealthStatus struct {
	// Connection health
	Connection ConnectionHealth `json:"connection"`
	// Queue health
	Queue queue.QueueHealth `json:"queue,omitempty"`
	// Overall status
	Status    queue.QueueHealthStatus `json:"status"`
	Timestamp time.Time               `json:"timestamp"`
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
func GetConnectionHealth(m *whatsapp.Manager) ConnectionHealth {
	status := m.GetConnectionStatus()

	var uptime time.Duration
	if m.State() == whatsapp.StateConnected && !status.LastConnectedAt.IsZero() {
		uptime = time.Since(status.LastConnectedAt)
	}

	return ConnectionHealth{
		State:           m.State().String(),
		Connected:       m.State() == whatsapp.StateConnected,
		Uptime:          uptime,
		ReconnectCount:  status.ReconnectCount,
		LastConnectedAt: status.LastConnectedAt,
	}
}

// GetHealthStatus returns the overall health status of the WhatsApp system.
func GetHealthStatus(m *whatsapp.Manager) HealthStatus {
	connHealth := GetConnectionHealth(m)

	status := queue.QueueHealthStatusHealthy
	if !connHealth.Connected {
		switch m.State() {
		case whatsapp.StateReconnecting:
			status = queue.QueueHealthStatusDegraded
		case whatsapp.StateFailed:
			status = queue.QueueHealthStatusUnhealthy
		default:
			status = queue.QueueHealthStatusDisconnected
		}
	}

	return HealthStatus{
		Connection: connHealth,
		Status:     status,
		Timestamp:  time.Now(),
	}
}

// IsHealthy returns true if the system is operational.
func (h HealthStatus) IsHealthy() bool {
	return h.Status == queue.QueueHealthStatusHealthy || h.Status == queue.QueueHealthStatusWarning
}

// IsDegraded returns true if the system is running but with issues.
func (h HealthStatus) IsDegraded() bool {
	return h.Status == queue.QueueHealthStatusDegraded
}

// IsUnhealthy returns true if the system is not operational.
func (h HealthStatus) IsUnhealthy() bool {
	return h.Status == queue.QueueHealthStatusUnhealthy || h.Status == queue.QueueHealthStatusDisconnected
}

