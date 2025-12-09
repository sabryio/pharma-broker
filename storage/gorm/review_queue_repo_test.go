package gorm

import (
	"context"
	"pharmabroker/domain/entity"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestReviewQueueRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)

	// Create raw message first
	rmRepo := NewRawMessageRepo(db.DB)
	rm := NewTestRawMessage(func(m *entity.RawMessage) {
		m.ID = "msg-123"
	})
	require.NoError(t, rmRepo.Save(context.Background(), rm))

	item := &entity.ReviewQueueItem{
		RawMessageID:  "msg-123",
		GroupName:     "Test Group",
		SenderName:    "Test Sender",
		Content:       "عندي اوجمنتين",
		ReplyContext:  "محتاج اوجمنتين",
		PartialItems:  []entity.ParsedItem{{Type: entity.MessageTypeOffer, Medication: "Augmentin"}},
		ParsePass:     1,
		AvgConfidence: 0.65,
		FailureReason: "Low confidence",
		Status:        entity.ReviewStatusPending,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}

	err := repo.Save(context.Background(), item)
	require.NoError(t, err)
	assert.NotEmpty(t, item.ID, "ID should be generated")

	// Retrieve and verify
	retrieved, err := repo.GetByID(context.Background(), item.ID)
	require.NoError(t, err)
	require.NotNil(t, retrieved)

	assert.Equal(t, item.RawMessageID, retrieved.RawMessageID)
	assert.Equal(t, item.GroupName, retrieved.GroupName)
	assert.Equal(t, item.Content, retrieved.Content)
	assert.Equal(t, item.ReplyContext, retrieved.ReplyContext)
	assert.Equal(t, entity.ReviewStatusPending, retrieved.Status)
	assert.Len(t, retrieved.PartialItems, 1)
	assert.Equal(t, "Augmentin", retrieved.PartialItems[0].Medication)
}

func TestReviewQueueRepo_GetByID_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)

	item, err := repo.GetByID(context.Background(), "non-existent-id")
	require.NoError(t, err)
	assert.Nil(t, item)
}

func TestReviewQueueRepo_GetPending(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Create 3 pending and 1 approved item
	rmRepo := NewRawMessageRepo(db.DB)
	for i := 0; i < 3; i++ {
		msgID := "msg-" + string(rune('a'+i))
		rm := NewTestRawMessage(func(m *entity.RawMessage) {
			m.ID = msgID
		})
		require.NoError(t, rmRepo.Save(ctx, rm))

		item := &entity.ReviewQueueItem{
			RawMessageID: msgID,
			Content:      "Test content",
			Status:       entity.ReviewStatusPending,
			CreatedAt:    time.Now().Add(time.Duration(i) * time.Second),
			UpdatedAt:    time.Now(),
		}
		require.NoError(t, repo.Save(ctx, item))
	}

	// Create one approved item
	rmApproved := NewTestRawMessage(func(m *entity.RawMessage) {
		m.ID = "msg-approved"
	})
	require.NoError(t, rmRepo.Save(ctx, rmApproved))

	approved := &entity.ReviewQueueItem{
		RawMessageID: "msg-approved",
		Content:      "Approved content",
		Status:       entity.ReviewStatusApproved,
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
	}
	require.NoError(t, repo.Save(ctx, approved))

	// Get pending items
	pending, err := repo.GetPending(ctx, 10, 0)
	require.NoError(t, err)
	assert.Len(t, pending, 3, "Should only return pending items")

	// Verify ordering (oldest first)
	assert.Equal(t, "msg-a", pending[0].RawMessageID)
}

func TestReviewQueueRepo_CountPending(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Initial count should be 0
	count, err := repo.CountPending(ctx)
	require.NoError(t, err)
	assert.Equal(t, int64(0), count)

	// Add pending items
	rmRepo := NewRawMessageRepo(db.DB)
	for i := 0; i < 5; i++ {
		msgID := "msg-" + string(rune('a'+i))
		rm := NewTestRawMessage(func(m *entity.RawMessage) {
			m.ID = msgID
		})
		require.NoError(t, rmRepo.Save(ctx, rm))

		item := &entity.ReviewQueueItem{
			RawMessageID: msgID,
			Content:      "Test",
			Status:       entity.ReviewStatusPending,
			CreatedAt:    time.Now(),
			UpdatedAt:    time.Now(),
		}
		require.NoError(t, repo.Save(ctx, item))
	}

	count, err = repo.CountPending(ctx)
	require.NoError(t, err)
	assert.Equal(t, int64(5), count)
}

func TestReviewQueueRepo_Approve(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Create pending item
	rmRepo := NewRawMessageRepo(db.DB)
	rm := NewTestRawMessage(func(m *entity.RawMessage) {
		m.ID = "msg-to-approve"
	})
	require.NoError(t, rmRepo.Save(ctx, rm))

	item := &entity.ReviewQueueItem{
		RawMessageID: "msg-to-approve",
		Content:      "Content to approve",
		Status:       entity.ReviewStatusPending,
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
	}
	require.NoError(t, repo.Save(ctx, item))

	// Approve with corrections
	correctedItems := []entity.ParsedItem{
		{Type: entity.MessageTypeOffer, Medication: "Corrected Medication"},
	}
	err := repo.Approve(ctx, item.ID, "admin@test.com", correctedItems, "Manually corrected")
	require.NoError(t, err)

	// Verify
	approved, err := repo.GetByID(ctx, item.ID)
	require.NoError(t, err)
	assert.Equal(t, entity.ReviewStatusApproved, approved.Status)
	assert.Equal(t, "admin@test.com", approved.ReviewedBy)
	assert.NotNil(t, approved.ReviewedAt)
	assert.Equal(t, "Manually corrected", approved.ReviewNote)
	assert.Len(t, approved.CorrectedItems, 1)
	assert.Equal(t, "Corrected Medication", approved.CorrectedItems[0].Medication)
}

func TestReviewQueueRepo_Reject(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Create pending item
	rmRepo := NewRawMessageRepo(db.DB)
	rm := NewTestRawMessage(func(m *entity.RawMessage) {
		m.ID = "msg-to-reject"
	})
	require.NoError(t, rmRepo.Save(ctx, rm))

	item := &entity.ReviewQueueItem{
		RawMessageID: "msg-to-reject",
		Content:      "Spam content",
		Status:       entity.ReviewStatusPending,
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
	}
	require.NoError(t, repo.Save(ctx, item))

	// Reject
	err := repo.Reject(ctx, item.ID, "moderator", "Not a medication message")
	require.NoError(t, err)

	// Verify
	rejected, err := repo.GetByID(ctx, item.ID)
	require.NoError(t, err)
	assert.Equal(t, entity.ReviewStatusRejected, rejected.Status)
	assert.Equal(t, "moderator", rejected.ReviewedBy)
	assert.Equal(t, "Not a medication message", rejected.ReviewNote)
}

func TestReviewQueueRepo_GetByRawMessageID(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Create item
	rmRepo := NewRawMessageRepo(db.DB)
	rm := NewTestRawMessage(func(m *entity.RawMessage) {
		m.ID = "unique-msg-id"
	})
	require.NoError(t, rmRepo.Save(ctx, rm))

	item := &entity.ReviewQueueItem{
		RawMessageID: "unique-msg-id",
		Content:      "Test content",
		Status:       entity.ReviewStatusPending,
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
	}
	require.NoError(t, repo.Save(ctx, item))

	// Find by raw message ID
	found, err := repo.GetByRawMessageID(ctx, "unique-msg-id")
	require.NoError(t, err)
	require.NotNil(t, found)
	assert.Equal(t, item.ID, found.ID)

	// Not found case
	notFound, err := repo.GetByRawMessageID(ctx, "non-existent")
	require.NoError(t, err)
	assert.Nil(t, notFound)
}

func TestReviewQueueRepo_Pagination(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewReviewQueueRepo(db.DB)
	ctx := context.Background()

	// Create 10 items
	rmRepo := NewRawMessageRepo(db.DB)
	for i := 0; i < 10; i++ {
		msgID := "msg-" + string(rune('a'+i))
		rm := NewTestRawMessage(func(m *entity.RawMessage) {
			m.ID = msgID
		})
		require.NoError(t, rmRepo.Save(ctx, rm))

		item := &entity.ReviewQueueItem{
			RawMessageID: msgID,
			Content:      "Test",
			Status:       entity.ReviewStatusPending,
			CreatedAt:    time.Now().Add(time.Duration(i) * time.Second),
			UpdatedAt:    time.Now(),
		}
		require.NoError(t, repo.Save(ctx, item))
	}

	// Get first page
	page1, err := repo.GetPending(ctx, 3, 0)
	require.NoError(t, err)
	assert.Len(t, page1, 3)

	// Get second page
	page2, err := repo.GetPending(ctx, 3, 3)
	require.NoError(t, err)
	assert.Len(t, page2, 3)

	// Ensure different items
	assert.NotEqual(t, page1[0].ID, page2[0].ID)
}
