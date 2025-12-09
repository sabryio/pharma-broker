package gorm

import (
	"testing"
	"time"

	"pharmabroker/domain/entity"
)

// =============================================================================
// OfferRepo Tests
// =============================================================================

func TestOfferRepo_Save_Insert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	offer := CreateTestOfferWithRawMessage(t, db)
	err := repo.Save(ctx, offer)
	assertNoError(t, err, "Save should succeed")

	// Verify it was saved
	saved, err := repo.GetByID(ctx, offer.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved, "Saved offer should not be nil")
	assertEqual(t, saved.Medication, offer.Medication, "Medication should match")
	assertEqual(t, saved.Quantity, offer.Quantity, "Quantity should match")
}

func TestOfferRepo_Save_Upsert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	offer := CreateTestOfferWithRawMessage(t, db)
	err := repo.Save(ctx, offer)
	assertNoError(t, err, "First save should succeed")

	// Update and save again (upsert)
	offer.Quantity = 100
	offer.Price = 200.0
	err = repo.Save(ctx, offer)
	assertNoError(t, err, "Upsert should succeed")

	// Verify update
	saved, err := repo.GetByID(ctx, offer.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Quantity, 100.0, "Quantity should be updated")
}

func TestOfferRepo_GetByID_Found(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	offer := CreateTestOfferWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, offer), "Save should succeed")

	found, err := repo.GetByID(ctx, offer.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, found, "Should find the offer")
	assertEqual(t, found.ID, offer.ID, "ID should match")
}

func TestOfferRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	found, err := repo.GetByID(ctx, "non-existent-id")
	assertNoError(t, err, "GetByID should not error for missing record")
	assertNil(t, found, "Should return nil for non-existent ID")
}

func TestOfferRepo_GetActive_Pagination(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	// Create 5 active offers
	for i := 0; i < 5; i++ {
		offer := CreateTestOfferWithRawMessage(t, db)
		assertNoError(t, repo.Save(ctx, offer), "Save should succeed")
	}

	// Get first page
	page1, err := repo.GetActive(ctx, 2, 0)
	assertNoError(t, err, "GetActive page 1 should succeed")
	assertEqual(t, len(page1), 2, "Should have 2 offers on first page")

	// Get second page
	page2, err := repo.GetActive(ctx, 2, 2)
	assertNoError(t, err, "GetActive page 2 should succeed")
	assertEqual(t, len(page2), 2, "Should have 2 offers on second page")

	// Get third page
	page3, err := repo.GetActive(ctx, 2, 4)
	assertNoError(t, err, "GetActive page 3 should succeed")
	assertEqual(t, len(page3), 1, "Should have 1 offer on third page")
}

func TestOfferRepo_GetActive_ExcludesInactive(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	// Create active offer
	activeOffer := CreateTestOfferWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, activeOffer), "Save active should succeed")

	// Create expired offer
	expiredOffer := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.Status = entity.StatusExpired
	})
	assertNoError(t, repo.Save(ctx, expiredOffer), "Save expired should succeed")

	// Get active - should only return the active one
	active, err := repo.GetActive(ctx, 10, 0)
	assertNoError(t, err, "GetActive should succeed")
	assertEqual(t, len(active), 1, "Should have 1 active offer")
	assertEqual(t, active[0].ID, activeOffer.ID, "Should be the active offer")
}

func TestOfferRepo_UpdateStatus(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	offer := CreateTestOfferWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, offer), "Save should succeed")

	// Update status
	err := repo.UpdateStatus(ctx, offer.ID, entity.StatusMatched)
	assertNoError(t, err, "UpdateStatus should succeed")

	// Verify status change
	saved, err := repo.GetByID(ctx, offer.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Status, entity.StatusMatched, "Status should be updated")
}

func TestOfferRepo_CountActive(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	// Create 3 active offers
	for i := 0; i < 3; i++ {
		offer := CreateTestOfferWithRawMessage(t, db)
		assertNoError(t, repo.Save(ctx, offer), "Save should succeed")
	}

	// Create 1 expired offer
	expiredOffer := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.Status = entity.StatusExpired
	})
	assertNoError(t, repo.Save(ctx, expiredOffer), "Save expired should succeed")

	// Count active
	count, err := repo.CountActive(ctx)
	assertNoError(t, err, "CountActive should succeed")
	assertEqual(t, count, int64(3), "Should count only active offers")
}

func TestOfferRepo_GetActive_OrderByCreatedAtDesc(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewOfferRepo(db.DB)
	ctx := testCtx()

	// Create offers with different timestamps
	now := time.Now()
	offer1 := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.CreatedAt = now.Add(-2 * time.Hour)
	})
	offer2 := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.CreatedAt = now.Add(-1 * time.Hour)
	})
	offer3 := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.CreatedAt = now
	})

	// Save in correct order (with different raw messages)
	assertNoError(t, repo.Save(ctx, offer1), "Save offer1")
	assertNoError(t, repo.Save(ctx, offer2), "Save offer2")
	assertNoError(t, repo.Save(ctx, offer3), "Save offer3")

	// Get active - should be ordered by created_at DESC (newest first)
	active, err := repo.GetActive(ctx, 10, 0)
	assertNoError(t, err, "GetActive should succeed")
	assertEqual(t, len(active), 3, "Should have 3 offers")
	assertEqual(t, active[0].ID, offer3.ID, "First should be newest")
	assertEqual(t, active[2].ID, offer1.ID, "Last should be oldest")
}
