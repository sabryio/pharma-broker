package gorm

import (
	"pharmabroker/domain/entity"
	"testing"
)

func TestLeaderboardRepo_GetTopDemand(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewLeaderboardRepo(db.DB)
	// We use direct DB access or other repos to seed data
	// But since we are in gorm package, we can just use db.Conn.Create

	ctx, cancel := testCtx()
	defer cancel()

	// 1. Setup Data
	// Med A: 3 distinct requests, 1 offer -> Ratio 3.0
	createRequest(t, db, "Med A", "req-1")
	createRequest(t, db, "Med A", "req-2")
	createRequest(t, db, "Med A", "req-3")
	createOffer(t, db, "Med A", "off-1")

	// Med B: 1 request, 2 offers -> Ratio 0.5
	createRequest(t, db, "Med B", "req-4")
	createOffer(t, db, "Med B", "off-2")
	createOffer(t, db, "Med B", "off-3")

	// Med C: 2 requests, 0 offers -> Ratio 999.0 (High demand)
	createRequest(t, db, "Med C", "req-5")
	createRequest(t, db, "Med C", "req-6")

	// 2. Test GetTopDemand
	stats, err := repo.GetTopDemand(ctx, 10)
	assertNoError(t, err, "GetTopDemand should succeed")

	if len(stats) != 3 {
		t.Fatalf("Expected 3 items in leaderboard, got %d", len(stats))
	}

	// Expected Order:
	// 1. Med C (Ratio 999.0)
	// 2. Med A (Ratio 3.0)
	// 3. Med B (Ratio 0.5)

	assertEqual(t, stats[0].Medication, "Med C", "First should be Med C")
	assertEqual(t, stats[1].Medication, "Med A", "Second should be Med A")
	assertEqual(t, stats[2].Medication, "Med B", "Third should be Med B")

	assertEqual(t, stats[0].Trend, "UP", "Med C Trend should be UP")
	assertEqual(t, stats[1].Trend, "UP", "Med A Trend should be UP")
	assertEqual(t, stats[2].Trend, "STABLE", "Med B Trend should be STABLE (0.5 is edge case in code usually <0.5 is DOWN)")
	// Code: < 0.5 is DOWN. 0.5 is STABLE.
}

func TestLeaderboardRepo_GetDemandForMedication(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewLeaderboardRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	createRequest(t, db, "Target Med", "r1")
	createRequest(t, db, "Target Med", "r2")
	createOffer(t, db, "Target Med", "o1")

	stats, err := repo.GetDemandForMedication(ctx, "Target Med")
	assertNoError(t, err, "GetDemandForMedication should succeed")

	assertEqual(t, stats.RequestCount, 2, "Request count")
	assertEqual(t, stats.OfferCount, 1, "Offer count")
	assertEqual(t, stats.DemandRatio, 2.0, "Demand ratio")
}

func TestLeaderboardRepo_RefreshAndCached(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewLeaderboardRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	createRequest(t, db, "Cached Med", "r1")

	// Refresh
	err := repo.RefreshLeaderboard(ctx)
	assertNoError(t, err, "RefreshLeaderboard should succeed")

	// Get Cached
	stats, err := repo.GetCachedLeaderboard(ctx, 10)
	assertNoError(t, err, "GetCachedLeaderboard should succeed")

	if len(stats) != 1 {
		t.Fatalf("Expected 1 cached item, got %d", len(stats))
	}
	assertEqual(t, stats[0].Medication, "Cached Med", "Medication name")

	// Check Last Refresh Time
	lastTime, err := repo.GetLastRefreshTime(ctx)
	assertNoError(t, err, "GetLastRefreshTime should succeed")
	if lastTime.IsZero() {
		t.Error("Last refresh time should not be zero")
	}
}

// Helpers

func createRequest(t *testing.T, db *TestDB, med, id string) {
	req := CreateTestRequestWithRawMessage(t, db, func(r *entity.Request) {
		r.ID = id
		r.Medication = med
	})
	if err := db.Conn.Save(ToRequestModel(req)).Error; err != nil {
		t.Fatalf("Failed to save request: %v", err)
	}
}

func createOffer(t *testing.T, db *TestDB, med, id string) {
	off := CreateTestOfferWithRawMessage(t, db, func(o *entity.Offer) {
		o.ID = id
		o.Medication = med
	})
	if err := db.Conn.Save(ToOfferModel(off)).Error; err != nil {
		t.Fatalf("Failed to save offer: %v", err)
	}
}
