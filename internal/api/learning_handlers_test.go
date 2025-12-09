package api

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/matching"
)

// Mock feedback repository
type mockLearningFeedbackRepo struct {
	stats *domain.FeedbackStats
	err   error
}

func (m *mockLearningFeedbackRepo) GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*domain.FeedbackStats, error) {
	return m.stats, m.err
}

func (m *mockLearningFeedbackRepo) CountByAction(ctx context.Context, action domain.FeedbackAction, startDate, endDate time.Time) (int64, error) {
	return 0, nil
}

// Mock weight history repository
type mockLearningWeightHistoryRepo struct {
	current *domain.WeightHistory
	history []*domain.WeightHistory
	err     error
}

func (m *mockLearningWeightHistoryRepo) GetCurrent(ctx context.Context) (*domain.WeightHistory, error) {
	return m.current, m.err
}

func (m *mockLearningWeightHistoryRepo) GetHistory(ctx context.Context, limit int) ([]*domain.WeightHistory, error) {
	if len(m.history) <= limit {
		return m.history, m.err
	}
	return m.history[:limit], m.err
}

func (m *mockLearningWeightHistoryRepo) GetBySource(ctx context.Context, source domain.WeightSource, limit int) ([]*domain.WeightHistory, error) {
	return m.history, m.err
}

func TestGetLearningStatus_NoScheduler(t *testing.T) {
	handlers := NewLearningHandlers(nil, nil, nil)

	req := httptest.NewRequest("GET", "/api/admin/learning/status", nil)
	w := httptest.NewRecorder()

	handlers.GetLearningStatus(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusServiceUnavailable)
	}
}

func TestGetLearningStatus_Success(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{
		Enabled:  true,
		Schedule: "0 3 * * *",
	}
	scheduler := matching.NewLearningScheduler(nil, cfg, nil)
	handlers := NewLearningHandlers(scheduler, nil, nil)

	req := httptest.NewRequest("GET", "/api/admin/learning/status", nil)
	w := httptest.NewRecorder()

	handlers.GetLearningStatus(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var response LearningStatusResponse
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if !response.Enabled {
		t.Error("Expected Enabled = true")
	}
	if response.Schedule != "0 3 * * *" {
		t.Errorf("Schedule = %s, want '0 3 * * *'", response.Schedule)
	}
}

func TestTriggerLearning_NoScheduler(t *testing.T) {
	handlers := NewLearningHandlers(nil, nil, nil)

	req := httptest.NewRequest("POST", "/api/admin/learning/trigger", nil)
	w := httptest.NewRecorder()

	handlers.TriggerLearning(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusServiceUnavailable)
	}
}

func TestApplyPendingWeights_NoConfirm(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{Enabled: true}
	scheduler := matching.NewLearningScheduler(nil, cfg, nil)
	handlers := NewLearningHandlers(scheduler, nil, nil)

	body := bytes.NewBufferString(`{"confirm": false}`)
	req := httptest.NewRequest("POST", "/api/admin/learning/apply", body)
	w := httptest.NewRecorder()

	handlers.ApplyPendingWeights(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusBadRequest)
	}
}

func TestRejectPendingWeights_Success(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{Enabled: true}
	scheduler := matching.NewLearningScheduler(nil, cfg, nil)
	handlers := NewLearningHandlers(scheduler, nil, nil)

	req := httptest.NewRequest("POST", "/api/admin/learning/reject", nil)
	w := httptest.NewRecorder()

	handlers.RejectPendingWeights(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}
}

func TestGetWeightHistory_Success(t *testing.T) {
	historyRepo := &mockLearningWeightHistoryRepo{
		history: []*domain.WeightHistory{
			{
				ID:               "test-1",
				MedicationWeight: 0.45,
				DosageWeight:     0.10,
				QuantityWeight:   0.20,
				PriceWeight:      0.15,
				RecencyWeight:    0.10,
				Source:           domain.WeightSourceManual,
				AppliedAt:        time.Now(),
			},
		},
	}

	handlers := NewLearningHandlers(nil, nil, historyRepo)

	req := httptest.NewRequest("GET", "/api/admin/learning/history", nil)
	w := httptest.NewRecorder()

	handlers.GetWeightHistory(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var response WeightHistoryResponse
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if response.Total != 1 {
		t.Errorf("Total = %d, want 1", response.Total)
	}
}

func TestGetFeedbackStats_Success(t *testing.T) {
	feedbackRepo := &mockLearningFeedbackRepo{
		stats: &domain.FeedbackStats{
			TotalFeedbacks:    200,
			ConfirmedCount:    150,
			RejectedCount:     50,
			ConfirmationRate:  0.75,
			ConfirmedAvgTotal: 0.85,
			RejectedAvgTotal:  0.65,
			MedicationDiff:    0.20,
			DosageDiff:        0.05,
		},
	}

	handlers := NewLearningHandlers(nil, feedbackRepo, nil)

	req := httptest.NewRequest("GET", "/api/admin/learning/feedback-stats", nil)
	w := httptest.NewRecorder()

	handlers.GetFeedbackStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var response FeedbackStatsResponse
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if response.TotalFeedbacks != 200 {
		t.Errorf("TotalFeedbacks = %d, want 200", response.TotalFeedbacks)
	}
	if response.ConfirmationRate != 0.75 {
		t.Errorf("ConfirmationRate = %v, want 0.75", response.ConfirmationRate)
	}
	// Use tolerance for floating point comparison
	if response.Separation < 0.19 || response.Separation > 0.21 {
		t.Errorf("Separation = %v, want ~0.20", response.Separation)
	}
}

func TestGetCurrentWeights_Default(t *testing.T) {
	historyRepo := &mockLearningWeightHistoryRepo{
		err:     nil,
		current: nil, // No current weights
	}

	handlers := NewLearningHandlers(nil, nil, historyRepo)

	req := httptest.NewRequest("GET", "/api/admin/learning/weights", nil)
	w := httptest.NewRecorder()

	handlers.GetCurrentWeights(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var response CurrentWeightsResponse
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if response.Source != "default" {
		t.Errorf("Source = %s, want 'default'", response.Source)
	}
}

func TestUpdateWeightsManually_InvalidSum(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{Enabled: true}
	scheduler := matching.NewLearningScheduler(nil, cfg, nil)
	handlers := NewLearningHandlers(scheduler, nil, nil)

	// Weights don't sum to 1.0
	body := bytes.NewBufferString(`{
		"weights": {"medication": 0.5, "dosage": 0.5, "quantity": 0.5, "price": 0.5, "recency": 0.5},
		"notes": "test"
	}`)
	req := httptest.NewRequest("PUT", "/api/admin/learning/weights", body)
	w := httptest.NewRecorder()

	handlers.UpdateWeightsManually(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusBadRequest)
	}
}
