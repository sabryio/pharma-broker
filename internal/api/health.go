package api

import (
	"context"
	"encoding/json"
	"net/http"
	"sync"
	"time"
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

// HealthResponse represents the overall health check response
type HealthResponse struct {
	Status     HealthStatus               `json:"status"`
	Timestamp  string                     `json:"timestamp"`
	Components map[string]ComponentHealth `json:"components"`
}

// HealthChecker provides health check functionality
type HealthChecker struct {
	dbPingFunc      func(ctx context.Context) error
	waConnectedFunc func() bool
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

// LiveHandler returns a simple liveness probe (is the process running?)
// GET /health/live
func (h *HealthChecker) LiveHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]interface{}{
		"status":    "OK",
		"timestamp": time.Now().Format(time.RFC3339),
	})
}

// ReadyHandler returns readiness probe (is the app ready to serve traffic?)
// GET /health/ready
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

	// Check WhatsApp
	if waConnected != nil {
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

	// Set HTTP status based on overall health
	statusCode := http.StatusOK
	if response.Status == HealthDown {
		statusCode = http.StatusServiceUnavailable
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(response)
}

// FullHealthHandler returns detailed health with all metrics
// GET /health
func (h *HealthChecker) FullHealthHandler(w http.ResponseWriter, r *http.Request) {
	h.ReadyHandler(w, r)
}
