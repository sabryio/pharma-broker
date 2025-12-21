package qr

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"
)

func init() {
	gin.SetMode(gin.TestMode)
}

func testConfig() Config {
	return Config{
		RenderTerminal: false,
		QRTimeout:      60 * time.Second,
		MaxRetries:     5,
	}
}

func TestNewHandler(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	if h == nil {
		t.Fatal("Expected handler to be created")
	}
	if h.state != StateWaiting {
		t.Errorf("Expected state %s, got %s", StateWaiting, h.state)
	}
}

func TestHandleQRCode(t *testing.T) {
	cfg := Config{RenderTerminal: false, QRTimeout: time.Minute, MaxRetries: 5}
	h := New(cfg, zerolog.Nop())
	defer h.Close()

	h.HandleQRCode("test-qr-code", cfg)

	state := h.GetState()
	if state.State != StateReady {
		t.Errorf("Expected state %s, got %s", StateReady, state.State)
	}
	if state.Code != "test-qr-code" {
		t.Errorf("Expected code 'test-qr-code', got '%s'", state.Code)
	}
	if state.Attempt != 1 {
		t.Errorf("Expected attempt 1, got %d", state.Attempt)
	}
}

func TestHandleEventSuccess(t *testing.T) {
	cfg := testConfig()
	h := New(cfg, zerolog.Nop())
	defer h.Close()

	h.HandleEvent("success", cfg)

	if !h.IsPaired() {
		t.Error("Expected handler to be paired")
	}
}

func TestHandleEventTimeout(t *testing.T) {
	cfg := testConfig()
	h := New(cfg, zerolog.Nop())
	defer h.Close()

	h.HandleEvent("timeout", cfg)

	state := h.GetState()
	if state.State != StateTimeout {
		t.Errorf("Expected state %s, got %s", StateTimeout, state.State)
	}
}

func TestHandleError(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	h.HandleError(http.ErrServerClosed)

	state := h.GetState()
	if state.State != StateError {
		t.Errorf("Expected state %s, got %s", StateError, state.State)
	}
	if state.Error == "" {
		t.Error("Expected error message")
	}
}

func TestSetPaired(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	h.SetPaired()

	if !h.IsPaired() {
		t.Error("Expected handler to be paired")
	}
}

func TestJSONHandler_Waiting(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr/json", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusAccepted {
		t.Errorf("Expected status %d, got %d", http.StatusAccepted, w.Code)
	}

	var resp Update
	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp.State != StateWaiting {
		t.Errorf("Expected state %s, got %s", StateWaiting, resp.State)
	}
}

func TestJSONHandler_Ready(t *testing.T) {
	cfg := Config{RenderTerminal: false, QRTimeout: time.Minute}
	h := New(cfg, zerolog.Nop())
	defer h.Close()

	h.HandleQRCode("test-code", cfg)

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr/json", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status %d, got %d", http.StatusOK, w.Code)
	}

	var resp Update
	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp.State != StateReady {
		t.Errorf("Expected state %s, got %s", StateReady, resp.State)
	}
}

func TestJSONHandler_Paired(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	h.SetPaired()

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr/json", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status %d, got %d", http.StatusOK, w.Code)
	}
}

func TestJSONHandler_Timeout(t *testing.T) {
	cfg := testConfig()
	h := New(cfg, zerolog.Nop())
	defer h.Close()

	h.HandleEvent("timeout", cfg)

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr/json", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusGone {
		t.Errorf("Expected status %d, got %d", http.StatusGone, w.Code)
	}
}

func TestJSONHandler_Error(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	h.HandleError(http.ErrServerClosed)

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr/json", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusInternalServerError {
		t.Errorf("Expected status %d, got %d", http.StatusInternalServerError, w.Code)
	}
}

func TestHTMLHandler(t *testing.T) {
	h := New(testConfig(), zerolog.Nop())
	defer h.Close()

	router := gin.New()
	h.RegisterRoutes(router.Group("/qr"))

	w := httptest.NewRecorder()
	req, _ := http.NewRequest("GET", "/qr", nil)
	router.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status %d, got %d", http.StatusOK, w.Code)
	}

	contentType := w.Header().Get("Content-Type")
	if contentType != "text/html; charset=utf-8" {
		t.Errorf("Expected content type 'text/html; charset=utf-8', got '%s'", contentType)
	}
}

func TestConfigValues(t *testing.T) {
	cfg := testConfig()

	if cfg.RenderTerminal {
		t.Error("Expected RenderTerminal to be false in test config")
	}
	if cfg.QRTimeout != 60*time.Second {
		t.Errorf("Expected QRTimeout 60s, got %v", cfg.QRTimeout)
	}
	if cfg.MaxRetries != 5 {
		t.Errorf("Expected MaxRetries 5, got %d", cfg.MaxRetries)
	}
}
