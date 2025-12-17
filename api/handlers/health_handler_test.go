package handlers

import (
	"context"
	"net/http"
	"testing"
)

func TestHealthReady(t *testing.T) {
	th := NewTestHelper(t)
	hc := NewHealthChecker()

	// Configure DB ping that succeeds
	hc.SetDBPingFunc(func(ctx context.Context) error {
		return nil
	})

	c, w := th.CreateContext("GET", "/health/ready", nil)

	hc.ReadyGin(c)

	th.AssertStatus(w, http.StatusOK)

	var resp HealthResponse
	th.AssertJSONResponse(w, &resp)

	if resp.Status != HealthOK {
		t.Errorf("Expected status OK, got %v", resp.Status)
	}

	if resp.Components["database"].Status != HealthOK {
		t.Errorf("Expected database status OK, got %v", resp.Components["database"].Status)
	}
}

func TestHealthReady_WithWhatsAppConnected(t *testing.T) {
	th := NewTestHelper(t)
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error { return nil })
	hc.SetWAConnectedFunc(func() bool { return true })

	c, w := th.CreateContext("GET", "/health/ready", nil)

	hc.ReadyGin(c)

	th.AssertStatus(w, http.StatusOK)

	var resp HealthResponse
	th.AssertJSONResponse(w, &resp)

	if resp.Components["whatsapp"].Status != HealthOK {
		t.Errorf("Expected whatsapp status OK, got %v", resp.Components["whatsapp"].Status)
	}
}

func TestHealthReady_DatabaseDown(t *testing.T) {
	th := NewTestHelper(t)
	hc := NewHealthChecker()

	hc.SetDBPingFunc(func(ctx context.Context) error {
		return context.DeadlineExceeded
	})

	c, w := th.CreateContext("GET", "/health/ready", nil)

	hc.ReadyGin(c)

	th.AssertStatus(w, http.StatusServiceUnavailable)
}
