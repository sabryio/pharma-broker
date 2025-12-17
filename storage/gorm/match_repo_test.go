package gorm

import (
	"testing"
	"time"

	"pharmabroker/domain/entity"
)

// =============================================================================
// MatchRepo Tests
// =============================================================================

func TestMatchRepo_Save_Insert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request first (foreign keys)
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create match
	match := NewTestMatch(offer.ID, request.ID)
	err := matchRepo.Save(ctx, match)
	assertNoError(t, err, "Save match should succeed")

	// Verify it was saved
	saved, err := matchRepo.GetByID(ctx, match.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved, "Saved match should not be nil")
	assertEqual(t, saved.OfferID, offer.ID, "OfferID should match")
	assertEqual(t, saved.RequestID, request.ID, "RequestID should match")
}

func TestMatchRepo_Save_Upsert_CompositeKey(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create first match
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "First save should succeed")

	// Update and save again
	match.Score = 0.95
	match.Reasoning = "Updated reasoning"
	err := matchRepo.Save(ctx, match)
	assertNoError(t, err, "Upsert should succeed")

	// Verify update
	saved, err := matchRepo.GetByID(ctx, match.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Score, 0.95, "Score should be updated")
}

func TestMatchRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	found, err := matchRepo.GetByID(ctx, "non-existent-id")
	assertNoError(t, err, "GetByID should not error for missing record")
	assertNil(t, found, "Should return nil for non-existent ID")
}

func TestMatchRepo_GetPending_WithPreload(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create pending match
	match := NewTestMatch(offer.ID, request.ID, func(m *entity.Match) {
		m.Status = entity.MatchStatusPending
	})
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Get pending - should include offer and request details
	pending, err := matchRepo.GetPending(ctx, 10, 0)
	assertNoError(t, err, "GetPending should succeed")
	assertEqual(t, len(pending), 1, "Should have 1 pending match")

	// Verify preloaded data
	assertNotNil(t, pending[0].Offer, "Offer should be preloaded")
	assertNotNil(t, pending[0].Request, "Request should be preloaded")
	assertEqual(t, pending[0].Offer.ID, offer.ID, "Offer ID should match")
	assertEqual(t, pending[0].Request.ID, request.ID, "Request ID should match")
}

func TestMatchRepo_UpdateStatus_Confirm(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create pending match
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Confirm match
	err := matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusConfirmed, "OPERATOR", "Test confirmation note")
	assertNoError(t, err, "UpdateStatus should succeed")

	// Verify status and confirmed_at
	saved, err := matchRepo.GetByID(ctx, match.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Status, entity.MatchStatusConfirmed, "Status should be CONFIRMED")
	assertNotNil(t, saved.ConfirmedAt, "ConfirmedAt should be set")
}

func TestMatchRepo_UpdateStatus_Reject(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create pending match
	match := NewTestMatch(offer.ID, request.ID)
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Reject match
	err := matchRepo.UpdateStatus(ctx, match.ID, entity.MatchStatusRejected, "OPERATOR", "Test rejection reason")
	assertNoError(t, err, "UpdateStatus should succeed")

	// Verify status
	saved, err := matchRepo.GetByID(ctx, match.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Status, entity.MatchStatusRejected, "Status should be REJECTED")
}

func TestMatchRepo_GetRecentConfirmed(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create confirmed match
	now := time.Now()
	match := NewTestMatch(offer.ID, request.ID, func(m *entity.Match) {
		m.Status = entity.MatchStatusConfirmed
		m.ConfirmedAt = &now
	})
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Get recent confirmed (using CountConfirmedToday)
	count, err := matchRepo.CountConfirmedToday(ctx)
	assertNoError(t, err, "CountConfirmedToday should succeed")
	assertEqual(t, count, int64(1), "Should have 1 confirmed match")
}

func TestMatchRepo_GetPending_ExcludesConfirmed(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.DB)
	requestRepo := NewRequestRepo(db.DB)
	matchRepo := NewMatchRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create offer and request
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request")

	// Create pending match
	pendingMatch := NewTestMatch(offer.ID, request.ID, func(m *entity.Match) {
		m.Status = entity.MatchStatusPending
	})
	assertNoError(t, matchRepo.Save(ctx, pendingMatch), "Save pending match")

	// Create another offer/request pair for confirmed match
	offer2 := CreateTestOfferWithRawMessage(t, db)
	request2 := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer2), "Save offer2")
	assertNoError(t, requestRepo.Save(ctx, request2), "Save request2")

	now := time.Now()
	confirmedMatch := NewTestMatch(offer2.ID, request2.ID, func(m *entity.Match) {
		m.Status = entity.MatchStatusConfirmed
		m.ConfirmedAt = &now
	})
	assertNoError(t, matchRepo.Save(ctx, confirmedMatch), "Save confirmed match")

	// Get pending - should only return pending
	pending, err := matchRepo.GetPending(ctx, 10, 0)
	assertNoError(t, err, "GetPending should succeed")
	assertEqual(t, len(pending), 1, "Should have 1 pending match")
	assertEqual(t, pending[0].ID, pendingMatch.ID, "Should be the pending match")
}
