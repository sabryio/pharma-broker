package gorm

import (
	"testing"
	"time"
)

func TestUnmappedRepo_Save_Conflict(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewUnmappedRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	rawText := "Conflicting Medication 500mg"

	// Initial save
	err := repo.Save(ctx, rawText, "AI Output 1", "Msg 1", "Group A", "msg-1")
	assertNoError(t, err, "First save should succeed")

	// Verify count is 1
	saved, err := repo.GetByRawText(ctx, rawText)
	assertNoError(t, err, "GetByRawText should succeed")
	assertEqual(t, saved.Count, 1, "Initial count should be 1")

	// Wait a moment to ensure UpdatedAt changes markedly if DB precision is low, though Postgres is usually fine
	time.Sleep(10 * time.Millisecond)
	initialUpdate := saved.UpdatedAt

	// Second save with same raw text (conflict)
	err = repo.Save(ctx, rawText, "AI Output 2", "Msg 2", "Group B", "msg-2")
	assertNoError(t, err, "Second save should succeed (upsert)")

	// Verify count is incremented
	updated, err := repo.GetByRawText(ctx, rawText)
	assertNoError(t, err, "GetByRawText should succeed after update")
	assertEqual(t, updated.Count, 2, "Count should be incremented to 2")

	if !updated.UpdatedAt.After(initialUpdate) {
		t.Errorf("UpdatedAt should be updated. Got %v, Initial %v", updated.UpdatedAt, initialUpdate)
	}
}

func TestUnmappedRepo_GetPending(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewUnmappedRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create unreviewed items
	assertNoError(t, repo.Save(ctx, "Raw 1", "AI 1", "Msg 1", "Grp 1", "m1"), "Save 1")
	assertNoError(t, repo.Save(ctx, "Raw 2", "AI 2", "Msg 2", "Grp 1", "m2"), "Save 2")
	// Save Raw 1 again to increase count
	assertNoError(t, repo.Save(ctx, "Raw 1", "AI 1", "Msg 3", "Grp 1", "m3"), "Save 1 again")

	// Create reviewed item (should be ignored)
	assertNoError(t, repo.Save(ctx, "Reviewed Item", "AI R", "Msg R", "Grp 1", "mr"), "Save reviewed")
	item, _ := repo.GetByRawText(ctx, "Reviewed Item")
	assertNoError(t, repo.MarkReviewed(ctx, item.ID, "Approved Name", "admin"), "Mark reviewed")

	// Test GetPending
	pending, err := repo.GetPending(ctx, 10, 0)
	assertNoError(t, err, "GetPending should succeed")
	assertEqual(t, len(pending), 2, "Should have 2 pending items")

	// Verify order (Count DESC) - Raw 1 has count 2, Raw 2 has count 1
	if pending[0].RawText != "Raw 1" {
		t.Errorf("Expected first item to be 'Raw 1' (count 2), got '%s'", pending[0].RawText)
	}
}

func TestUnmappedRepo_MarkReviewed(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewUnmappedRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	rawText := "To Review"
	assertNoError(t, repo.Save(ctx, rawText, "AI", "Msg", "Grp", "id"), "Save")

	item, err := repo.GetByRawText(ctx, rawText)
	assertNoError(t, err, "Get item")

	err = repo.MarkReviewed(ctx, item.ID, "Approved Name", "reviewer-1")
	assertNoError(t, err, "MarkReviewed should succeed")

	// Verify update
	updated, err := repo.GetByRawText(ctx, rawText)
	assertNoError(t, err, "Get item updated")
	assertEqual(t, updated.Reviewed, true, "Should be reviewed")
	assertEqual(t, updated.ApprovedName, "Approved Name", "ApprovedName mismatch")
	assertEqual(t, updated.ReviewedBy, "reviewer-1", "ReviewedBy mismatch")
	assertNotNil(t, updated.ReviewedAt, "ReviewedAt should be set")
}

func TestUnmappedRepo_Counts(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewUnmappedRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// 2 pending
	assertNoError(t, repo.Save(ctx, "Pending 1", "AI", "Msg", "Grp", "m1"), "Save P1")
	assertNoError(t, repo.Save(ctx, "Pending 2", "AI", "Msg", "Grp", "m2"), "Save P2")

	// 1 reviewed
	assertNoError(t, repo.Save(ctx, "Reviewed 1", "AI", "Msg", "Grp", "m3"), "Save R1")
	item, _ := repo.GetByRawText(ctx, "Reviewed 1")
	repo.MarkReviewed(ctx, item.ID, "Appr", "admin")

	// Check Total Count
	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 3, "Total count should be 3")

	// Check Pending Count
	pendingCount, err := repo.CountPending(ctx)
	assertNoError(t, err, "CountPending should succeed")
	assertEqual(t, pendingCount, 2, "Pending count should be 2")
}
