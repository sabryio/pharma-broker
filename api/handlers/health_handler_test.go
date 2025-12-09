package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestHealthReady(t *testing.T) {
	hc := NewHealthChecker()

	// Configure DB ping that succeeds
	hc.SetDBPingFunc(func(ctx context.Context) error {
		return nil
	})

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp HealthResponse
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp.Status != HealthOK {
		t.Errorf("Expected status OK, got %v", resp.Status)
	}

	if resp.Components["database"].Status != HealthOK {
		t.Errorf("Expected database status OK, got %v", resp.Components["database"].Status)
	}
}

func TestHealthReady_WithWhatsAppConnected(t *testing.T) {
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error { return nil })
	hc.SetWAConnectedFunc(func() bool { return true })

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp HealthResponse
	json.NewDecoder(w.Body).Decode(&resp)

	if resp.Components["whatsapp"].Status != HealthOK {
		t.Errorf("Expected whatsapp status OK, got %v", resp.Components["whatsapp"].Status)
	}
}

func TestHealthReady_DatabaseDown(t *testing.T) {
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error {
		return context.DeadlineExceeded
	})

	req := httptest.NewRequest(http.MethodGet, "/health/ready", nil)
	w := httptest.NewRecorder()

	hc.ReadyHandler(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Expected status 503, got %d", w.Code)
	}
}
