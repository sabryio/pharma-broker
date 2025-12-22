package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// RequestHandler handles request-related operations
type RequestHandler struct {
	repo repository.RequestRepository
	log  zerolog.Logger
}

// NewRequestHandler creates a new RequestHandler
func NewRequestHandler(repo repository.RequestRepository, log zerolog.Logger) *RequestHandler {
	return &RequestHandler{
		repo: repo,
		log:  log.With().Str("component", "RequestHandler").Logger(),
	}
}

// GetRequestsGin returns active requests with pagination
func (h *RequestHandler) GetRequestsGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, offset := GetPaginationGin(c)
	query := c.Query("q")

	var requests []*entity.Request
	var err error

	if query != "" {
		requests, err = h.repo.Search(ctx, query, limit, offset)
	} else {
		requests, err = h.repo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get requests")
		DatabaseErrorGin(c, "Failed to fetch requests")
		return
	}

	total, _ := h.repo.CountActive(ctx)
	SuccessWithMetaGin(c, requests, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetRequestGin returns a single request by ID
func (h *RequestHandler) GetRequestGin(c *gin.Context) {
	id, ok := ValidateID(c, "id")
	if !ok {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	request, err := h.repo.GetByID(ctx, id)
	if err != nil {
		NotFoundGin(c, ErrRequestNotFound())
		return
	}

	SuccessGin(c, request)
}
