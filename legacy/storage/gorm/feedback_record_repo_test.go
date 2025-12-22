package gorm

import (
	"testing"
	"time"

	"github.com/google/uuid"

	"pharmabroker/domain/entity"
)

// =============================================================================
// FeedbackRecordRepo Tests
// =============================================================================

func TestFeedbackRecordRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	feedback := &entity.FeedbackRecord{
		ID:              uuid.New().String(),
		MatchID:         "match-123",
		OfferID:         "offer-123",
		RequestID:       "request-123",
		Action:          entity.FeedbackActionConfirmed,
		MedicationScore: 0.9,
		DosageScore:     1.0,
		QuantityScore:   0.8,
		PriceScore:      0.7,
		RecencyScore:    1.0,
		TotalScore:      0.88,
		FeedbackAt:      time.Now(),
		UserID:          "user-1",
	}

	err := repo.Save(ctx, feedback)
	assertNoError(t, err, "Save should succeed")

	// Verify
	saved, err := repo.GetByID(ctx, feedback.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved, "Saved feedback should not be nil")
	assertEqual(t, saved.MatchID, feedback.MatchID, "MatchID should match")
}

func TestFeedbackRecordRepo_GetByDateRange(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	now := time.Now()

	// Create 3 records
	records := []*entity.FeedbackRecord{
		{
			ID:         uuid.New().String(),
			FeedbackAt: now.Add(-2 * time.Hour),
			Action:     entity.FeedbackActionConfirmed,
		},
		{
			ID:         uuid.New().String(),
			FeedbackAt: now.Add(-25 * time.Hour), // Outside 24h range
			Action:     entity.FeedbackActionConfirmed,
		},
		{
			ID:         uuid.New().String(),
			FeedbackAt: now.Add(-1 * time.Hour),
			Action:     entity.FeedbackActionRejected,
		},
	}

	for _, r := range records {
		assertNoError(t, repo.Save(ctx, r), "Save record")
	}

	// query last 24h
	rangeRecords, err := repo.GetByDateRange(ctx, now.Add(-24*time.Hour), now)
	assertNoError(t, err, "GetByDateRange should succeed")
	assertEqual(t, len(rangeRecords), 2, "Should return 2 records")
}

func TestFeedbackRecordRepo_GetFeedbackStats(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	now := time.Now()

	// 1. High score confirmed
	assertNoError(t, repo.Save(ctx, &entity.FeedbackRecord{
		ID:              uuid.New().String(),
		FeedbackAt:      now,
		Action:          entity.FeedbackActionConfirmed,
		MedicationScore: 1.0,
	}), "Save 1")

	// 2. Low score confirmed
	assertNoError(t, repo.Save(ctx, &entity.FeedbackRecord{
		ID:              uuid.New().String(),
		FeedbackAt:      now,
		Action:          entity.FeedbackActionConfirmed,
		MedicationScore: 0.8,
	}), "Save 2")

	// 3. Low score rejected
	assertNoError(t, repo.Save(ctx, &entity.FeedbackRecord{
		ID:              uuid.New().String(),
		FeedbackAt:      now,
		Action:          entity.FeedbackActionRejected,
		MedicationScore: 0.2, // Low
	}), "Save 3")

	// Get stats
	stats, err := repo.GetFeedbackStats(ctx, now.Add(-1*time.Hour), now.Add(1*time.Hour))
	assertNoError(t, err, "GetFeedbackStats should succeed")

	assertEqual(t, stats.TotalFeedbacks, 3, "Total feedbacks")
	assertEqual(t, stats.ConfirmedCount, 2, "Confirmed count")
	assertEqual(t, stats.RejectedCount, 1, "Rejected count")

	// Avg Medication score for confirmed: (1.0 + 0.8) / 2 = 0.9
	assertEqual(t, stats.ConfirmedAvgMedication, 0.9, "Confirmed Avg Medication")
	// Avg Medication score for rejected: 0.2
	assertEqual(t, stats.RejectedAvgMedication, 0.2, "Rejected Avg Medication")
}

func TestFeedbackRecordRepo_CountByAction(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewFeedbackRecordRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	now := time.Now()

	assertNoError(t, repo.Save(ctx, &entity.FeedbackRecord{
		ID:         uuid.New().String(),
		FeedbackAt: now,
		Action:     entity.FeedbackActionConfirmed,
	}), "Save confirmed")

	assertNoError(t, repo.Save(ctx, &entity.FeedbackRecord{
		ID:         uuid.New().String(),
		FeedbackAt: now,
		Action:     entity.FeedbackActionRejected,
	}), "Save rejected")

	count, err := repo.CountByAction(ctx, entity.FeedbackActionConfirmed, now.Add(-1*time.Hour), now.Add(1*time.Hour))
	assertNoError(t, err, "CountByAction should succeed")
	assertEqual(t, count, int64(1), "Should count 1 confirmed")
}
