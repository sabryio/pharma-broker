package handlers

import (
	"context"
	"net/http"
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
	th := NewTestHelper(t)
	log := zerolog.Nop()
	handlers := NewLearningHandler(nil, nil, nil, log)

	c, w := th.CreateContext("GET", "/api/admin/learning/status", nil)

	handlers.GetLearningStatusGin(c)

	th.AssertStatus(w, http.StatusServiceUnavailable)
}

func TestGetLearningStatus_Success(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{
		status: ai.SchedulerStatus{
			Enabled:  true,
			Schedule: "0 3 * * *",
		},
	}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	c, w := th.CreateContext("GET", "/api/admin/learning/status", nil)

	handlers.GetLearningStatusGin(c)

	th.AssertStatus(w, http.StatusOK)

	var wrapper struct {
		Success bool                   `json:"success"`
		Data    LearningStatusResponse `json:"data"`
	}
	th.AssertJSONResponse(w, &wrapper)

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
	th := NewTestHelper(t)
	log := zerolog.Nop()
	handlers := NewLearningHandler(nil, nil, nil, log)

	c, w := th.CreateContext("POST", "/api/admin/learning/trigger", nil)

	handlers.TriggerLearningGin(c)

	th.AssertStatus(w, http.StatusServiceUnavailable)
}

func TestApplyPendingWeights_NoConfirm(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	body := map[string]bool{"confirm": false}
	c, w := th.CreateContext("POST", "/api/admin/learning/apply", body)

	handlers.ApplyPendingWeightsGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}

func TestRejectPendingWeights_Success(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	c, w := th.CreateContext("POST", "/api/admin/learning/reject", nil)

	handlers.RejectPendingWeightsGin(c)

	th.AssertStatus(w, http.StatusOK)
}

func TestGetWeightHistory_Success(t *testing.T) {
	th := NewTestHelper(t)
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

	c, w := th.CreateContext("GET", "/api/admin/learning/history", nil)

	handlers.GetWeightHistoryGin(c)

	th.AssertStatus(w, http.StatusOK)

	var wrapper struct {
		Success bool                  `json:"success"`
		Data    WeightHistoryResponse `json:"data"`
	}
	th.AssertJSONResponse(w, &wrapper)

	if wrapper.Data.Total != 1 {
		t.Errorf("Total = %d, want 1", wrapper.Data.Total)
	}
}

func TestGetFeedbackStats_Success(t *testing.T) {
	th := NewTestHelper(t)
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

	c, w := th.CreateContext("GET", "/api/admin/learning/feedback-stats", nil)

	handlers.GetFeedbackStatsGin(c)

	th.AssertStatus(w, http.StatusOK)

	var wrapper struct {
		Success bool                  `json:"success"`
		Data    FeedbackStatsResponse `json:"data"`
	}
	th.AssertJSONResponse(w, &wrapper)

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
	th := NewTestHelper(t)
	log := zerolog.Nop()
	historyRepo := &mockLearningWeightHistoryRepo{
		err:     nil,
		current: nil, // No current weights
	}

	handlers := NewLearningHandler(nil, nil, historyRepo, log)

	c, w := th.CreateContext("GET", "/api/admin/learning/weights", nil)

	handlers.GetCurrentWeightsGin(c)

	th.AssertStatus(w, http.StatusOK)

	var wrapper struct {
		Success bool                   `json:"success"`
		Data    CurrentWeightsResponse `json:"data"`
	}
	th.AssertJSONResponse(w, &wrapper)

	if wrapper.Data.Source != "default" {
		t.Errorf("Source = %s, want 'default'", wrapper.Data.Source)
	}
}

func TestUpdateWeightsManually_InvalidSum(t *testing.T) {
	th := NewTestHelper(t)
	log := zerolog.Nop()
	scheduler := &mockLearningScheduler{}
	handlers := NewLearningHandler(scheduler, nil, nil, log)

	// Weights don't sum to 1.0
	body := map[string]interface{}{
		"weights": map[string]float64{"medication": 0.5, "dosage": 0.5, "quantity": 0.5, "price": 0.5, "recency": 0.5},
		"notes":   "test",
	}
	c, w := th.CreateContext("PUT", "/api/admin/learning/weights", body)

	handlers.UpdateWeightsManuallyGin(c)

	th.AssertStatus(w, http.StatusBadRequest)
}
