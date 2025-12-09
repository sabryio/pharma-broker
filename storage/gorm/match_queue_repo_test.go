package gorm

import (
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// =============================================================================
// MatchQueueRepo Tests
// =============================================================================

func TestMatchQueueRepo_Enqueue(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	item := &domain.MatchQueueItem{
		SourceType: "OFFER",
		SourceID:   "offer-123",
	}

	err := repo.Enqueue(ctx, item)
	assertNoError(t, err, "Enqueue should succeed")

	// Verify ID was generated
	if item.ID == "" {
		t.Error("ID should have been generated")
	}

	// Verify count
	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 1, "Should have 1 item in queue")
}

func TestMatchQueueRepo_Enqueue_WithID(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	customID := "custom-id-123"
	item := &domain.MatchQueueItem{
		ID:         customID,
		SourceType: "REQUEST",
		SourceID:   "request-123",
		CreatedAt:  time.Now(),
	}

	err := repo.Enqueue(ctx, item)
	assertNoError(t, err, "Enqueue should succeed")

	// Verify ID was preserved
	assertEqual(t, item.ID, customID, "ID should be preserved")
}

func TestMatchQueueRepo_DequeueBatch(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	// Enqueue 5 items
	for i := 0; i < 5; i++ {
		item := &domain.MatchQueueItem{
			SourceType: "OFFER",
			SourceID:   "offer-" + string(rune('a'+i)),
		}
		assertNoError(t, repo.Enqueue(ctx, item), "Enqueue should succeed")
	}

	// Dequeue batch of 3
	batch, err := repo.DequeueBatch(ctx, 3)
	assertNoError(t, err, "DequeueBatch should succeed")
	assertEqual(t, len(batch), 3, "Should get 3 items")

	// Items should still be in queue (DequeueBatch doesn't delete)
	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 5, "Queue should still have 5 items")
}

func TestMatchQueueRepo_DequeueBatch_FIFO(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	// Enqueue items with specific order
	now := time.Now()
	items := []*domain.MatchQueueItem{
		{SourceType: "OFFER", SourceID: "first", CreatedAt: now.Add(-2 * time.Second)},
		{SourceType: "OFFER", SourceID: "second", CreatedAt: now.Add(-1 * time.Second)},
		{SourceType: "OFFER", SourceID: "third", CreatedAt: now},
	}

	for _, item := range items {
		assertNoError(t, repo.Enqueue(ctx, item), "Enqueue should succeed")
	}

	// Dequeue should be FIFO (oldest first)
	batch, err := repo.DequeueBatch(ctx, 10)
	assertNoError(t, err, "DequeueBatch should succeed")
	assertEqual(t, batch[0].SourceID, "first", "First should be oldest")
	assertEqual(t, batch[2].SourceID, "third", "Last should be newest")
}

func TestMatchQueueRepo_Delete(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	// Enqueue an item
	item := &domain.MatchQueueItem{
		SourceType: "OFFER",
		SourceID:   "offer-123",
	}
	assertNoError(t, repo.Enqueue(ctx, item), "Enqueue should succeed")

	// Delete it
	err := repo.Delete(ctx, item.ID)
	assertNoError(t, err, "Delete should succeed")

	// Verify it's gone
	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 0, "Queue should be empty")
}

func TestMatchQueueRepo_Count_Empty(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 0, "Empty queue should have count 0")
}

func TestMatchQueueRepo_DequeueBatch_Empty(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMatchQueueRepo(db.DB)
	ctx := testCtx()

	batch, err := repo.DequeueBatch(ctx, 10)
	assertNoError(t, err, "DequeueBatch should succeed on empty queue")
	assertEqual(t, len(batch), 0, "Should return empty slice")
}
