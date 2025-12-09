package handlers

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestOfferHandler_GetOffers(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockOfferRepo{offers: []*entity.Offer{
		{ID: "offer-1", Medication: "Paracetamol", Quantity: 100, Status: entity.StatusActive},
		{ID: "offer-2", Medication: "Ibuprofen", Quantity: 50, Status: entity.StatusActive},
	}}
	h := NewOfferHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/offers", nil)
	w := httptest.NewRecorder()

	h.GetOffers(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	var resp struct {
		Data []*entity.Offer `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if len(resp.Data) != 2 {
		t.Errorf("Expected 2 offers, got %d", len(resp.Data))
	}
}

func TestOfferHandler_GetOffer(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockOfferRepo{offers: []*entity.Offer{
		{ID: "offer-1", Medication: "Paracetamol", Quantity: 100, Status: entity.StatusActive},
	}}
	h := NewOfferHandler(repo, log)

	// Note: PathValue requires Go 1.22+ routing with path patterns
	// For testing, we can create a request with PathValue set manually
	req := httptest.NewRequest(http.MethodGet, "/api/offers/offer-1", nil)
	req.SetPathValue("id", "offer-1")
	w := httptest.NewRecorder()

	h.GetOffer(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestOfferHandler_GetOffer_MissingID(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockOfferRepo{}
	h := NewOfferHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/offers/", nil)
	// ID not set, so PathValue("id") returns ""
	w := httptest.NewRecorder()

	h.GetOffer(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Expected status 400, got %d", w.Code)
	}
}
