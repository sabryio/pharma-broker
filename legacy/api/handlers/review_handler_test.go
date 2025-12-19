package handlers

import (
	"net/http"
	"testing"
	"time"

	"pharmabroker/domain/entity"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"
)

func TestReviewHandler_GetPendingReviews(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending, CreatedAt: time.Now()},
		{ID: "review-2", Status: entity.ReviewStatusPending, CreatedAt: time.Now()},
	}}
	h := NewReviewHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/review/queue", nil)

	h.GetPendingReviewsGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestReviewHandler_GetReviewCount(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1"},
		{ID: "review-2"},
	}}
	h := NewReviewHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/review/count", nil)

	h.GetReviewCountGin(c)

	th.AssertStatus(w, http.StatusOK)

	// Response is wrapped in {success: true, data: {...}}
	var resp struct {
		Success bool `json:"success"`
		Data    struct {
			PendingCount int64 `json:"pending_count"`
		} `json:"data"`
	}
	th.AssertJSONResponse(w, &resp)

	if resp.Data.PendingCount != 2 {
		t.Errorf("Expected pending_count 2, got %d", resp.Data.PendingCount)
	}
}

func TestReviewHandler_GetReviewItem(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/review/review-1", nil)
	c.Params = gin.Params{{Key: "id", Value: "review-1"}}

	h.GetReviewItemGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestReviewHandler_GetReviewItem_NotFound(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{}}
	h := NewReviewHandler(repo, log)

	c, w := th.CreateContext("GET", "/api/review/nonexistent", nil)
	c.Params = gin.Params{{Key: "id", Value: "nonexistent"}}

	h.GetReviewItemGin(c)

	th.AssertStatus(w, http.StatusNotFound)
}

func TestReviewHandler_ApproveReview(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	body := ApproveReviewRequest{
		ReviewedBy: "admin",
		Note:       "Looks good",
	}
	c, w := th.CreateContext("POST", "/api/review/review-1/approve", body)
	c.Params = gin.Params{{Key: "id", Value: "review-1"}}

	h.ApproveReviewGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestReviewHandler_RejectReview(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{
		{ID: "review-1", Status: entity.ReviewStatusPending},
	}}
	h := NewReviewHandler(repo, log)

	body := RejectReviewRequest{
		ReviewedBy: "admin",
		Reason:     "Invalid data",
	}
	c, w := th.CreateContext("POST", "/api/review/review-1/reject", body)
	c.Params = gin.Params{{Key: "id", Value: "review-1"}}

	h.RejectReviewGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestReviewHandler_ApproveReview_NotFound(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	repo := &mockReviewRepo{items: []*entity.ReviewQueueItem{}}
	h := NewReviewHandler(repo, log)

	body := ApproveReviewRequest{ReviewedBy: "admin"}
	c, w := th.CreateContext("POST", "/api/review/nonexistent/approve", body)
	c.Params = gin.Params{{Key: "id", Value: "nonexistent"}}

	h.ApproveReviewGin(c)

	th.AssertStatus(w, http.StatusNotFound)
}
