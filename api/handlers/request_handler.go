package handlers

import (
	"context"
	"net/http"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"

	"github.com/rs/zerolog"
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

// GetRequests returns active requests with pagination
func (h *RequestHandler) GetRequests(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, offset := getPagination(r)
	query := r.URL.Query().Get("q")

	var requests []*entity.Request
	var err error

	if query != "" {
		requests, err = h.repo.Search(ctx, query, limit, offset)
	} else {
		requests, err = h.repo.GetActive(ctx, limit, offset)
	}

	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get requests")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch requests"))
		return
	}

	total, _ := h.repo.CountActive(ctx)
	successWithMeta(w, requests, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetRequest returns a single request by ID
func (h *RequestHandler) GetRequest(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing request ID"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	request, err := h.repo.GetByID(ctx, id)
	if err != nil {
		errorWithCode(w, http.StatusNotFound, ErrRequestNotFound())
		return
	}

	success(w, request)
}
