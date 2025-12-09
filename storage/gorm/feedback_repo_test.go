package gorm

import (
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// =============================================================================
// FeedbackRepo Tests
// =============================================================================

func TestFeedbackRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Create offer, request, match (for foreign key)
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Create feedback
	fb := NewTestMatchFeedback(match.ID)
	err := feedbackRepo.Save(ctx, fb)
	assertNoError(t, err, "Save feedback should succeed")

	// Verify
	feedbacks, err := feedbackRepo.GetByMatchID(ctx, match.ID)
	assertNoError(t, err, "GetByMatchID should succeed")
	assertEqual(t, len(feedbacks), 1, "Should have 1 feedback")
}

func TestFeedbackRepo_RecordFeedback(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Setup
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Use RecordFeedback alias
	fb := NewTestMatchFeedback(match.ID)
	err := feedbackRepo.RecordFeedback(ctx, fb)
	assertNoError(t, err, "RecordFeedback should succeed")

	// Verify via GetFeedbackByMatch alias
	feedbacks, err := feedbackRepo.GetFeedbackByMatch(ctx, match.ID)
	assertNoError(t, err, "GetFeedbackByMatch should succeed")
	assertEqual(t, len(feedbacks), 1, "Should have 1 feedback")
}

func TestFeedbackRepo_GetByMatchID(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Setup two matches
	offer1 := CreateTestOfferWithRawMessage(t, db)
	request1 := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer1), "Save offer1")
	assertNoError(t, requestRepo.Save(ctx, request1), "Save request1")
	match1 := NewTestMatch(offer1.ID, request1.ID)
	assertNoError(t, matchRepo.Save(ctx, match1), "Save match1")

	offer2 := CreateTestOfferWithRawMessage(t, db)
	request2 := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer2), "Save offer2")
	assertNoError(t, requestRepo.Save(ctx, request2), "Save request2")
	match2 := NewTestMatch(offer2.ID, request2.ID)
	assertNoError(t, matchRepo.Save(ctx, match2), "Save match2")

	// Add feedback to match1 only
	fb := NewTestMatchFeedback(match1.ID)
	assertNoError(t, feedbackRepo.Save(ctx, fb), "Save feedback")

	// GetByMatchID for match1 should return 1
	feedbacks, err := feedbackRepo.GetByMatchID(ctx, match1.ID)
	assertNoError(t, err, "GetByMatchID should succeed")
	assertEqual(t, len(feedbacks), 1, "Should have 1 feedback for match1")

	// GetByMatchID for match2 should return 0
	feedbacks, err = feedbackRepo.GetByMatchID(ctx, match2.ID)
	assertNoError(t, err, "GetByMatchID should succeed")
	assertEqual(t, len(feedbacks), 0, "Should have 0 feedback for match2")
}

func TestFeedbackRepo_GetRecent_Ordering(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Setup match
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Create feedbacks with different times
	now := time.Now()
	fb1 := NewTestMatchFeedback(match.ID, func(f *domain.MatchFeedback) {
		f.CreatedAt = now.Add(-2 * time.Hour)
	})
	fb2 := NewTestMatchFeedback(match.ID, func(f *domain.MatchFeedback) {
		f.CreatedAt = now.Add(-1 * time.Hour)
	})
	fb3 := NewTestMatchFeedback(match.ID, func(f *domain.MatchFeedback) {
		f.CreatedAt = now
	})

	assertNoError(t, feedbackRepo.Save(ctx, fb1), "Save fb1")
	assertNoError(t, feedbackRepo.Save(ctx, fb2), "Save fb2")
	assertNoError(t, feedbackRepo.Save(ctx, fb3), "Save fb3")

	// GetRecent should be ordered DESC (newest first)
	recent, err := feedbackRepo.GetRecent(ctx, 10)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(recent), 3, "Should have 3 feedbacks")
	assertEqual(t, recent[0].ID, fb3.ID, "First should be newest")
	assertEqual(t, recent[2].ID, fb1.ID, "Last should be oldest")
}

func TestFeedbackRepo_GetRecentFeedback(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Setup
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Create 5 feedbacks
	for i := 0; i < 5; i++ {
		fb := NewTestMatchFeedback(match.ID)
		assertNoError(t, feedbackRepo.Save(ctx, fb), "Save feedback")
	}

	// GetRecentFeedback with limit
	recent, err := feedbackRepo.GetRecentFeedback(ctx, 3)
	assertNoError(t, err, "GetRecentFeedback should succeed")
	assertEqual(t, len(recent), 3, "Should have 3 feedbacks")
}

func TestFeedbackRepo_AnalyzeFeedback_Stats(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	feedbackRepo := NewFeedbackRepo(db.DB)
	ctx := testCtx()

	// Setup
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Create 3 confirmed, 1 rejected
	for i := 0; i < 3; i++ {
		fb := NewTestMatchFeedback(match.ID, func(f *domain.MatchFeedback) {
			f.Decision = domain.FeedbackConfirmed
		})
		assertNoError(t, feedbackRepo.Save(ctx, fb), "Save confirmed")
	}
	fb := NewTestMatchFeedback(match.ID, func(f *domain.MatchFeedback) {
		f.Decision = domain.FeedbackRejected
	})
	assertNoError(t, feedbackRepo.Save(ctx, fb), "Save rejected")

	// Analyze
	analysis, err := feedbackRepo.AnalyzeFeedback(ctx, 7)
	assertNoError(t, err, "AnalyzeFeedback should succeed")
	assertEqual(t, analysis.TotalFeedback, 4, "Total should be 4")
	assertEqual(t, analysis.PositiveFeedback, 3, "Positive should be 3")
	assertEqual(t, analysis.NegativeFeedback, 1, "Negative should be 1")
	assertEqual(t, analysis.AccuracyRate, 75.0, "Accuracy should be 75%")
}
