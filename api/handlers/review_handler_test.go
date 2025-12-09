package handlers

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

func TestReviewHandler_GetPendingReviews(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending, CreatedAt: time.Now()},
		{ID: "review-2", Status: entity.ReviewStatusPending, CreatedAt: time.Now()},
	}}
	h := NewReviewHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/review/queue", nil)
	w := httptest.NewRecorder()

	h.GetPendingReviews(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestReviewHandler_GetReviewCount(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1"},
		{ID: "review-2"},
	}}
	h := NewReviewHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/review/count", nil)
	w := httptest.NewRecorder()

	h.GetReviewCount(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}

	// Response is wrapped in {success: true, data: {...}}
	var resp struct {
		Success bool `json:"success"`
		Data    struct {
			PendingCount int64 `json:"pending_count"`
		} `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if resp.Data.PendingCount != 2 {
		t.Errorf("Expected pending_count 2, got %d", resp.Data.PendingCount)
	}
}

func TestReviewHandler_GetReviewItem(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/review/review-1", nil)
	req.SetPathValue("id", "review-1")
	w := httptest.NewRecorder()

	h.GetReviewItem(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestReviewHandler_GetReviewItem_NotFound(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{}}
	h := NewReviewHandler(repo, log)

	req := httptest.NewRequest(http.MethodGet, "/api/review/nonexistent", nil)
	req.SetPathValue("id", "nonexistent")
	w := httptest.NewRecorder()

	h.GetReviewItem(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("Expected status 404, got %d", w.Code)
	}
}

func TestReviewHandler_ApproveReview(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	body, _ := json.Marshal(ApproveReviewRequest{
		ReviewedBy: "admin",
		Note:       "Looks good",
	})
	req := httptest.NewRequest(http.MethodPost, "/api/review/review-1/approve", bytes.NewReader(body))
	req.SetPathValue("id", "review-1")
	w := httptest.NewRecorder()

	h.ApproveReview(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestReviewHandler_RejectReview(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	body, _ := json.Marshal(RejectReviewRequest{
		ReviewedBy: "admin",
		Reason:     "Invalid data",
	})
	req := httptest.NewRequest(http.MethodPost, "/api/review/review-1/reject", bytes.NewReader(body))
	req.SetPathValue("id", "review-1")
	w := httptest.NewRecorder()

	h.RejectReview(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

func TestReviewHandler_ApproveReview_NotFound(t *testing.T) {
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{}}
	h := NewReviewHandler(repo, log)

	body, _ := json.Marshal(ApproveReviewRequest{ReviewedBy: "admin"})
	req := httptest.NewRequest(http.MethodPost, "/api/review/nonexistent/approve", bytes.NewReader(body))
	req.SetPathValue("id", "nonexistent")
	w := httptest.NewRecorder()

	h.ApproveReview(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("Expected status 404, got %d", w.Code)
	}
}
