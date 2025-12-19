package handlers

import (
	"net/http"
	"testing"

	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
)

func TestRequestHandler_GetRequests(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockRequestRepo{requests: []*entity.Request{
		{ID: "req-1", Medication: "Paracetamol", Quantity: 50, Status: entity.StatusActive},
	}}
	h := NewRequestHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/requests", nil)

	h.GetRequestsGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestRequestHandler_GetRequest_MissingID(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockRequestRepo{}
	h := NewRequestHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/requests/", nil)
	// No params set - ID will be empty

	h.GetRequestGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}
