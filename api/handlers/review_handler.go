package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
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

// ApproveReviewRequest represents approval request body
type ApproveReviewRequest struct {
	ReviewedBy     string              `json:"reviewed_by"`
	CorrectedItems []entity.ParsedItem `json:"corrected_items,omitempty"`
	Note           string              `json:"note,omitempty"`
}

// RejectReviewRequest represents rejection request body
type RejectReviewRequest struct {
	ReviewedBy string `json:"reviewed_by"`
	Reason     string `json:"reason"`
}

// GetPendingReviewsGin returns pending review items with pagination
func (h *ReviewHandler) GetPendingReviewsGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, offset := GetPaginationGin(c)

	items, err := h.reviewRepo.GetPending(ctx, limit, offset)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to fetch pending reviews")
		DatabaseErrorGin(c, "Failed to fetch pending reviews")
		return
	}

	total, _ := h.reviewRepo.CountPending(ctx)
	SuccessWithMetaGin(c, items, &Meta{Total: total, Limit: limit, Offset: offset})
}

// GetReviewCountGin returns the count of pending reviews
func (h *ReviewHandler) GetReviewCountGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	count, err := h.reviewRepo.CountPending(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to count pending reviews")
		DatabaseErrorGin(c, "Failed to count pending reviews")
		return
	}

	SuccessGin(c, map[string]int64{"pending_count": count})
}

// GetReviewItemGin returns a specific review item by ID
func (h *ReviewHandler) GetReviewItemGin(c *gin.Context) {
	id, ok := GetPathIDGin(c, "id")
	if !ok {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item")
		DatabaseErrorGin(c, "Failed to fetch review item")
		return
	}

	if item == nil {
		NotFoundGin(c, ErrNotFound("Review item not found"))
		return
	}

	SuccessGin(c, item)
}

// ApproveReviewGin approves a review item with optional corrections
func (h *ReviewHandler) ApproveReviewGin(c *gin.Context) {
	id, ok := GetPathIDGin(c, "id")
	if !ok {
		return
	}

	var req ApproveReviewRequest
	if !BindJSONGin(c, &req) {
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin"
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item for approval")
		DatabaseErrorGin(c, "Failed to fetch review item")
		return
	}
	if item == nil {
		NotFoundGin(c, ErrNotFound("Review item not found"))
		return
	}

	if err := h.reviewRepo.Approve(ctx, id, req.ReviewedBy, req.CorrectedItems, req.Note); err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to approve review")
		InternalErrorGin(c, "Failed to approve review")
		return
	}

	h.log.Info().Str("id", id).Str("reviewed_by", req.ReviewedBy).Msg("Review approved")
	SuccessGin(c, map[string]string{
		"status":  "approved",
		"id":      id,
		"message": "Review item approved successfully",
	})
}

// RejectReviewGin rejects a review item
func (h *ReviewHandler) RejectReviewGin(c *gin.Context) {
	id, ok := GetPathIDGin(c, "id")
	if !ok {
		return
	}

	var req RejectReviewRequest
	if !BindJSONGin(c, &req) {
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin"
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	item, err := h.reviewRepo.GetByID(ctx, id)
	if err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to fetch review item for rejection")
		DatabaseErrorGin(c, "Failed to fetch review item")
		return
	}
	if item == nil {
		NotFoundGin(c, ErrNotFound("Review item not found"))
		return
	}

	if err := h.reviewRepo.Reject(ctx, id, req.ReviewedBy, req.Reason); err != nil {
		h.log.Error().Err(err).Str("id", id).Msg("Failed to reject review")
		InternalErrorGin(c, "Failed to reject review")
		return
	}

	h.log.Info().Str("id", id).Str("reviewed_by", req.ReviewedBy).Str("reason", req.Reason).Msg("Review rejected")
	SuccessGin(c, map[string]string{
		"status":  "rejected",
		"id":      id,
		"message": "Review item rejected",
	})
}
