package storage

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/google/uuid"

	"pharmabroker/internal/domain"
)

func TestWeightHistoryRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	history := &domain.WeightHistory{
		ID:               uuid.New().String(),
		MedicationWeight: 0.45,
		DosageWeight:     0.10,
		QuantityWeight:   0.20,
		PriceWeight:      0.15,
		RecencyWeight:    0.10,
		Source:           domain.WeightSourceManual,
		AppliedAt:        time.Now(),
		Notes:            "Test weights",
	}

	err := repo.Save(ctx, history)
	if err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify it was saved
	current, err := repo.GetCurrent(ctx)
	if err != nil {
		t.Fatalf("GetCurrent failed: %v", err)
	}
	if current == nil {
		t.Fatal("Expected weight history, got nil")
	}
	if current.ID != history.ID {
		t.Errorf("ID mismatch: got %v, want %v", current.ID, history.ID)
	}
	if current.MedicationWeight != 0.45 {
		t.Errorf("MedicationWeight = %v, want 0.45", current.MedicationWeight)
	}
	if current.Source != domain.WeightSourceManual {
		t.Errorf("Source = %v, want MANUAL", current.Source)
	}
}

func TestWeightHistoryRepo_GetCurrent(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create 3 weight configurations
	histories := []*domain.WeightHistory{
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.50,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.10,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceDefault,
			AppliedAt:        now.Add(-48 * time.Hour),
		},
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.45,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceManual,
			AppliedAt:        now.Add(-24 * time.Hour),
		},
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.40,
			DosageWeight:     0.15,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceAutoLearned,
			AppliedAt:        now,
		},
	}

	for _, h := range histories {
		if err := repo.Save(ctx, h); err != nil {
			t.Fatalf("Save failed: %v", err)
		}
	}

	// Get current should return the most recent one
	current, err := repo.GetCurrent(ctx)
	if err != nil {
		t.Fatalf("GetCurrent failed: %v", err)
	}
	if current == nil {
		t.Fatal("Expected weight history, got nil")
	}
	if current.ID != histories[2].ID {
		t.Errorf("Expected most recent weight history")
	}
	if current.MedicationWeight != 0.40 {
		t.Errorf("MedicationWeight = %v, want 0.40", current.MedicationWeight)
	}
	if current.Source != domain.WeightSourceAutoLearned {
		t.Errorf("Source = %v, want AUTO_LEARNED", current.Source)
	}
}

func TestWeightHistoryRepo_GetCurrent_Empty(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	current, err := repo.GetCurrent(ctx)
	if err != nil {
		t.Fatalf("GetCurrent failed: %v", err)
	}
	if current != nil {
		t.Errorf("Expected nil for empty table, got %v", current)
	}
}

func TestWeightHistoryRepo_GetHistory(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create 5 weight configurations
	for i := 0; i < 5; i++ {
		history := &domain.WeightHistory{
			ID:               uuid.New().String(),
			MedicationWeight: 0.40 + float64(i)*0.01,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceManual,
			AppliedAt:        now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, history); err != nil {
			t.Fatalf("Save failed: %v", err)
		}
	}

	// Get history with limit
	histories, err := repo.GetHistory(ctx, 3)
	if err != nil {
		t.Fatalf("GetHistory failed: %v", err)
	}

	if len(histories) != 3 {
		t.Errorf("Expected 3 histories, got %d", len(histories))
	}

	// Verify ordered by applied_at DESC
	if len(histories) >= 2 {
		if histories[0].AppliedAt.Before(histories[1].AppliedAt) {
			t.Error("History should be ordered by applied_at DESC")
		}
	}

	// Get all history (no limit)
	allHistories, err := repo.GetHistory(ctx, 0)
	if err != nil {
		t.Fatalf("GetHistory with no limit failed: %v", err)
	}
	if len(allHistories) != 5 {
		t.Errorf("Expected 5 histories, got %d", len(allHistories))
	}
}

func TestWeightHistoryRepo_GetBySource(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create manual weight configurations
	for i := 0; i < 3; i++ {
		history := &domain.WeightHistory{
			ID:               uuid.New().String(),
			MedicationWeight: 0.45,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceManual,
			AppliedAt:        now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, history); err != nil {
			t.Fatalf("Save manual failed: %v", err)
		}
	}

	// Create auto-learned weight configurations
	for i := 0; i < 2; i++ {
		history := &domain.WeightHistory{
			ID:               uuid.New().String(),
			MedicationWeight: 0.40,
			DosageWeight:     0.15,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceAutoLearned,
			AppliedAt:        now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, history); err != nil {
			t.Fatalf("Save auto-learned failed: %v", err)
		}
	}

	// Get manual weights
	manualHistories, err := repo.GetBySource(ctx, domain.WeightSourceManual, 0)
	if err != nil {
		t.Fatalf("GetBySource failed: %v", err)
	}
	if len(manualHistories) != 3 {
		t.Errorf("Expected 3 manual histories, got %d", len(manualHistories))
	}
	for _, h := range manualHistories {
		if h.Source != domain.WeightSourceManual {
			t.Error("All histories should have MANUAL source")
		}
	}

	// Get auto-learned weights
	autoHistories, err := repo.GetBySource(ctx, domain.WeightSourceAutoLearned, 0)
	if err != nil {
		t.Fatalf("GetBySource failed: %v", err)
	}
	if len(autoHistories) != 2 {
		t.Errorf("Expected 2 auto-learned histories, got %d", len(autoHistories))
	}
}

func TestWeightHistoryRepo_GetByDateRange(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create weight configurations at different times
	histories := []*domain.WeightHistory{
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.45,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceManual,
			AppliedAt:        now.Add(-1 * time.Hour),
		},
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.40,
			DosageWeight:     0.15,
			QuantityWeight:   0.20,
			PriceWeight:      0.15,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceAutoLearned,
			AppliedAt:        now.Add(-12 * time.Hour),
		},
		{
			ID:               uuid.New().String(),
			MedicationWeight: 0.50,
			DosageWeight:     0.10,
			QuantityWeight:   0.20,
			PriceWeight:      0.10,
			RecencyWeight:    0.10,
			Source:           domain.WeightSourceDefault,
			AppliedAt:        now.Add(-25 * time.Hour), // Outside range
		},
	}

	for _, h := range histories {
		if err := repo.Save(ctx, h); err != nil {
			t.Fatalf("Save failed: %v", err)
		}
	}

	// Query last 24 hours
	startDate := now.Add(-24 * time.Hour)
	endDate := now

	results, err := repo.GetByDateRange(ctx, startDate, endDate)
	if err != nil {
		t.Fatalf("GetByDateRange failed: %v", err)
	}

	// Should get 2 results
	if len(results) != 2 {
		t.Errorf("Expected 2 results, got %d", len(results))
	}
}

func TestWeightHistoryRepo_SaveWithMetrics(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	metrics := &domain.PerformanceMetrics{
		Precision:         0.85,
		Recall:            0.90,
		F1Score:           0.875,
		ConfirmationRate:  0.75,
		AvgScoreConfirmed: 0.92,
		AvgScoreRejected:  0.55,
		SampleSize:        150,
		EvaluatedAt:       time.Now(),
	}

	err := repo.SaveWithMetrics(ctx,
		0.40, 0.15, 0.20, 0.15, 0.10,
		domain.WeightSourceAutoLearned,
		metrics,
		"Learned from 150 samples")

	if err != nil {
		t.Fatalf("SaveWithMetrics failed: %v", err)
	}

	// Verify it was saved with metrics
	current, err := repo.GetCurrent(ctx)
	if err != nil {
		t.Fatalf("GetCurrent failed: %v", err)
	}
	if current == nil {
		t.Fatal("Expected weight history, got nil")
	}

	if current.MedicationWeight != 0.40 {
		t.Errorf("MedicationWeight = %v, want 0.40", current.MedicationWeight)
	}
	if current.Source != domain.WeightSourceAutoLearned {
		t.Errorf("Source = %v, want AUTO_LEARNED", current.Source)
	}
	if current.Notes != "Learned from 150 samples" {
		t.Errorf("Notes = %v, want 'Learned from 150 samples'", current.Notes)
	}

	// Verify metrics were serialized
	if current.PerformanceMetrics == "" {
		t.Error("PerformanceMetrics should not be empty")
	}

	// Deserialize and verify
	var savedMetrics domain.PerformanceMetrics
	err = json.Unmarshal([]byte(current.PerformanceMetrics), &savedMetrics)
	if err != nil {
		t.Fatalf("Failed to unmarshal metrics: %v", err)
	}

	if savedMetrics.Precision != 0.85 {
		t.Errorf("Precision = %v, want 0.85", savedMetrics.Precision)
	}
	if savedMetrics.SampleSize != 150 {
		t.Errorf("SampleSize = %v, want 150", savedMetrics.SampleSize)
	}
}

func TestWeightHistoryRepo_SaveWithMetrics_NoMetrics(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewWeightHistoryRepo(db.GormDB)
	ctx := context.Background()

	err := repo.SaveWithMetrics(ctx,
		0.45, 0.10, 0.20, 0.15, 0.10,
		domain.WeightSourceManual,
		nil,
		"Manual override")

	if err != nil {
		t.Fatalf("SaveWithMetrics failed: %v", err)
	}

	current, err := repo.GetCurrent(ctx)
	if err != nil {
		t.Fatalf("GetCurrent failed: %v", err)
	}
	if current == nil {
		t.Fatal("Expected weight history, got nil")
	}

	if current.PerformanceMetrics != "" {
		t.Errorf("PerformanceMetrics should be empty when no metrics provided")
	}
}
