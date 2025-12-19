package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
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

// GetOffersGin returns active offers with pagination
func (h *OfferHandler) GetOffersGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, offset := GetPaginationGin(c)
	query := c.Query("q")

	var offers []*entity.Offer
	var err error

	if query != "" {
		offers, err = h.repo.Search(ctx, query, limit, offset)
	} else {
		offers, err = h.repo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get offers")
		DatabaseErrorGin(c, "Failed to fetch offers")
		return
	}

	total, _ := h.repo.CountActive(ctx)
	SuccessWithMetaGin(c, offers, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetOfferGin returns a single offer by ID
func (h *OfferHandler) GetOfferGin(c *gin.Context) {
	id, ok := ValidateID(c, "id")
	if !ok {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	offer, err := h.repo.GetByID(ctx, id)
	if err != nil {
		NotFoundGin(c, ErrOfferNotFound())
		return
	}

	SuccessGin(c, offer)
}
