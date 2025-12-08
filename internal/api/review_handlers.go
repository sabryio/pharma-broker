package api

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"

	"pharmabroker/internal/domain"
)

// ReviewQueueRepository defines the repository interface for review queue
type ReviewQueueRepository interface {
	GetPending(ctx context.Context, limit, offset int) ([]*domain.ReviewQueueItem, error)
	CountPending(ctx context.Context) (int64, error)
	GetByID(ctx context.Context, id string) (*domain.ReviewQueueItem, error)
	Approve(ctx context.Context, id string, reviewedBy string, correctedItems []domain.ParsedItem, note string) error
	Reject(ctx context.Context, id string, reviewedBy string, reason string) error
}

// ReviewHandlers handles review queue API endpoints
type ReviewHandlers struct {
	reviewRepo ReviewQueueRepository
}

// NewReviewHandlers creates new review handlers
func NewReviewHandlers(reviewRepo ReviewQueueRepository) *ReviewHandlers {
	return &ReviewHandlers{reviewRepo: reviewRepo}
}

// RegisterRoutes registers review queue routes on the given mux
func (h *ReviewHandlers) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/review/queue", h.GetPendingReviews)
	mux.HandleFunc("GET /api/review/count", h.GetReviewCount)
	mux.HandleFunc("GET /api/review/{id}", h.GetReviewItem)
	mux.HandleFunc("POST /api/review/{id}/approve", h.ApproveReview)
	mux.HandleFunc("POST /api/review/{id}/reject", h.RejectReview)
}

// GetPendingReviews returns pending review items
// GET /api/review/queue?limit=20&offset=0
func (h *ReviewHandlers) GetPendingReviews(w http.ResponseWriter, r *http.Request) {
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

	items, err := h.reviewRepo.GetPending(r.Context(), limit, offset)
	if err != nil {
		http.Error(w, "Failed to fetch pending reviews", http.StatusInternalServerError)
		return
	}

	total, _ := h.reviewRepo.CountPending(r.Context())

	response := map[string]interface{}{
		"items":  items,
		"total":  total,
		"limit":  limit,
		"offset": offset,
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// GetReviewCount returns the count of pending reviews
// GET /api/review/count
func (h *ReviewHandlers) GetReviewCount(w http.ResponseWriter, r *http.Request) {
	count, err := h.reviewRepo.CountPending(r.Context())
	if err != nil {
		http.Error(w, "Failed to count pending reviews", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]int64{"pending_count": count})
}

// GetReviewItem returns a specific review item by ID
// GET /api/review/{id}
func (h *ReviewHandlers) GetReviewItem(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	item, err := h.reviewRepo.GetByID(r.Context(), id)
	if err != nil {
		http.Error(w, "Failed to fetch review item", http.StatusInternalServerError)
		return
	}

	if item == nil {
		http.Error(w, "Review item not found", http.StatusNotFound)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(item)
}

// ApproveRequest represents approval request body
type ApproveRequest struct {
	ReviewedBy     string              `json:"reviewed_by"`
	CorrectedItems []domain.ParsedItem `json:"corrected_items,omitempty"`
	Note           string              `json:"note,omitempty"`
}

// ApproveReview approves a review item with optional corrections
// POST /api/review/{id}/approve
func (h *ReviewHandlers) ApproveReview(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	var req ApproveRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin" // Default if not provided
	}

	// Verify item exists
	item, err := h.reviewRepo.GetByID(r.Context(), id)
	if err != nil {
		http.Error(w, "Failed to fetch review item", http.StatusInternalServerError)
		return
	}
	if item == nil {
		http.Error(w, "Review item not found", http.StatusNotFound)
		return
	}

	if err := h.reviewRepo.Approve(r.Context(), id, req.ReviewedBy, req.CorrectedItems, req.Note); err != nil {
		http.Error(w, "Failed to approve review", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "approved",
		"id":      id,
		"message": "Review item approved successfully",
	})
}

// RejectRequest represents rejection request body
type RejectRequest struct {
	ReviewedBy string `json:"reviewed_by"`
	Reason     string `json:"reason"`
}

// RejectReview rejects a review item
// POST /api/review/{id}/reject
func (h *ReviewHandlers) RejectReview(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")

	var req RejectRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Invalid request body", http.StatusBadRequest)
		return
	}

	if req.ReviewedBy == "" {
		req.ReviewedBy = "admin"
	}

	// Verify item exists
	item, err := h.reviewRepo.GetByID(r.Context(), id)
	if err != nil {
		http.Error(w, "Failed to fetch review item", http.StatusInternalServerError)
		return
	}
	if item == nil {
		http.Error(w, "Review item not found", http.StatusNotFound)
		return
	}

	if err := h.reviewRepo.Reject(r.Context(), id, req.ReviewedBy, req.Reason); err != nil {
		http.Error(w, "Failed to reject review", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{
		"status":  "rejected",
		"id":      id,
		"message": "Review item rejected",
	})
}
