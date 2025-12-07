package storage

import (
	"testing"

	"pharmabroker/internal/domain"
)

// =============================================================================
// StatsRepo Tests
// =============================================================================

func TestStatsRepo_GetStats(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	offerRepo := NewOfferRepo(db.GormDB)
	requestRepo := NewRequestRepo(db.GormDB)
	matchRepo := NewMatchRepo(db.GormDB)
	groupRepo := NewGroupRepo(db.GormDB)
	statsRepo := NewStatsRepo(db.GormDB)
	ctx := testCtx()

	// Create 3 active offers
	for i := 0; i < 3; i++ {
		offer := CreateTestOfferWithRawMessage(t, db)
		assertNoError(t, offerRepo.Save(ctx, offer), "Save offer")
	}

	// Create 2 active requests
	for i := 0; i < 2; i++ {
		req := CreateTestRequestWithRawMessage(t, db)
		assertNoError(t, requestRepo.Save(ctx, req), "Save request")
	}

	// Create 1 pending match
	offer := CreateTestOfferWithRawMessage(t, db)
	request := CreateTestRequestWithRawMessage(t, db)
	assertNoError(t, offerRepo.Save(ctx, offer), "Save offer for match")
	assertNoError(t, requestRepo.Save(ctx, request), "Save request for match")
	match := NewTestMatch(offer.ID, request.ID, func(m *domain.Match) {
		m.Status = domain.MatchStatusPending
	})
	assertNoError(t, matchRepo.Save(ctx, match), "Save match")

	// Create 2 monitored groups
	for i := 0; i < 2; i++ {
		assertNoError(t, groupRepo.Save(ctx, NewTestGroup(func(g *domain.Group) {
			g.Monitored = true
		})), "Save group")
	}

	// Get stats
	stats, err := statsRepo.GetStats(ctx)
	assertNoError(t, err, "GetStats should succeed")

	// Note: Offers include the one used for match (3 + 1 = 4)
	// Requests include the one used for match (2 + 1 = 3)
	assertEqual(t, stats.ActiveOffers, int64(4), "Should have 4 active offers")
	assertEqual(t, stats.ActiveRequests, int64(3), "Should have 3 active requests")
	assertEqual(t, stats.PendingMatches, int64(1), "Should have 1 pending match")
	assertEqual(t, stats.MonitoredGroups, 2, "Should have 2 monitored groups")
}

func TestStatsRepo_GetStats_Empty(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	statsRepo := NewStatsRepo(db.GormDB)
	ctx := testCtx()

	// Get stats on empty DB
	stats, err := statsRepo.GetStats(ctx)
	assertNoError(t, err, "GetStats should succeed on empty DB")

	assertEqual(t, stats.ActiveOffers, int64(0), "Should have 0 active offers")
	assertEqual(t, stats.ActiveRequests, int64(0), "Should have 0 active requests")
	assertEqual(t, stats.PendingMatches, int64(0), "Should have 0 pending matches")
	assertEqual(t, stats.MonitoredGroups, 0, "Should have 0 monitored groups")
}

func TestStatsRepo_GetProcessedToday(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	rawMsgRepo := NewRawMessageRepo(db.GormDB)
	statsRepo := NewStatsRepo(db.GormDB)
	ctx := testCtx()

	// Create and process 3 messages
	for i := 0; i < 3; i++ {
		msg := NewTestRawMessage()
		assertNoError(t, rawMsgRepo.Save(ctx, msg), "Save message")
		assertNoError(t, rawMsgRepo.MarkProcessed(ctx, msg.ID, nil), "Mark processed")
	}

	// Create 1 unprocessed message
	unprocessed := NewTestRawMessage()
	assertNoError(t, rawMsgRepo.Save(ctx, unprocessed), "Save unprocessed")

	// Get processed today
	count, err := statsRepo.GetProcessedToday(ctx)
	assertNoError(t, err, "GetProcessedToday should succeed")
	assertEqual(t, count, int64(3), "Should have 3 processed today")
}
