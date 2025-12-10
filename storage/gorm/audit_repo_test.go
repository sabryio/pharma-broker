package gorm

import (
	"testing"
	"time"

	"pharmabroker/domain/entity"
)

// =============================================================================
// AuditRepo Tests
// =============================================================================

func TestAuditRepo_Log(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	err := repo.Log(ctx, entity.AuditMatchConfirmed, "match-123", "Match confirmed details")
	assertNoError(t, err, "Log should succeed")
}

func TestAuditRepo_LogWithValues(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	err := repo.LogWithValues(ctx, entity.AuditConfigChanged, "config-1", "old-val", "new-val", "Config updated")
	assertNoError(t, err, "LogWithValues should succeed")
}

func TestAuditRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	log := &entity.AuditLog{
		Action:    entity.AuditGroupEnabled,
		EntityID:  "group-123",
		Details:   "Group enabled manual",
		CreatedAt: time.Now(),
	}

	err := repo.Save(ctx, log)
	assertNoError(t, err, "Save should succeed")
}

func TestAuditRepo_GetRecent(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create 3 logs
	for i := 0; i < 3; i++ {
		err := repo.Log(ctx, entity.AuditReportGenerated, "report-id", "Generated")
		assertNoError(t, err, "Log should succeed")
	}

	logs, err := repo.GetRecent(ctx, 2)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 2, "Should return 2 logs")
}

func TestAuditRepo_GetByAction(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Log different actions
	assertNoError(t, repo.Log(ctx, entity.AuditMatchConfirmed, "m1", "c1"), "Log 1")
	assertNoError(t, repo.Log(ctx, entity.AuditMatchRejected, "m2", "r1"), "Log 2")
	assertNoError(t, repo.Log(ctx, entity.AuditMatchConfirmed, "m3", "c2"), "Log 3")

	logs, err := repo.GetByAction(ctx, entity.AuditMatchConfirmed, 10)
	assertNoError(t, err, "GetByAction should succeed")
	assertEqual(t, len(logs), 2, "Should find 2 confirmed logs")
}

func TestAuditRepo_GetByEntity(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	targetID := "target-entity"
	otherID := "other-entity"

	assertNoError(t, repo.Log(ctx, entity.AuditGroupDisabled, targetID, "disabled"), "Log target")
	assertNoError(t, repo.Log(ctx, entity.AuditGroupDisabled, otherID, "disabled"), "Log other")
	assertNoError(t, repo.Log(ctx, entity.AuditGroupEnabled, targetID, "enabled"), "Log target again")

	logs, err := repo.GetByEntity(ctx, targetID, 10)
	assertNoError(t, err, "GetByEntity should succeed")
	assertEqual(t, len(logs), 2, "Should find 2 logs for target entity")
}

func TestAuditRepo_Count(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Log 3 actions
	assertNoError(t, repo.Log(ctx, entity.AuditMatchConfirmed, "1", ""), "Log 1")
	assertNoError(t, repo.Log(ctx, entity.AuditMatchConfirmed, "2", ""), "Log 2")
	assertNoError(t, repo.Log(ctx, entity.AuditMatchConfirmed, "3", ""), "Log 3")

	count, err := repo.Count(ctx)
	assertNoError(t, err, "Count should succeed")
	assertEqual(t, count, 3, "Should count 3 logs")
}
