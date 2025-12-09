package handlers

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestRequestHandler_GetRequests(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockRequestRepo{requests: []*entity.Request{
		{ID: "req-1", Medication: "Paracetamol", Quantity: 50, Status: entity.StatusActive},
	}}
	h := NewRequestHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/requests", nil)
	w := httptest.NewRecorder()

	h.GetRequests(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestRequestHandler_GetRequest_MissingID(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockRequestRepo{}
	h := NewRequestHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/requests/", nil)
	w := httptest.NewRecorder()

	h.GetRequest(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}
