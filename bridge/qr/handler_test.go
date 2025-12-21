package qr

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

func newTestHandler() *Handler {
	logger := zerolog.Nop()
	return New(DefaultConfig(), logger)
}

func TestNewHandler(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	if h.state != StateWaiting {
		t.Errorf("expected initial state %s, got %s", StateWaiting, h.state)
	}

	if h.IsPaired() {
		t.Error("expected IsPaired() to be false initially")
	}
}

func TestHandleQRCode(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	cfg := Config{RenderTerminal: false, QRTimeout: 60 * time.Second}
	h.HandleQRCode("test-qr-code", cfg)

	state := h.GetState()
	if state.State != StateReady {
		t.Errorf("expected state %s, got %s", StateReady, state.State)
	}
	if state.Code != "test-qr-code" {
		t.Errorf("expected code 'test-qr-code', got '%s'", state.Code)
	}
	if state.ExpiresAt.IsZero() {
		t.Error("expected ExpiresAt to be set")
	}
}

func TestHandleEventSuccess(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	cfg := DefaultConfig()
	h.HandleEvent("success", cfg)

	if !h.IsPaired() {
		t.Error("expected IsPaired() to be true after success")
	}

	state := h.GetState()
	if state.State != StatePaired {
		t.Errorf("expected state %s, got %s", StatePaired, state.State)
	}
}

func TestHandleEventTimeout(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	cfg := DefaultConfig()
	h.HandleEvent("timeout", cfg)

	state := h.GetState()
	if state.State != StateTimeout {
		t.Errorf("expected state %s, got %s", StateTimeout, state.State)
	}
}

func TestHandleError(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	h.HandleError(http.ErrServerClosed)

	state := h.GetState()
	if state.State != StateError {
		t.Errorf("expected state %s, got %s", StateError, state.State)
	}
	if state.Error == "" {
		t.Error("expected error message to be set")
	}
}

func TestSetPaired(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	h.SetPaired()

	if !h.IsPaired() {
		t.Error("expected IsPaired() to be true after SetPaired()")
	}
}

func TestHTTPHandler_Waiting(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	req := httptest.NewRequest("GET", "/qr/json", nil)
	w := httptest.NewRecorder()

	h.HTTPHandler(w, req)

	if w.Code != http.StatusAccepted {
		t.Errorf("expected status %d, got %d", http.StatusAccepted, w.Code)
	}

	var state QRUpdate
	json.NewDecoder(w.Body).Decode(&state)
	if state.State != StateWaiting {
		t.Errorf("expected state %s, got %s", StateWaiting, state.State)
	}
}

func TestHTTPHandler_Ready(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	cfg := Config{RenderTerminal: false, QRTimeout: 60 * time.Second}
	h.HandleQRCode("test-code", cfg)

	req := httptest.NewRequest("GET", "/qr/json", nil)
	w := httptest.NewRecorder()

	h.HTTPHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, w.Code)
	}

	var state QRUpdate
	json.NewDecoder(w.Body).Decode(&state)
	if state.State != StateReady {
		t.Errorf("expected state %s, got %s", StateReady, state.State)
	}
	if state.Code != "test-code" {
		t.Errorf("expected code 'test-code', got '%s'", state.Code)
	}
}

func TestHTTPHandler_Paired(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	h.SetPaired()

	req := httptest.NewRequest("GET", "/qr/json", nil)
	w := httptest.NewRecorder()

	h.HTTPHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, w.Code)
	}

	var state QRUpdate
	json.NewDecoder(w.Body).Decode(&state)
	if state.State != StatePaired {
		t.Errorf("expected state %s, got %s", StatePaired, state.State)
	}
}

func TestHTTPHandler_Timeout(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	h.HandleEvent("timeout", DefaultConfig())

	req := httptest.NewRequest("GET", "/qr/json", nil)
	w := httptest.NewRecorder()

	h.HTTPHandler(w, req)

	if w.Code != http.StatusGone {
		t.Errorf("expected status %d, got %d", http.StatusGone, w.Code)
	}
}

func TestHTTPHandler_Error(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	h.HandleError(http.ErrServerClosed)

	req := httptest.NewRequest("GET", "/qr/json", nil)
	w := httptest.NewRecorder()

	h.HTTPHandler(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Errorf("expected status %d, got %d", http.StatusInternalServerError, w.Code)
	}
}

func TestHTMLHandler(t *testing.T) {
	h := newTestHandler()
	defer h.Close()

	req := httptest.NewRequest("GET", "/qr", nil)
	w := httptest.NewRecorder()

	h.HTMLHandler(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, w.Code)
	}

	contentType := w.Header().Get("Content-Type")
	if contentType != "text/html; charset=utf-8" {
		t.Errorf("expected Content-Type 'text/html; charset=utf-8', got '%s'", contentType)
	}

	body := w.Body.String()
	if len(body) < 100 {
		t.Error("expected HTML body to be non-empty")
	}
}

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()

	if !cfg.RenderTerminal {
		t.Error("expected RenderTerminal to be true by default")
	}
	if cfg.QRTimeout != 60*time.Second {
		t.Errorf("expected QRTimeout to be 60s, got %v", cfg.QRTimeout)
	}
}
