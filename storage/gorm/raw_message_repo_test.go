package gorm

import (
	"errors"
	"testing"
	"time"

	"pharmabroker/internal/domain"
)

// =============================================================================
// RawMessageRepo Tests
// =============================================================================

func TestRawMessageRepo_Save_Success(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	msg := NewTestRawMessage()
	err := repo.Save(ctx, msg)
	assertNoError(t, err, "Save should succeed")

	// Verify it was saved
	saved, err := repo.GetByID(ctx, msg.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved, "Saved message should not be nil")
	assertEqual(t, saved.Content, msg.Content, "Content should match")
}

func TestRawMessageRepo_Save_Duplicate_ExternalID(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	externalID := "shared-external-id"

	// Save first message
	msg1 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.ExternalID = externalID
	})
	err := repo.Save(ctx, msg1)
	assertNoError(t, err, "First save should succeed")

	// Save second message with same ExternalID (should fail due to unique constraint)
	msg2 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.ExternalID = externalID
	})
	err = repo.Save(ctx, msg2)
	if err == nil {
		t.Error("Expected error for duplicate ExternalID, got nil")
	}
}

func TestRawMessageRepo_GetByID_Found(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	msg := NewTestRawMessage()
	err := repo.Save(ctx, msg)
	assertNoError(t, err, "Save should succeed")

	found, err := repo.GetByID(ctx, msg.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, found, "Should find the message")
	assertEqual(t, found.ID, msg.ID, "ID should match")
	assertEqual(t, found.GroupJID, msg.GroupJID, "GroupJID should match")
}

func TestRawMessageRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	found, err := repo.GetByID(ctx, "non-existent-id")
	assertNoError(t, err, "GetByID should not error for missing record")
	assertNil(t, found, "Should return nil for non-existent ID")
}

func TestRawMessageRepo_GetUnprocessed_Ordering(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	// Create messages with different timestamps (oldest first)
	now := time.Now()
	msg1 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.Timestamp = now.Add(-2 * time.Hour)
	})
	msg2 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.Timestamp = now.Add(-1 * time.Hour)
	})
	msg3 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.Timestamp = now
	})

	// Save in random order
	assertNoError(t, repo.Save(ctx, msg2), "Save msg2")
	assertNoError(t, repo.Save(ctx, msg3), "Save msg3")
	assertNoError(t, repo.Save(ctx, msg1), "Save msg1")

	// Get unprocessed - should be ordered by timestamp ASC (FIFO)
	unprocessed, err := repo.GetUnprocessed(ctx, 10)
	assertNoError(t, err, "GetUnprocessed should succeed")
	assertEqual(t, len(unprocessed), 3, "Should have 3 unprocessed messages")

	// Verify FIFO ordering (oldest first)
	assertEqual(t, unprocessed[0].ID, msg1.ID, "First should be oldest")
	assertEqual(t, unprocessed[1].ID, msg2.ID, "Second should be middle")
	assertEqual(t, unprocessed[2].ID, msg3.ID, "Third should be newest")
}

func TestRawMessageRepo_MarkProcessed_Success(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	msg := NewTestRawMessage()
	assertNoError(t, repo.Save(ctx, msg), "Save should succeed")

	// Mark as processed
	err := repo.MarkProcessed(ctx, msg.ID, nil)
	assertNoError(t, err, "MarkProcessed should succeed")

	// Verify processed
	saved, err := repo.GetByID(ctx, msg.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved.ProcessedAt, "ProcessedAt should be set")
}

func TestRawMessageRepo_MarkProcessed_WithError(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	msg := NewTestRawMessage()
	assertNoError(t, repo.Save(ctx, msg), "Save should succeed")

	// Mark as processed with error
	processErr := errors.New("parsing failed")
	err := repo.MarkProcessed(ctx, msg.ID, processErr)
	assertNoError(t, err, "MarkProcessed should succeed")

	// Verify error field is set
	saved, err := repo.GetByID(ctx, msg.ID)
	assertNoError(t, err, "GetByID should succeed")
	assertNotNil(t, saved.ProcessedAt, "ProcessedAt should be set")
	if saved.Error == "" {
		t.Error("Error field should be set")
	}
}

func TestRawMessageRepo_GetLastMessageBySender(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewRawMessageRepo(db.DB)
	ctx := testCtx()

	groupJID := "test-group@g.us"
	senderJID := "sender@s.whatsapp.net"

	// Create multiple messages from same sender
	now := time.Now()
	msg1 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.GroupJID = groupJID
		m.SenderJID = senderJID
		m.Timestamp = now.Add(-1 * time.Hour)
	})
	msg2 := NewTestRawMessage(func(m *domain.RawMessage) {
		m.GroupJID = groupJID
		m.SenderJID = senderJID
		m.Timestamp = now // Latest
	})

	assertNoError(t, repo.Save(ctx, msg1), "Save msg1")
	assertNoError(t, repo.Save(ctx, msg2), "Save msg2")

	// Get last message
	last, err := repo.GetLastMessageBySender(ctx, groupJID, senderJID)
	assertNoError(t, err, "GetLastMessageBySender should succeed")
	assertNotNil(t, last, "Should find last message")
	assertEqual(t, last.ID, msg2.ID, "Should return the latest message")
}
