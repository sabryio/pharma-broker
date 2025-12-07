package storage

import (
	"testing"
)

// =============================================================================
// ConfigRepo Tests
// =============================================================================

func TestConfigRepo_GetAll_Defaults(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Empty DB should return defaults
	cfg, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertNotNil(t, cfg, "Config should not be nil")
	assertEqual(t, cfg.AutoParseEnabled, true, "AutoParseEnabled should default to true")
	assertEqual(t, cfg.SkipOwnMessages, true, "SkipOwnMessages should default to true")
}

func TestConfigRepo_Set_Get(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Set a value
	err := repo.Set(ctx, "test_key", "test_value")
	assertNoError(t, err, "Set should succeed")

	// Get the value
	val, err := repo.Get(ctx, "test_key")
	assertNoError(t, err, "Get should succeed")
	assertEqual(t, val, "test_value", "Value should match")
}

func TestConfigRepo_Set_Overwrite(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Set initial value
	err := repo.Set(ctx, "test_key", "initial")
	assertNoError(t, err, "First Set should succeed")

	// Overwrite
	err = repo.Set(ctx, "test_key", "updated")
	assertNoError(t, err, "Second Set should succeed")

	// Verify
	val, err := repo.Get(ctx, "test_key")
	assertNoError(t, err, "Get should succeed")
	assertEqual(t, val, "updated", "Value should be updated")
}

func TestConfigRepo_Delete(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Set a value
	err := repo.Set(ctx, "test_key", "test_value")
	assertNoError(t, err, "Set should succeed")

	// Delete it
	err = repo.Delete(ctx, "test_key")
	assertNoError(t, err, "Delete should succeed")

	// Verify it's gone
	_, err = repo.Get(ctx, "test_key")
	if err == nil {
		t.Error("Expected error for deleted key, got nil")
	}
}

func TestConfigRepo_UpdateFromMap_MultipleKeys(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Update multiple keys
	values := map[string]any{
		"auto_parse_enabled": false,
		"skip_own_messages":  true,
		"admin_phone":        "+201234567890",
	}
	err := repo.UpdateFromMap(ctx, values)
	assertNoError(t, err, "UpdateFromMap should succeed")

	// Verify
	cfg, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, cfg.AutoParseEnabled, false, "AutoParseEnabled should be false")
	assertEqual(t, cfg.SkipOwnMessages, true, "SkipOwnMessages should be true")
	assertEqual(t, cfg.AdminPhone, "+201234567890", "AdminPhone should match")
}

func TestConfigRepo_GetAll_WithStoredValues(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewConfigRepo(db.GormDB)
	ctx := testCtx()

	// Store values
	assertNoError(t, repo.Set(ctx, "auto_parse_enabled", "false"), "Set auto_parse")
	assertNoError(t, repo.Set(ctx, "admin_phone", "+201111111111"), "Set admin_phone")

	// GetAll should reflect stored values
	cfg, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, cfg.AutoParseEnabled, false, "AutoParseEnabled should be false")
	assertEqual(t, cfg.AdminPhone, "+201111111111", "AdminPhone should match")
}
