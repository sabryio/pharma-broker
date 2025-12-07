package storage

import (
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// =============================================================================
// RequestRepo Tests
// =============================================================================

func TestRequestRepo_Save_Insert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	req := CreateTestRequestWithRawMessage(t, db)
	err := repo.Save(ctx, req)
	assertNoError(t, err, "Save should succeed")

	// Verify it was saved
	saved, err := repo.GetByID(ctx, req.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved, "Saved request should not be nil")
	assertEqual(t, saved.Medication, req.Medication, "Medication should match")
}

func TestRequestRepo_Save_Upsert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	req := CreateTestRequestWithRawMessage(t, db)
	err := repo.Save(ctx, req)
	assertNoError(t, err, "First save should succeed")

	// Update and save again (upsert)
	req.Quantity = 50
	req.MaxPrice = 180.0
	err = repo.Save(ctx, req)
	assertNoError(t, err, "Upsert should succeed")

	// Verify update
	saved, err := repo.GetByID(ctx, req.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Quantity, 50.0, "Quantity should be updated")
}

func TestRequestRepo_GetByID_Found(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	req := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, req), "Save should succeed")

	found, err := repo.GetByID(ctx, req.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, found, "Should find the request")
	assertEqual(t, found.ID, req.ID, "ID should match")
}

func TestRequestRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	found, err := repo.GetByID(ctx, "non-existent-id")
	assertNoError(t, err, "GetByID should not error for missing record")
	assertNil(t, found, "Should return nil for non-existent ID")
}

func TestRequestRepo_GetActive_UrgentFirst(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	// Create non-urgent request first
	nonUrgent := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Urgent = false
	})
	assertNoError(t, repo.Save(ctx, nonUrgent), "Save non-urgent")

	// Create urgent request second
	urgent := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Urgent = true
	})
	assertNoError(t, repo.Save(ctx, urgent), "Save urgent")

	// Get active - urgent should come first
	active, err := repo.GetActive(ctx, 10, 0)
	assertNoError(t, err, "GetActive should succeed")
	assertEqual(t, len(active), 2, "Should have 2 requests")
	assertEqual(t, active[0].Urgent, true, "First should be urgent")
	assertEqual(t, active[1].Urgent, false, "Second should be non-urgent")
}

func TestRequestRepo_GetActive_Pagination(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	// Create 5 requests
	for i := 0; i < 5; i++ {
		req := CreateTestRequestWithRawMessage(t, db)
		assertNoError(t, repo.Save(ctx, req), "Save should succeed")
	}

	// Get first page
	page1, err := repo.GetActive(ctx, 2, 0)
	assertNoError(t, err, "GetActive page 1 should succeed")
	assertEqual(t, len(page1), 2, "Should have 2 requests on first page")

	// Get second page
	page2, err := repo.GetActive(ctx, 2, 2)
	assertNoError(t, err, "GetActive page 2 should succeed")
	assertEqual(t, len(page2), 2, "Should have 2 requests on second page")
}

func TestRequestRepo_GetActive_ExcludesInactive(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	// Create active request
	activeReq := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, activeReq), "Save active should succeed")

	// Create matched request
	matchedReq := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Status = domain.StatusMatched
	})
	assertNoError(t, repo.Save(ctx, matchedReq), "Save matched should succeed")

	// Get active - should only return the active one
	active, err := repo.GetActive(ctx, 10, 0)
	assertNoError(t, err, "GetActive should succeed")
	assertEqual(t, len(active), 1, "Should have 1 active request")
	assertEqual(t, active[0].ID, activeReq.ID, "Should be the active request")
}

func TestRequestRepo_UpdateStatus(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	req := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, repo.Save(ctx, req), "Save should succeed")

	// Update status
	err := repo.UpdateStatus(ctx, req.ID, domain.StatusMatched)
	assertNoError(t, err, "UpdateStatus should succeed")

	// Verify status change
	saved, err := repo.GetByID(ctx, req.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertEqual(t, saved.Status, domain.StatusMatched, "Status should be updated")
}

func TestRequestRepo_CountActive(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	// Create 3 active requests
	for i := 0; i < 3; i++ {
		req := CreateTestRequestWithRawMessage(t, db)
		assertNoError(t, repo.Save(ctx, req), "Save should succeed")
	}

	// Create 1 expired request
	expiredReq := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Status = domain.StatusExpired
	})
	assertNoError(t, repo.Save(ctx, expiredReq), "Save expired should succeed")

	// Count active
	count, err := repo.CountActive(ctx)
	assertNoError(t, err, "CountActive should succeed")
	assertEqual(t, count, int64(3), "Should count only active requests")
}

func TestRequestRepo_GetActive_UrgentThenByCreatedAt(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRequestRepo(db.GormDB)
	ctx := testCtx()

	now := time.Now()

	// Create older urgent
	olderUrgent := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Urgent = true
		r.CreatedAt = now.Add(-2 * time.Hour)
	})
	// Create newer urgent
	newerUrgent := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Urgent = true
		r.CreatedAt = now
	})
	// Create non-urgent
	nonUrgent := CreateTestRequestWithRawMessage(t, db, func(r *domain.Request) {
		r.Urgent = false
		r.CreatedAt = now.Add(-1 * time.Hour)
	})

	assertNoError(t, repo.Save(ctx, olderUrgent), "Save older urgent")
	assertNoError(t, repo.Save(ctx, newerUrgent), "Save newer urgent")
	assertNoError(t, repo.Save(ctx, nonUrgent), "Save non-urgent")

	// Get active - urgent first (newest to oldest), then non-urgent
	active, err := repo.GetActive(ctx, 10, 0)
	assertNoError(t, err, "GetActive should succeed")
	assertEqual(t, len(active), 3, "Should have 3 requests")
	assertEqual(t, active[0].ID, newerUrgent.ID, "First should be newest urgent")
	assertEqual(t, active[1].ID, olderUrgent.ID, "Second should be older urgent")
	assertEqual(t, active[2].ID, nonUrgent.ID, "Third should be non-urgent")
}
