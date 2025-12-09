package handlers

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"pharmabroker/ai"
	"pharmabroker/domain/entity"
	"pharmabroker/matching"

	"github.com/rs/zerolog"
)

// Mock feedback repository
type mockLearningFeedbackRepo struct {
	stats *entity.FeedbackStats
	err   error
}

func (m *mockLearningFeedbackRepo) GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*entity.FeedbackStats, error) {
	return m.stats, m.err
}

func (m *mockLearningFeedbackRepo) CountByAction(ctx context.Context, action entity.FeedbackAction, startDate, endDate time.Time) (int64, error) {
	return 0, nil
}

// Mock weight history repository
type mockLearningWeightHistoryRepo struct {
	current *entity.WeightHistory
	history []*entity.WeightHistory
	err     error
}

func (m *mockLearningWeightHistoryRepo) GetCurrent(ctx context.Context) (*entity.WeightHistory, error) {
	return m.current, m.err
}

func (m *mockLearningWeightHistoryRepo) GetHistory(ctx context.Context, limit int) ([]*entity.WeightHistory, error) {
	if len(m.history) <= limit {
		return m.history, m.err
	}
	return m.history[:limit], m.err
}

func (m *mockLearningWeightHistoryRepo) GetBySource(ctx context.Context, source entity.WeightSource, limit int) ([]*entity.WeightHistory, error) {
	return m.history, m.err
}

// Mock scheduler for testing
type mockLearningScheduler struct {
	status        ai.SchedulerStatus
	runNowErr     error
	applyErr      error
	rollbackErr   error
	applyManualFn func(ctx context.Context, weights matching.Weights, notes string) error
}

func (m *mockLearningScheduler) Start() error {
	return nil
}

func (m *mockLearningScheduler) Stop() {
	// No-op
}

func (m *mockLearningScheduler) Status() ai.SchedulerStatus {
	return m.status
}

func (m *mockLearningScheduler) RunNow() error {
	return m.runNowErr
}

func (m *mockLearningScheduler) ApplyPending(ctx context.Context) error {
	return m.applyErr
}

func (m *mockLearningScheduler) RejectPending() {
	// No-op
}

func (m *mockLearningScheduler) Rollback(ctx context.Context) error {
	return m.rollbackErr
}

func (m *mockLearningScheduler) ApplyWeightsManual(ctx context.Context, weights matching.Weights, notes string) error {
	if m.applyManualFn != nil {
		return m.applyManualFn(ctx, weights, notes)
	}
	return nil
}

func TestGetLearningStatus_NoScheduler(t *testing.T) {
	log := zerolog.Nop()
	handlers := NewLearningHandler(nil, nil, nil, log)

	req := httptest.NewRequest("GET", "/api/admin/learning/status", nil)
	w := httptest.NewRecorder()

	handlers.GetLearningStatus(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusServiceUnavailable)
	}
}

func TestGetLearningStatus_Success(t *testing.T) {
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{
		status: ai.SchedulerStatus{
			Enabled:  true,
			Schedule: "0 3 * * *",
		},
	}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	req := httptest.NewRequest("GET", "/api/admin/learning/status", nil)
	w := httptest.NewRecorder()

	handlers.GetLearningStatus(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	// Decode the wrapper response first
	var wrapper struct {
		Success bool                   `json:"success"`
		Data    LearningStatusResponse `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&wrapper); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if !wrapper.Success {
		t.Error("Expected Success = true")
	}
	if !wrapper.Data.Enabled {
		t.Error("Expected Enabled = true")
	}
	if wrapper.Data.Schedule != "0 3 * * *" {
		t.Errorf("Schedule = %s, want '0 3 * * *'", wrapper.Data.Schedule)
	}
}

func TestTriggerLearning_NoScheduler(t *testing.T) {
	log := zerolog.Nop()
	handlers := NewLearningHandler(nil, nil, nil, log)

	req := httptest.NewRequest("POST", "/api/admin/learning/trigger", nil)
	w := httptest.NewRecorder()

	handlers.TriggerLearning(w, req)

	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusServiceUnavailable)
	}
}

func TestApplyPendingWeights_NoConfirm(t *testing.T) {
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	body := bytes.NewBufferString(`{"confirm": false}`)
	req := httptest.NewRequest("POST", "/api/admin/learning/apply", body)
	w := httptest.NewRecorder()

	handlers.ApplyPendingWeights(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusBadRequest)
	}
}

func TestRejectPendingWeights_Success(t *testing.T) {
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	req := httptest.NewRequest("POST", "/api/admin/learning/reject", nil)
	w := httptest.NewRecorder()

	handlers.RejectPendingWeights(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}
}

func TestGetWeightHistory_Success(t *testing.T) {
	log := zerolog.Nop()
	historyRepo := &mockLearningWeightHistoryRepo{
		history: []*entity.WeightHistory{
			{
				ID:               "test-1",
				MedicationWeight: 0.45,
				DosageWeight:     0.10,
				QuantityWeight:   0.20,
				PriceWeight:      0.15,
				RecencyWeight:    0.10,
				Source:           entity.WeightSourceManual,
				AppliedAt:        time.Now(),
			},
		},
	}

	handlers := NewLearningHandler(nil, nil, historyRepo, log)

	req := httptest.NewRequest("GET", "/api/admin/learning/history", nil)
	w := httptest.NewRecorder()

	handlers.GetWeightHistory(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var wrapper struct {
		Success bool                  `json:"success"`
		Data    WeightHistoryResponse `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&wrapper); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if wrapper.Data.Total != 1 {
		t.Errorf("Total = %d, want 1", wrapper.Data.Total)
	}
}

func TestGetFeedbackStats_Success(t *testing.T) {
	log := zerolog.Nop()
	feedbackRepo := &mockLearningFeedbackRepo{
		stats: &entity.FeedbackStats{
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

	handlers := NewLearningHandler(nil, feedbackRepo, nil, log)

	req := httptest.NewRequest("GET", "/api/admin/learning/feedback-stats", nil)
	w := httptest.NewRecorder()

	handlers.GetFeedbackStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var wrapper struct {
		Success bool                  `json:"success"`
		Data    FeedbackStatsResponse `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&wrapper); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if wrapper.Data.TotalFeedbacks != 200 {
		t.Errorf("TotalFeedbacks = %d, want 200", wrapper.Data.TotalFeedbacks)
	}
	if wrapper.Data.ConfirmationRate != 0.75 {
		t.Errorf("ConfirmationRate = %v, want 0.75", wrapper.Data.ConfirmationRate)
	}
	// Use tolerance for floating point comparison
	if wrapper.Data.Separation < 0.19 || wrapper.Data.Separation > 0.21 {
		t.Errorf("Separation = %v, want ~0.20", wrapper.Data.Separation)
	}
}

func TestGetCurrentWeights_Default(t *testing.T) {
	log := zerolog.Nop()
	historyRepo := &mockLearningWeightHistoryRepo{
		err:     nil,
		current: nil, // No current weights
	}

	handlers := NewLearningHandler(nil, nil, historyRepo, log)

	req := httptest.NewRequest("GET", "/api/admin/learning/weights", nil)
	w := httptest.NewRecorder()

	handlers.GetCurrentWeights(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusOK)
	}

	var wrapper struct {
		Success bool                   `json:"success"`
		Data    CurrentWeightsResponse `json:"data"`
	}
	if err := json.NewDecoder(w.Body).Decode(&wrapper); err != nil {
		t.Fatalf("Failed to decode response: %v", err)
	}

	if wrapper.Data.Source != "default" {
		t.Errorf("Source = %s, want 'default'", wrapper.Data.Source)
	}
}

func TestUpdateWeightsManually_InvalidSum(t *testing.T) {
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

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
