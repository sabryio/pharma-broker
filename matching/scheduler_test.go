package matching

import (
	"context"
	"log/slog"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

func TestNewLearningScheduler(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{
		Enabled:  false,
		Schedule: "0 3 * * *",
	}

	scheduler := NewLearningScheduler(nil, cfg, nil)

	if scheduler == nil {
		t.Fatal("Expected scheduler, got nil")
	}
	if scheduler.lastStatus != JobStatusPending {
		t.Errorf("lastStatus = %v, want %v", scheduler.lastStatus, JobStatusPending)
	}
}

func TestLearningScheduler_Start_Disabled(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{
		Enabled: false,
	}

	scheduler := NewLearningScheduler(nil, cfg, slog.Default())

	err := scheduler.Start()
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	// Should not have started cron
	if scheduler.cron != nil {
		t.Error("Expected cron to be nil when disabled")
	}
}

func TestLearningScheduler_Start_InvalidSchedule(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{
		Enabled:  true,
		Schedule: "invalid-cron",
	}

	scheduler := NewLearningScheduler(nil, cfg, slog.Default())

	err := scheduler.Start()
	if err == nil {
		t.Error("Expected error for invalid schedule, got nil")
	}
}

func TestLearningScheduler_ShouldApply(t *testing.T) {
	tests := []struct {
		name   string
		config config.AutoApplyConfig
		old    domain.PerformanceMetrics
		new    domain.PerformanceMetrics
		want   bool
	}{
		{
			name: "RequireImprovement disabled - always apply",
			config: config.AutoApplyConfig{
				Enabled:            true,
				RequireImprovement: false,
			},
			old:  domain.PerformanceMetrics{},
			new:  domain.PerformanceMetrics{},
			want: true,
		},
		{
			name: "Separation improved, rate stable - apply",
			config: config.AutoApplyConfig{
				Enabled:                 true,
				RequireImprovement:      true,
				MinSeparationGain:       0.01,
				MaxConfirmationRateDrop: 0.05,
			},
			old: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.85,
				AvgScoreRejected:  0.65,
				ConfirmationRate:  0.75,
			},
			new: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.88,
				AvgScoreRejected:  0.62,
				ConfirmationRate:  0.76,
			},
			want: true,
		},
		{
			name: "Separation not improved enough - skip",
			config: config.AutoApplyConfig{
				Enabled:                 true,
				RequireImprovement:      true,
				MinSeparationGain:       0.10, // Need 10% gain
				MaxConfirmationRateDrop: 0.05,
			},
			old: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.85,
				AvgScoreRejected:  0.65,
				ConfirmationRate:  0.75,
			},
			new: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.86,
				AvgScoreRejected:  0.64,
				ConfirmationRate:  0.75,
			},
			want: false, // Only gained 0.02
		},
		{
			name: "Confirmation rate dropped too much - skip",
			config: config.AutoApplyConfig{
				Enabled:                 true,
				RequireImprovement:      true,
				MinSeparationGain:       0.01,
				MaxConfirmationRateDrop: 0.05,
			},
			old: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.80,
				AvgScoreRejected:  0.60,
				ConfirmationRate:  0.80,
			},
			new: domain.PerformanceMetrics{
				AvgScoreConfirmed: 0.90,
				AvgScoreRejected:  0.55,
				ConfirmationRate:  0.70, // Dropped by 0.10
			},
			want: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := config.AdaptiveLearningConfig{
				AutoApply: tt.config,
			}
			scheduler := NewLearningScheduler(nil, cfg, nil)

			got := scheduler.shouldApply(tt.old, tt.new)
			if got != tt.want {
				t.Errorf("shouldApply() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestLearningScheduler_Status(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{
		Enabled:  true,
		Schedule: "0 3 * * *",
	}

	scheduler := NewLearningScheduler(nil, cfg, nil)

	status := scheduler.Status()

	if !status.Enabled {
		t.Error("Expected Enabled = true")
	}
	if status.Schedule != "0 3 * * *" {
		t.Errorf("Schedule = %v, want '0 3 * * *'", status.Schedule)
	}
	if status.LastStatus != JobStatusPending {
		t.Errorf("LastStatus = %v, want %v", status.LastStatus, JobStatusPending)
	}
}

func TestLearningScheduler_ApplyPendingNone(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{}
	scheduler := NewLearningScheduler(nil, cfg, nil)

	err := scheduler.ApplyPending(context.Background())
	if err == nil {
		t.Error("Expected error when no pending weights, got nil")
	}
}

func TestLearningScheduler_RejectPending(t *testing.T) {
	cfg := config.AdaptiveLearningConfig{}
	scheduler := NewLearningScheduler(nil, cfg, slog.Default())

	// Set some pending weights
	scheduler.pendingApply = &Weights{Medication: 0.5}
	scheduler.pendingReason = "test"

	scheduler.RejectPending()

	if scheduler.pendingApply != nil {
		t.Error("Expected pendingApply to be nil after reject")
	}
	if scheduler.lastStatus != JobStatusSkipped {
		t.Errorf("lastStatus = %v, want %v", scheduler.lastStatus, JobStatusSkipped)
	}
}

func TestLearningScheduler_UpdateConfig(t *testing.T) {
	scorer := NewScorer(nil, nil)
	feedbackRepo := &mockFeedbackRepo{}
	historyRepo := &mockWeightHistoryRepo{}
	learner := NewWeightLearner(feedbackRepo, historyRepo, scorer)

	cfg := config.AdaptiveLearningConfig{
		Algorithm: config.LearningAlgorithmConfig{
			LearningRate: 0.1,
			MinSamples:   100,
		},
	}

	scheduler := NewLearningScheduler(learner, cfg, slog.Default())

	// Update config
	newCfg := config.AdaptiveLearningConfig{
		Algorithm: config.LearningAlgorithmConfig{
			LearningRate: 0.05,
			MinSamples:   200,
		},
	}

	err := scheduler.UpdateConfig(newCfg)
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	// Verify learner config was updated
	learnerConfig := learner.GetConfig()
	if learnerConfig.LearningRate != 0.05 {
		t.Errorf("LearningRate = %v, want 0.05", learnerConfig.LearningRate)
	}
	if learnerConfig.MinSamples != 200 {
		t.Errorf("MinSamples = %v, want 200", learnerConfig.MinSamples)
	}
}

func TestLearningScheduler_BuildNote(t *testing.T) {
	scheduler := &LearningScheduler{}

	old := Weights{
		Medication: 0.45, Dosage: 0.10, Quantity: 0.20, Price: 0.15, Recency: 0.10,
	}
	new := Weights{
		Medication: 0.48, Dosage: 0.10, Quantity: 0.18, Price: 0.14, Recency: 0.10,
	}
	metrics := domain.PerformanceMetrics{
		SampleSize:        200,
		AvgScoreConfirmed: 0.88,
		AvgScoreRejected:  0.65,
	}

	note := scheduler.buildNote(old, new, metrics)

	if note == "" {
		t.Error("Expected non-empty note")
	}

	// Should contain sample size
	if !contains(note, "200") {
		t.Errorf("Note should contain sample size, got: %s", note)
	}
}

// Helper function
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func TestLearningScheduler_RunNow_InsufficientData(t *testing.T) {
	feedbackRepo := &mockFeedbackRepo{
		stats: &domain.FeedbackStats{
			TotalFeedbacks: 50, // Less than minimum
		},
	}

	scorer := NewScorer(nil, nil)
	learner := NewWeightLearner(feedbackRepo, &mockWeightHistoryRepo{}, scorer)

	cfg := config.AdaptiveLearningConfig{
		Enabled: true,
		Algorithm: config.LearningAlgorithmConfig{
			MinSamples:         100,
			AnalysisWindowDays: 30,
		},
	}

	scheduler := NewLearningScheduler(learner, cfg, slog.Default())

	err := scheduler.RunNow()

	// Should fail due to insufficient data
	if err == nil {
		t.Error("Expected error for insufficient data, got nil")
	}

	status := scheduler.Status()
	if status.LastStatus != JobStatusFailed {
		t.Errorf("LastStatus = %v, want %v", status.LastStatus, JobStatusFailed)
	}
}

func TestLearningScheduler_RunNow_Success_AutoApplyDisabled(t *testing.T) {
	feedbackRepo := &mockFeedbackRepo{
		stats: &domain.FeedbackStats{
			TotalFeedbacks:         200,
			ConfirmedCount:         150,
			ConfirmationRate:       0.75,
			ConfirmedAvgMedication: 0.90,
			RejectedAvgMedication:  0.60,
			MedicationDiff:         0.30,
			ConfirmedAvgDosage:     0.85,
			RejectedAvgDosage:      0.80,
			DosageDiff:             0.05,
			ConfirmedAvgQuantity:   0.88,
			RejectedAvgQuantity:    0.82,
			QuantityDiff:           0.06,
			ConfirmedAvgPrice:      0.80,
			RejectedAvgPrice:       0.75,
			PriceDiff:              0.05,
			ConfirmedAvgRecency:    0.92,
			RejectedAvgRecency:     0.88,
			RecencyDiff:            0.04,
			ConfirmedAvgTotal:      0.87,
			RejectedAvgTotal:       0.75,
		},
	}

	scorer := NewScorer(nil, nil)
	learner := NewWeightLearner(feedbackRepo, &mockWeightHistoryRepo{}, scorer)

	cfg := config.AdaptiveLearningConfig{
		Enabled: true,
		Algorithm: config.LearningAlgorithmConfig{
			LearningRate:       0.1,
			MinWeight:          0.05,
			MaxWeight:          0.70,
			MinChange:          0.02,
			MinSamples:         100,
			AnalysisWindowDays: 30,
		},
		AutoApply: config.AutoApplyConfig{
			Enabled: false, // Manual review
		},
	}

	scheduler := NewLearningScheduler(learner, cfg, slog.Default())

	err := scheduler.RunNow()

	// Should succeed but not apply
	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	status := scheduler.Status()

	// Should have recommended status (not applied)
	if status.LastStatus != JobStatusRecommended {
		t.Errorf("LastStatus = %v, want %v", status.LastStatus, JobStatusRecommended)
	}

	// Should have pending weights
	if status.PendingApply == nil {
		t.Error("Expected pending weights, got nil")
	}

	if status.PendingReason == "" {
		t.Error("Expected pending reason, got empty")
	}
}

func TestSchedulerStatus_AllFields(t *testing.T) {
	now := time.Now()
	metrics := &domain.PerformanceMetrics{SampleSize: 100}
	pending := &Weights{Medication: 0.5}

	status := SchedulerStatus{
		Enabled:       true,
		Schedule:      "0 3 * * *",
		LastRun:       now,
		LastStatus:    JobStatusRecommended,
		LastError:     nil,
		LastMetrics:   metrics,
		PendingApply:  pending,
		PendingReason: "manual review",
	}

	if !status.Enabled {
		t.Error("Expected Enabled = true")
	}
	if status.LastMetrics.SampleSize != 100 {
		t.Error("Expected SampleSize = 100")
	}
	if status.PendingApply.Medication != 0.5 {
		t.Error("Expected Medication = 0.5")
	}
}
