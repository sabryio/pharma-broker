package storage

import (
	"testing"
	"time"
)

// =============================================================================
// AuditRepo Tests
// =============================================================================

func TestAuditRepo_Log(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Log an action
	err := repo.Log(ctx, AuditMatchConfirmed, "match-123", "Match confirmed by operator")
	assertNoError(t, err, "Log should succeed")

	// Verify via GetRecent
	logs, err := repo.GetRecent(ctx, 10)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 1, "Should have 1 log")
	assertEqual(t, logs[0].Action, AuditMatchConfirmed, "Action should match")
	assertEqual(t, logs[0].EntityID, "match-123", "EntityID should match")
}

func TestAuditRepo_LogWithValues(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Log with old/new values
	err := repo.LogWithValues(ctx, AuditConfigChanged, "auto_parse_enabled", "false", "true", "Config updated")
	assertNoError(t, err, "LogWithValues should succeed")

	// Verify
	logs, err := repo.GetRecent(ctx, 10)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 1, "Should have 1 log")
	assertEqual(t, logs[0].OldValue, "false", "OldValue should match")
	assertEqual(t, logs[0].NewValue, "true", "NewValue should match")
}

func TestAuditRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	log := &AuditLog{
		ID:        "test-log-id",
		Action:    AuditConfigChanged,
		EntityID:  "config-key",
		OldValue:  "old-val",
		NewValue:  "new-val",
		Details:   "Auto-parse enabled",
		IPAddress: "127.0.0.1",
	}

	err := repo.Save(ctx, log)
	assertNoError(t, err, "Save should succeed")

	// Verify
	logs, err := repo.GetRecent(ctx, 10)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 1, "Should have 1 log")
	assertEqual(t, logs[0].OldValue, "old-val", "OldValue should match")
}

func TestAuditRepo_GetRecent_Ordering(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Use Save with explicit timestamps to ensure ordering
	now := time.Now()
	log1 := &AuditLog{ID: "log-1", Action: AuditMatchConfirmed, EntityID: "match-1", Details: "First", CreatedAt: now.Add(-2 * time.Second)}
	log2 := &AuditLog{ID: "log-2", Action: AuditMatchRejected, EntityID: "match-2", Details: "Second", CreatedAt: now.Add(-1 * time.Second)}
	log3 := &AuditLog{ID: "log-3", Action: AuditConfigChanged, EntityID: "config-1", Details: "Third", CreatedAt: now}

	assertNoError(t, repo.Save(ctx, log1), "Save first")
	assertNoError(t, repo.Save(ctx, log2), "Save second")
	assertNoError(t, repo.Save(ctx, log3), "Save third")

	// GetRecent should be ordered DESC (newest first)
	logs, err := repo.GetRecent(ctx, 10)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 3, "Should have 3 logs")
	assertEqual(t, logs[0].Details, "Third", "First should be newest")
	assertEqual(t, logs[1].Details, "Second", "Second should be middle")
	assertEqual(t, logs[2].Details, "First", "Last should be oldest")
}

func TestAuditRepo_GetByAction(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Log different actions
	assertNoError(t, repo.Log(ctx, AuditMatchConfirmed, "match-1", "Confirmed 1"), "Log confirmed")
	assertNoError(t, repo.Log(ctx, AuditMatchConfirmed, "match-2", "Confirmed 2"), "Log confirmed 2")
	assertNoError(t, repo.Log(ctx, AuditMatchRejected, "match-3", "Rejected"), "Log rejected")

	// Get only confirmed
	logs, err := repo.GetByAction(ctx, AuditMatchConfirmed, 10)
	assertNoError(t, err, "GetByAction should succeed")
	assertEqual(t, len(logs), 2, "Should have 2 confirmed logs")
}

func TestAuditRepo_GetByEntity(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Log multiple actions for same entity
	assertNoError(t, repo.Log(ctx, AuditMatchConfirmed, "match-123", "Action 1"), "Log 1")
	assertNoError(t, repo.LogWithValues(ctx, AuditConfigChanged, "match-123", "old", "new", "Action 2"), "Log 2")
	assertNoError(t, repo.Log(ctx, AuditMatchConfirmed, "match-456", "Other"), "Log other")

	// Get by entity
	logs, err := repo.GetByEntity(ctx, "match-123", 10)
	assertNoError(t, err, "GetByEntity should succeed")
	assertEqual(t, len(logs), 2, "Should have 2 logs for match-123")
}

func TestAuditRepo_GetRecent_Limit(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewAuditRepo(db.GormDB)
	ctx := testCtx()

	// Log 5 actions
	for i := 0; i < 5; i++ {
		assertNoError(t, repo.Log(ctx, AuditMatchConfirmed, "match", "Action"), "Log")
	}

	// Get with limit 3
	logs, err := repo.GetRecent(ctx, 3)
	assertNoError(t, err, "GetRecent should succeed")
	assertEqual(t, len(logs), 3, "Should respect limit")
}
