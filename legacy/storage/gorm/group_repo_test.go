package gorm

import (
	"testing"

	"pharmabroker/domain/entity"
)

// =============================================================================
// GroupRepo Tests
// =============================================================================

func TestGroupRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	group := NewTestGroup()
	err := repo.Save(ctx, group)
	assertNoError(t, err, "Save should succeed")

	// Verify it was saved
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 1, "Should have 1 group")
	assertEqual(t, all[0].JID, group.JID, "JID should match")
}

func TestGroupRepo_GetAll(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create 3 groups
	for i := 0; i < 3; i++ {
		assertNoError(t, repo.Save(ctx, NewTestGroup()), "Save should succeed")
	}

	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 3, "Should have 3 groups")
}

func TestGroupRepo_GetMonitored_FiltersCorrectly(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create monitored group (explicitly set)
	monitored := NewTestGroup(func(g *entity.Group) {
		g.Monitored = true
	})
	assertNoError(t, repo.Save(ctx, monitored), "Save monitored")

	// Create non-monitored group (explicitly set)
	notMonitored := NewTestGroup(func(g *entity.Group) {
		g.Monitored = false
	})
	assertNoError(t, repo.Save(ctx, notMonitored), "Save not monitored")

	// Get monitored
	groups, err := repo.GetMonitored(ctx)
	assertNoError(t, err, "GetMonitored should succeed")
	assertEqual(t, len(groups), 1, "Should have 1 monitored group")
	assertEqual(t, groups[0].JID, monitored.JID, "Should be the monitored group")
}

func TestGroupRepo_SetMonitored_Toggle(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create monitored group
	group := NewTestGroup(func(g *entity.Group) {
		g.Monitored = true
	})
	assertNoError(t, repo.Save(ctx, group), "Save should succeed")

	// Disable monitoring
	err := repo.SetMonitored(ctx, group.JID, false)
	assertNoError(t, err, "SetMonitored should succeed")

	// Verify
	monitored, err := repo.GetMonitored(ctx)
	assertNoError(t, err, "GetMonitored should succeed")
	assertEqual(t, len(monitored), 0, "Should have no monitored groups")
}

func TestGroupRepo_SaveFromSync_Insert(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	jid := "new-group@g.us"
	name := "New Group"
	description := "New group description"

	err := repo.SaveFromSync(ctx, jid, name, description)
	assertNoError(t, err, "SaveFromSync should succeed")

	// Verify it was created
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 1, "Should have 1 group")
	assertEqual(t, all[0].JID, jid, "JID should match")
	assertEqual(t, all[0].Name, name, "Name should match")
	// SaveFromSync sets Monitored to false by design (new groups need to be explicitly enabled)
	assertEqual(t, all[0].Monitored, false, "New groups from sync should not be monitored by default")
}

func TestGroupRepo_SaveFromSync_Update(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	jid := "existing-group@g.us"

	// Create group
	err := repo.SaveFromSync(ctx, jid, "Original Name", "Original desc")
	assertNoError(t, err, "First SaveFromSync should succeed")

	// Update with new name
	err = repo.SaveFromSync(ctx, jid, "Updated Name", "Updated desc")
	assertNoError(t, err, "Second SaveFromSync should succeed")

	// Verify update
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 1, "Should still have 1 group")
	assertEqual(t, all[0].Name, "Updated Name", "Name should be updated")
}

func TestGroupRepo_EnableFromConfig_Batch(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create 3 groups with Monitored=false
	jids := []string{}
	for i := 0; i < 3; i++ {
		group := NewTestGroup(func(g *entity.Group) {
			g.Monitored = false // Explicitly not monitored
		})
		assertNoError(t, repo.Save(ctx, group), "Save should succeed")
		jids = append(jids, group.JID)
	}

	// Verify none are monitored initially
	monitored, err := repo.GetMonitored(ctx)
	assertNoError(t, err, "GetMonitored should succeed")
	assertEqual(t, len(monitored), 0, "Should have 0 monitored groups initially")

	// Enable first 2
	count, err := repo.EnableFromConfig(ctx, jids[:2])
	assertNoError(t, err, "EnableFromConfig should succeed")
	assertEqual(t, count, 2, "Should enable 2 groups")

	// Verify
	monitored, err = repo.GetMonitored(ctx)
	assertNoError(t, err, "GetMonitored should succeed")
	assertEqual(t, len(monitored), 2, "Should have 2 monitored groups")
}

func TestGroupRepo_IncrementMessageCount(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewGroupRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create group with 0 message count
	group := NewTestGroup(func(g *entity.Group) {
		g.MessageCount = 0
	})
	assertNoError(t, repo.Save(ctx, group), "Save should succeed")

	// Increment
	err := repo.IncrementMessageCount(ctx, group.JID)
	assertNoError(t, err, "IncrementMessageCount should succeed")

	// Increment again
	err = repo.IncrementMessageCount(ctx, group.JID)
	assertNoError(t, err, "Second IncrementMessageCount should succeed")

	// Verify
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, all[0].MessageCount, int64(2), "MessageCount should be 2")
}
