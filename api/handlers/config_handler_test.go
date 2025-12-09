package handlers

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/rs/zerolog"
)

func TestConfigHandler_GetConfig(t *testing.T) {
	log := zerolog.Nop()
	h := NewConfigHandler(&mockConfigRepo{}, log)

	req := httptest.NewRequest(http.MethodGet, "/api/config", nil)
	w := httptest.NewRecorder()

	h.GetConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}
