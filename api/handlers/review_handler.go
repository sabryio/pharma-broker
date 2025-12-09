package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"
	"time"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"

	"github.com/rs/zerolog"
)

// ReviewHandler handles review queue operations
type ReviewHandler struct {
	reviewRepo repository.ReviewQueueRepository
	log        zerolog.Logger
}

// NewReviewHandler creates a new ReviewHandler
func NewReviewHandler(reviewRepo repository.ReviewQueueRepository, log zerolog.Logger) *ReviewHandler {
	return &ReviewHandler{
		reviewRepo: reviewRepo,
		log:        log.With().Str("component", "ReviewHandler").Logger(),
	}
}

// GetPendingReviews returns pending review items with pagination
// GET /api/review/queue?limit=20&offset=0
func (h *ReviewHandler) GetPendingReviews(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit := 20
	offset := 0

	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}

	if o := r.URL.Query().Get("offset"); o != "" {
		if parsed, err := strconv.Atoi(o); err == nil && parsed >= 0 {
			offset = parsed
		}
	}

	items, err := h.reviewRepo.GetPending(ctx, limit, offset)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to fetch pending reviews")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch pending reviews"))
		return
	}

	total, _ := h.reviewRepo.CountPending(ctx)
	successWithMeta(w, items, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetReviewCount returns the count of pending reviews
// GET /api/review/count
func (h *ReviewHandler) GetReviewCount(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	count, err := h.reviewRepo.CountPending(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to count pending reviews")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to count pending reviews"))
		return
	}

	success(w, map[string]int64{"pending_count": count})
}

// GetReviewItem returns a specific review item by ID
// GET /api/review/{id}
func (h *ReviewHandler) GetReviewItem(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing review ID"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch review item"))
		return
	}

	if item == nil {
		errorWithCode(w, http.StatusNotFound, ErrNotFound("Review item not found"))
		return
	}

	success(w, item)
}

// ApproveReviewRequest represents approval request body
type ApproveReviewRequest struct {
	ReviewedBy     string              `json:"reviewed_by"`
	CorrectedItems []entity.ParsedItem `json:"corrected_items,omitempty"`
	Note           string              `json:"note,omitempty"`
}

// ApproveReview approves a review item with optional corrections
// POST /api/review/{id}/approve
func (h *ReviewHandler) ApproveReview(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing review ID"))
		return
	}

	var req ApproveReviewRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin" // Default if not provided
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Verify item exists
	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item for approval")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch review item"))
		return
	}
	if item == nil {
		errorWithCode(w, http.StatusNotFound, ErrNotFound("Review item not found"))
		return
	}

	if err := h.reviewRepo.Approve(ctx, id, req.ReviewedBy, req.CorrectedItems, req.Note); err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to approve review")
		errorWithCode(w, http.StatusInternalServerError, ErrInternal("Failed to approve review"))
		return
	}

	h.log.Info().Str("id", id).Str("reviewed_by", req.ReviewedBy).Msg("Review approved")
	success(w, map[string]string{
		"status":  "approved",
		"id":      id,
		"message": "Review item approved successfully",
	})
}

// RejectReviewRequest represents rejection request body
type RejectReviewRequest struct {
	ReviewedBy string `json:"reviewed_by"`
	Reason     string `json:"reason"`
}

// RejectReview rejects a review item
// POST /api/review/{id}/reject
func (h *ReviewHandler) RejectReview(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if id == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing review ID"))
		return
	}

	var req RejectReviewRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin"
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Verify item exists
	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item for rejection")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch review item"))
		return
	}
	if item == nil {
		errorWithCode(w, http.StatusNotFound, ErrNotFound("Review item not found"))
		return
	}

	if err := h.reviewRepo.Reject(ctx, id, req.ReviewedBy, req.Reason); err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to reject review")
		errorWithCode(w, http.StatusInternalServerError, ErrInternal("Failed to reject review"))
		return
	}

	h.log.Info().Str("id", id).Str("reviewed_by", req.ReviewedBy).Str("reason", req.Reason).Msg("Review rejected")
	success(w, map[string]string{
		"status":  "rejected",
		"id":      id,
		"message": "Review item rejected",
	})
}
