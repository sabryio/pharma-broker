package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"sync"
	"time"

	"github.com/gin-gonic/gin"
)

// HealthStatus represents the health of a component
type HealthStatus string

const (
	HealthOK       HealthStatus = "OK"
	HealthDegraded HealthStatus = "DEGRADED"
	HealthDown     HealthStatus = "DOWN"
)

// ComponentHealth represents health of a single component
type ComponentHealth struct {
	Status  HealthStatus `json:"status"`
	Message string       `json:"message,omitempty"`
	Latency string       `json:"latency,omitempty"`
}

// WhatsAppHealth represents detailed WhatsApp connection health
type WhatsAppHealth struct {
	Status          HealthStatus `json:"status"`
	State           string       `json:"state"`
	ReconnectCount  int          `json:"reconnect_count,omitempty"`
	LastConnectedAt string       `json:"last_connected_at,omitempty"`
	UptimeSeconds   int64        `json:"uptime_seconds,omitempty"`
	Message         string       `json:"message,omitempty"`
}

// HealthResponse represents the overall health check response
type HealthResponse struct {
	Status     HealthStatus               `json:"status"`
	Timestamp  string                     `json:"timestamp"`
	Components map[string]ComponentHealth `json:"components"`
	WhatsApp   *WhatsAppHealth            `json:"whatsapp,omitempty"`
}

// WAStatusProvider provides detailed WhatsApp connection status
type WAStatusProvider interface {
	IsConnected() bool
	State() interface{ String() string }
	GetConnectionStatus() interface {
		ReconnectCount() int
		LastConnectedAt() time.Time
		UptimeSeconds() int64
	}
}

// HealthChecker provides health check functionality
type HealthChecker struct {
	dbPingFunc      func(ctx context.Context) error
	waConnectedFunc func() bool
	waStatusFunc    func() (state string, reconnectCount int, lastConnected time.Time, uptimeSeconds int64)
	aiHealthFunc    func(ctx context.Context) error
	mu              sync.RWMutex
}

// NewHealthChecker creates a new health checker
func NewHealthChecker() *HealthChecker {
	return &HealthChecker{}
}

// SetDBPingFunc sets the database ping function
func (h *HealthChecker) SetDBPingFunc(fn func(ctx context.Context) error) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.dbPingFunc = fn
}

// SetWAConnectedFunc sets the WhatsApp connection check function
func (h *HealthChecker) SetWAConnectedFunc(fn func() bool) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.waConnectedFunc = fn
}

// SetAIHealthFunc sets the AI provider health check function
func (h *HealthChecker) SetAIHealthFunc(fn func(ctx context.Context) error) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.aiHealthFunc = fn
}

// SetWAStatusFunc sets the WhatsApp detailed status function
func (h *HealthChecker) SetWAStatusFunc(fn func() (state string, reconnectCount int, lastConnected time.Time, uptimeSeconds int64)) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.waStatusFunc = fn
}

// LiveHandler returns a simple liveness probe (is the process running?)
func (h *HealthChecker) LiveHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":    "OK",
		"timestamp": time.Now().Format(time.RFC3339),
	})
}

// ReadyHandler returns readiness probe (is the app ready to serve traffic?)
func (h *HealthChecker) ReadyHandler(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	h.mu.RLock()
	dbPing := h.dbPingFunc
	waConnected := h.waConnectedFunc
	aiHealth := h.aiHealthFunc
	h.mu.RUnlock()

	response := HealthResponse{
		Status:     HealthOK,
		Timestamp:  time.Now().Format(time.RFC3339),
		Components: make(map[string]ComponentHealth),
	}

	// Check Database
	if dbPing != nil {
		start := time.Now()
		if err := dbPing(ctx); err != nil {
			response.Components["database"] = ComponentHealth{
				Status:  HealthDown,
				Message: err.Error(),
			}
			response.Status = HealthDown
		} else {
			response.Components["database"] = ComponentHealth{
				Status:  HealthOK,
				Latency: time.Since(start).String(),
			}
		}
	} else {
		response.Components["database"] = ComponentHealth{
			Status:  HealthDegraded,
			Message: "health check not configured",
		}
	}

	// Check WhatsApp (detailed)
	h.mu.RLock()
	waStatus := h.waStatusFunc
	h.mu.RUnlock()

	if waStatus != nil {
		state, reconnectCount, lastConnected, uptimeSeconds := waStatus()
		isConnected := state == "CONNECTED"

		waHealth := &WhatsAppHealth{
			State:          state,
			ReconnectCount: reconnectCount,
			UptimeSeconds:  uptimeSeconds,
		}

		if !lastConnected.IsZero() {
			waHealth.LastConnectedAt = lastConnected.Format(time.RFC3339)
		}

		if isConnected {
			waHealth.Status = HealthOK
			response.Components["whatsapp"] = ComponentHealth{Status: HealthOK}
		} else {
			waHealth.Status = HealthDegraded
			waHealth.Message = "state: " + state
			response.Components["whatsapp"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: state,
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		}

		response.WhatsApp = waHealth
	} else if waConnected != nil {
		// Fallback to simple check
		if waConnected() {
			response.Components["whatsapp"] = ComponentHealth{
				Status: HealthOK,
			}
		} else {
			response.Components["whatsapp"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: "not connected",
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		}
	} else {
		response.Components["whatsapp"] = ComponentHealth{
			Status:  HealthDegraded,
			Message: "health check not configured",
		}
	}

	// Check AI Provider
	if aiHealth != nil {
		start := time.Now()
		if err := aiHealth(ctx); err != nil {
			response.Components["ai"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: err.Error(),
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		} else {
			response.Components["ai"] = ComponentHealth{
				Status:  HealthOK,
				Latency: time.Since(start).String(),
			}
		}
	} else {
		response.Components["ai"] = ComponentHealth{
			Status:  HealthOK,
			Message: "always available (Docker)",
		}
	}

	statusCode := http.StatusOK
	if response.Status == HealthDown {
		statusCode = http.StatusServiceUnavailable
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(response)
}

// FullHealthHandler returns detailed health with all metrics
func (h *HealthChecker) FullHealthHandler(w http.ResponseWriter, r *http.Request) {
	h.ReadyHandler(w, r)
}

// ============================================================================
// Gin Handlers
// ============================================================================

// LiveGin returns a simple liveness probe (Gin)
func (h *HealthChecker) LiveGin(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"status":    "OK",
		"timestamp": time.Now().Format(time.RFC3339),
	})
}

// ReadyGin returns readiness probe (Gin)
func (h *HealthChecker) ReadyGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	h.mu.RLock()
	dbPing := h.dbPingFunc
	waConnected := h.waConnectedFunc
	aiHealth := h.aiHealthFunc
	waStatus := h.waStatusFunc
	h.mu.RUnlock()

	response := HealthResponse{
		Status:     HealthOK,
		Timestamp:  time.Now().Format(time.RFC3339),
		Components: make(map[string]ComponentHealth),
	}

	// Check Database
	if dbPing != nil {
		start := time.Now()
		if err := dbPing(ctx); err != nil {
			response.Components["database"] = ComponentHealth{
				Status:  HealthDown,
				Message: err.Error(),
			}
			response.Status = HealthDown
		} else {
			response.Components["database"] = ComponentHealth{
				Status:  HealthOK,
				Latency: time.Since(start).String(),
			}
		}
	} else {
		response.Components["database"] = ComponentHealth{
			Status:  HealthDegraded,
			Message: "health check not configured",
		}
	}

	// Check WhatsApp
	if waStatus != nil {
		state, reconnectCount, lastConnected, uptimeSeconds := waStatus()
		isConnected := state == "CONNECTED"

		waHealth := &WhatsAppHealth{
			State:          state,
			ReconnectCount: reconnectCount,
			UptimeSeconds:  uptimeSeconds,
		}

		if !lastConnected.IsZero() {
			waHealth.LastConnectedAt = lastConnected.Format(time.RFC3339)
		}

		if isConnected {
			waHealth.Status = HealthOK
			response.Components["whatsapp"] = ComponentHealth{Status: HealthOK}
		} else {
			waHealth.Status = HealthDegraded
			waHealth.Message = "state: " + state
			response.Components["whatsapp"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: state,
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		}
		response.WhatsApp = waHealth
	} else if waConnected != nil {
		if waConnected() {
			response.Components["whatsapp"] = ComponentHealth{Status: HealthOK}
		} else {
			response.Components["whatsapp"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: "not connected",
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		}
	} else {
		response.Components["whatsapp"] = ComponentHealth{
			Status:  HealthDegraded,
			Message: "health check not configured",
		}
	}

	// Check AI Provider
	if aiHealth != nil {
		start := time.Now()
		if err := aiHealth(ctx); err != nil {
			response.Components["ai"] = ComponentHealth{
				Status:  HealthDegraded,
				Message: err.Error(),
			}
			if response.Status == HealthOK {
				response.Status = HealthDegraded
			}
		} else {
			response.Components["ai"] = ComponentHealth{
				Status:  HealthOK,
				Latency: time.Since(start).String(),
			}
		}
	} else {
		response.Components["ai"] = ComponentHealth{
			Status:  HealthOK,
			Message: "always available (Docker)",
		}
	}

	statusCode := http.StatusOK
	if response.Status == HealthDown {
		statusCode = http.StatusServiceUnavailable
	}

	c.JSON(statusCode, response)
}

// FullHealthGin returns detailed health with all metrics (Gin)
func (h *HealthChecker) FullHealthGin(c *gin.Context) {
	h.ReadyGin(c)
}
