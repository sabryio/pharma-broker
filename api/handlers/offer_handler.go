package handlers

import (
	"context"
	"net/http"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"

	"github.com/rs/zerolog"
)

// OfferHandler handles offer-related requests
type OfferHandler struct {
	repo repository.OfferRepository
	log  zerolog.Logger
}

// NewOfferHandler creates a new OfferHandler
func NewOfferHandler(repo repository.OfferRepository, log zerolog.Logger) *OfferHandler {
	return &OfferHandler{
		repo: repo,
		log:  log.With().Str("component", "OfferHandler").Logger(),
	}
}

// GetOffers returns active offers with pagination
func (h *OfferHandler) GetOffers(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, offset := getPagination(r)
	query := r.URL.Query().Get("q")

	var offers []*entity.Offer
	var err error

	if query != "" {
		offers, err = h.repo.Search(ctx, query, limit, offset)
	} else {
		offers, err = h.repo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get offers")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch offers"))
		return
	}

	total, _ := h.repo.CountActive(ctx)
	successWithMeta(w, offers, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetOffer returns a single offer by ID
func (h *OfferHandler) GetOffer(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing offer ID"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	offer, err := h.repo.GetByID(ctx, id)
	if err != nil {
		errorWithCode(w, http.StatusNotFound, ErrOfferNotFound())
		return
	}

	success(w, offer)
}
