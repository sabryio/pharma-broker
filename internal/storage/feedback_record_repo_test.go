package storage

import (
	"context"
	"testing"
	"time"

	"github.com/google/uuid"

	"pharmabroker/internal/domain"
)

func TestFeedbackRecordRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	feedback := &domain.FeedbackRecord{
		ID:              uuid.New().String(),
		MatchID:         uuid.New().String(),
		OfferID:         uuid.New().String(),
		RequestID:       uuid.New().String(),
		Action:          domain.MatchFeedbackConfirmed,
		MedicationScore: 0.95,
		DosageScore:     0.90,
		QuantityScore:   1.0,
		PriceScore:      0.85,
		RecencyScore:    0.95,
		TotalScore:      0.93,
		FeedbackAt:      time.Now(),
		UserID:          "user123",
	}

	err := repo.Save(ctx, feedback)
	if err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Verify it was saved
	retrieved, err := repo.GetByID(ctx, feedback.ID)
	if err != nil {
		t.Fatalf("GetByID failed: %v", err)
	}
	if retrieved == nil {
		t.Fatal("Expected feedback record, got nil")
	}
	if retrieved.ID != feedback.ID {
		t.Errorf("ID mismatch: got %v, want %v", retrieved.ID, feedback.ID)
	}
	if retrieved.Action != domain.MatchFeedbackConfirmed {
		t.Errorf("Action mismatch: got %v, want %v", retrieved.Action, domain.MatchFeedbackConfirmed)
	}
	if retrieved.TotalScore != 0.93 {
		t.Errorf("TotalScore mismatch: got %v, want 0.93", retrieved.TotalScore)
	}
}

func TestFeedbackRecordRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	result, err := repo.GetByID(ctx, "non-existent-id")
	if err != nil {
		t.Fatalf("Expected no error, got %v", err)
	}
	if result != nil {
		t.Errorf("Expected nil result for non-existent ID, got %v", result)
	}
}

func TestFeedbackRecordRepo_GetByDateRange(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create feedback records at different times
	feedbacks := []*domain.FeedbackRecord{
		{
			ID:         uuid.New().String(),
			OfferID:    uuid.New().String(),
			RequestID:  uuid.New().String(),
			Action:     domain.MatchFeedbackConfirmed,
			TotalScore: 0.9,
			FeedbackAt: now.Add(-1 * time.Hour),
		},
		{
			ID:         uuid.New().String(),
			OfferID:    uuid.New().String(),
			RequestID:  uuid.New().String(),
			Action:     domain.MatchFeedbackRejected,
			TotalScore: 0.5,
			FeedbackAt: now.Add(-2 * time.Hour),
		},
		{
			ID:         uuid.New().String(),
			OfferID:    uuid.New().String(),
			RequestID:  uuid.New().String(),
			Action:     domain.MatchFeedbackConfirmed,
			TotalScore: 0.95,
			FeedbackAt: now.Add(-25 * time.Hour), // Outside range
		},
	}

	for _, fb := range feedbacks {
		if err := repo.Save(ctx, fb); err != nil {
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

	// Should get 2 results (not the one from 25h ago)
	if len(results) != 2 {
		t.Errorf("Expected 2 results, got %d", len(results))
	}

	// Verify ordered by feedback_at DESC
	if len(results) == 2 {
		if results[0].FeedbackAt.Before(results[1].FeedbackAt) {
			t.Error("Results should be ordered by feedback_at DESC")
		}
	}
}

func TestFeedbackRecordRepo_GetFeedbackStats(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create confirmed feedback with high scores
	for i := 0; i < 3; i++ {
		fb := &domain.FeedbackRecord{
			ID:              uuid.New().String(),
			OfferID:         uuid.New().String(),
			RequestID:       uuid.New().String(),
			Action:          domain.MatchFeedbackConfirmed,
			MedicationScore: 0.95,
			DosageScore:     0.90,
			QuantityScore:   0.85,
			PriceScore:      0.80,
			RecencyScore:    0.90,
			TotalScore:      0.88,
			FeedbackAt:      now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, fb); err != nil {
			t.Fatalf("Save confirmed feedback failed: %v", err)
		}
	}

	// Create rejected feedback with lower scores
	for i := 0; i < 2; i++ {
		fb := &domain.FeedbackRecord{
			ID:              uuid.New().String(),
			OfferID:         uuid.New().String(),
			RequestID:       uuid.New().String(),
			Action:          domain.MatchFeedbackRejected,
			MedicationScore: 0.60,
			DosageScore:     0.50,
			QuantityScore:   0.40,
			PriceScore:      0.30,
			RecencyScore:    0.50,
			TotalScore:      0.46,
			FeedbackAt:      now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, fb); err != nil {
			t.Fatalf("Save rejected feedback failed: %v", err)
		}
	}

	// Get stats
	startDate := now.Add(-24 * time.Hour)
	endDate := now

	stats, err := repo.GetFeedbackStats(ctx, startDate, endDate)
	if err != nil {
		t.Fatalf("GetFeedbackStats failed: %v", err)
	}

	// Verify counts
	if stats.TotalFeedbacks != 5 {
		t.Errorf("TotalFeedbacks = %d, want 5", stats.TotalFeedbacks)
	}
	if stats.ConfirmedCount != 3 {
		t.Errorf("ConfirmedCount = %d, want 3", stats.ConfirmedCount)
	}
	if stats.RejectedCount != 2 {
		t.Errorf("RejectedCount = %d, want 2", stats.RejectedCount)
	}

	// Verify confirmation rate
	expectedRate := 3.0 / 5.0
	if stats.ConfirmationRate != expectedRate {
		t.Errorf("ConfirmationRate = %v, want %v", stats.ConfirmationRate, expectedRate)
	}

	// Verify confirmed averages are higher
	if stats.ConfirmedAvgMedication <= stats.RejectedAvgMedication {
		t.Error("Expected confirmed avg medication to be higher than rejected")
	}
	if stats.ConfirmedAvgTotal <= stats.RejectedAvgTotal {
		t.Error("Expected confirmed avg total to be higher than rejected")
	}

	// Verify differences are positive (confirmed > rejected)
	if stats.MedicationDiff <= 0 {
		t.Errorf("MedicationDiff = %v, expected positive", stats.MedicationDiff)
	}
	if stats.DosageDiff <= 0 {
		t.Errorf("DosageDiff = %v, expected positive", stats.DosageDiff)
	}
}

func TestFeedbackRecordRepo_GetFeedbackStats_NoData(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()
	startDate := now.Add(-24 * time.Hour)
	endDate := now

	stats, err := repo.GetFeedbackStats(ctx, startDate, endDate)
	if err != nil {
		t.Fatalf("GetFeedbackStats failed: %v", err)
	}

	if stats.TotalFeedbacks != 0 {
		t.Errorf("TotalFeedbacks = %d, want 0", stats.TotalFeedbacks)
	}
	if stats.ConfirmationRate != 0 {
		t.Errorf("ConfirmationRate = %v, want 0", stats.ConfirmationRate)
	}
}

func TestFeedbackRecordRepo_CountByAction(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.GormDB)
	ctx := context.Background()

	now := time.Now()

	// Create confirmed feedbacks
	for i := 0; i < 3; i++ {
		fb := &domain.FeedbackRecord{
			ID:         uuid.New().String(),
			OfferID:    uuid.New().String(),
			RequestID:  uuid.New().String(),
			Action:     domain.MatchFeedbackConfirmed,
			TotalScore: 0.9,
			FeedbackAt: now.Add(-time.Duration(i) * time.Hour),
		}
		if err := repo.Save(ctx, fb); err != nil {
			t.Fatalf("Save failed: %v", err)
		}
	}

	// Create rejected feedback
	fb := &domain.FeedbackRecord{
		ID:         uuid.New().String(),
		OfferID:    uuid.New().String(),
		RequestID:  uuid.New().String(),
		Action:     domain.MatchFeedbackRejected,
		TotalScore: 0.5,
		FeedbackAt: now,
	}
	if err := repo.Save(ctx, fb); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	startDate := now.Add(-24 * time.Hour)
	endDate := now

	// Count confirmed
	count, err := repo.CountByAction(ctx, domain.MatchFeedbackConfirmed, startDate, endDate)
	if err != nil {
		t.Fatalf("CountByAction failed: %v", err)
	}
	if count != 3 {
		t.Errorf("Confirmed count = %d, want 3", count)
	}

	// Count rejected
	count, err = repo.CountByAction(ctx, domain.MatchFeedbackRejected, startDate, endDate)
	if err != nil {
		t.Fatalf("CountByAction failed: %v", err)
	}
	if count != 1 {
		t.Errorf("Rejected count = %d, want 1", count)
	}
}
