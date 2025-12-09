package handlers

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestStatsHandler_GetStats(t *testing.T) {
	log := zerolog.Nop()
	h := NewStatsHandler(&mockStatsRepo{}, log)

	req := httptest.NewRequest(http.MethodGet, "/api/stats", nil)
	w := httptest.NewRecorder()

	h.GetStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp struct {
		Data *entity.Stats `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp.Data.ActiveOffers != 10 {
		t.Errorf("Expected 10 offers, got %d", resp.Data.ActiveOffers)
	}
}
