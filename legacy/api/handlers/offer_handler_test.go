package handlers

import (
	"net/http"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
)

func TestOfferHandler_GetOffers(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockOfferRepo{offers: []*entity.Offer{
		{ID: "offer-1", Medication: "Paracetamol", Quantity: 100, Status: entity.StatusActive},
		{ID: "offer-2", Medication: "Ibuprofen", Quantity: 50, Status: entity.StatusActive},
	}}
	h := NewOfferHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/offers", nil)

	h.GetOffersGin(c)

	th.AssertStatus(w, http.StatusOK)

	var resp struct {
		Data []*entity.Offer `json:"data"`
	}
	th.AssertJSONResponse(w, &resp)

	if len(resp.Data) != 2 {
		t.Errorf("Expected 2 offers, got %d", len(resp.Data))
	}
}

func TestOfferHandler_GetOffer(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockOfferRepo{offers: []*entity.Offer{
		{ID: "offer-1", Medication: "Paracetamol", Quantity: 100, Status: entity.StatusActive},
	}}
	h := NewOfferHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/offers/offer-1", nil)
	c.Params = gin.Params{{Key: "id", Value: "offer-1"}}

	h.GetOfferGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestOfferHandler_GetOffer_MissingID(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockOfferRepo{}
	h := NewOfferHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/offers/", nil)
	// No params set - ID will be empty

	h.GetOfferGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}
