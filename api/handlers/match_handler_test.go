package handlers

import (
	"net/http"
	"testing"

	"github.com/rs/zerolog"

	"pharmabroker/api/sse"
	"pharmabroker/domain/entity"
)

func TestMatchHandler_GetMatches(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	matchRepo := &mockMatchRepo{matches: []*entity.MatchWithDetails{
		{Match: entity.Match{ID: "match-1", OfferID: "offer-1", RequestID: "req-1", Score: 0.9, Status: entity.MatchStatusPending}},
	}}
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(matchRepo, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	c, w := th.CreateContext("GET", "/api/matches", nil)

	h.GetMatchesGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestMatchHandler_ConfirmMatch_MissingID(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(&mockMatchRepo{}, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	c, w := th.CreateContext("POST", "/api/matches//confirm", map[string]interface{}{})
	// No params set - ID will be empty

	h.ConfirmMatchGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}

func TestMatchHandler_RejectMatch_MissingID(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	sseHub := sse.NewSSEHub()
	h := NewMatchHandler(&mockMatchRepo{}, &mockOfferRepo{}, &mockRequestRepo{}, nil, sseHub, log)

	c, w := th.CreateContext("POST", "/api/matches//reject", map[string]interface{}{})
	// No params set - ID will be empty

	h.RejectMatchGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}
