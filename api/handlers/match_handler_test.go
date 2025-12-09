package handlers

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/api/sse"
	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestMatchHandler_GetMatches(t *testing.T) {
	log := zerolog.Nop()
	matchRepo := &mockMatchRepo{matches: []*entity.MatchWithDetails{
		{Match: entity.Match{ID: "match-1", OfferID: "offer-1", RequestID: "req-1", Score: 0.9, Status: entity.MatchStatusPending}},
	}}
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(matchRepo, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	req := httptest.NewRequest(http.MethodGet, "/api/matches", nil)
	w := httptest.NewRecorder()

	h.GetMatches(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestMatchHandler_ConfirmMatch_MissingID(t *testing.T) {
	log := zerolog.Nop()
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(&mockMatchRepo{}, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	req := httptest.NewRequest(http.MethodPost, "/api/matches//confirm", bytes.NewReader([]byte("{}")))
	w := httptest.NewRecorder()

	h.ConfirmMatch(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}

func TestMatchHandler_RejectMatch_MissingID(t *testing.T) {
	log := zerolog.Nop()
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(&mockMatchRepo{}, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	req := httptest.NewRequest(http.MethodPost, "/api/matches//reject", bytes.NewReader([]byte("{}")))
	w := httptest.NewRecorder()

	h.RejectMatch(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}
