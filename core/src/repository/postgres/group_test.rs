//! Integration tests for GroupRepository
//! Mirrors: legacy/storage/gorm/group_repo_test.go

use crate::repository::GroupRepository;
use crate::repository::postgres::PostgresGroupRepo;
use crate::repository::postgres::testing::{TestDb, new_test_group};

/// Test saving a group
/// Mirrors: TestGroupRepo_Save
#[tokio::test]
async fn test_save() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    let group = new_test_group();
    let result = repo.save(&group).await;
    assert!(result.is_ok(), "Save should succeed");

    // Verify it was saved
    let all = repo.get_all().await.expect("GetAll should succeed");
    assert_eq!(all.len(), 1, "Should have 1 group");
    assert_eq!(all[0].jid, group.jid, "JID should match");
}

/// Test GetAll
/// Mirrors: TestGroupRepo_GetAll
#[tokio::test]
async fn test_get_all() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    // Create 3 groups
    for _ in 0..3 {
        repo.save(&new_test_group())
            .await
            .expect("Save should succeed");
    }

    let all = repo.get_all().await.expect("GetAll should succeed");
    assert_eq!(all.len(), 3, "Should have 3 groups");
}

/// Test GetMonitored filters correctly
/// Mirrors: TestGroupRepo_GetMonitored_FiltersCorrectly
#[tokio::test]
async fn test_get_monitored_filters_correctly() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    // Create monitored group
    let mut monitored = new_test_group();
    monitored.monitored = true;
    repo.save(&monitored).await.expect("Save monitored");

    // Create non-monitored group
    let mut not_monitored = new_test_group();
    not_monitored.monitored = false;
    repo.save(&not_monitored).await.expect("Save not monitored");

    // Get monitored
    let groups = repo
        .get_monitored()
        .await
        .expect("GetMonitored should succeed");
    assert_eq!(groups.len(), 1, "Should have 1 monitored group");
    assert_eq!(
        groups[0].jid, monitored.jid,
        "Should be the monitored group"
    );
}

/// Test SetMonitored toggle
/// Mirrors: TestGroupRepo_SetMonitored_Toggle
#[tokio::test]
async fn test_set_monitored_toggle() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    // Create monitored group
    let mut group = new_test_group();
    group.monitored = true;
    repo.save(&group).await.expect("Save should succeed");

    // Disable monitoring
    repo.update_monitored(&group.jid, false)
        .await
        .expect("SetMonitored should succeed");

    // Verify
    let monitored = repo
        .get_monitored()
        .await
        .expect("GetMonitored should succeed");
    assert_eq!(monitored.len(), 0, "Should have no monitored groups");
}

/// Test GetByJID
#[tokio::test]
async fn test_get_by_jid() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    let group = new_test_group();
    repo.save(&group).await.expect("Save should succeed");

    // Found
    let found = repo
        .get_by_jid(&group.jid)
        .await
        .expect("GetByJID")
        .unwrap();
    assert_eq!(found.jid, group.jid, "JID should match");

    // Not found
    let not_found = repo
        .get_by_jid("non-existent@g.us")
        .await
        .expect("GetByJID");
    assert!(
        not_found.is_none(),
        "Should return None for non-existent JID"
    );
}

/// Test IsMonitored
#[tokio::test]
async fn test_is_monitored() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    // Create monitored group
    let mut monitored = new_test_group();
    monitored.monitored = true;
    repo.save(&monitored).await.expect("Save monitored");

    // Create non-monitored group
    let mut not_monitored = new_test_group();
    not_monitored.monitored = false;
    repo.save(&not_monitored).await.expect("Save not monitored");

    // Check
    let is_monitored = repo
        .is_monitored(&monitored.jid)
        .await
        .expect("IsMonitored");
    assert!(is_monitored, "Should be monitored");

    let is_not_monitored = repo
        .is_monitored(&not_monitored.jid)
        .await
        .expect("IsMonitored");
    assert!(!is_not_monitored, "Should not be monitored");

    // Non-existent returns false
    let non_existent = repo
        .is_monitored("non-existent@g.us")
        .await
        .expect("IsMonitored");
    assert!(!non_existent, "Non-existent should return false");
}

/// Test IncrementMessageCount
/// Mirrors: TestGroupRepo_IncrementMessageCount
#[tokio::test]
async fn test_increment_message_count() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    // Create group with 0 message count
    let mut group = new_test_group();
    group.message_count = 0;
    repo.save(&group).await.expect("Save should succeed");

    // Increment
    repo.increment_message_count(&group.jid)
        .await
        .expect("IncrementMessageCount");

    // Increment again
    repo.increment_message_count(&group.jid)
        .await
        .expect("Second IncrementMessageCount");

    // Verify
    let saved = repo
        .get_by_jid(&group.jid)
        .await
        .expect("GetByJID")
        .unwrap();
    assert_eq!(saved.message_count, 2, "MessageCount should be 2");
}

/// Test Delete
#[tokio::test]
async fn test_delete() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    let group = new_test_group();
    repo.save(&group).await.expect("Save should succeed");

    // Delete
    let deleted = repo
        .delete(&group.jid)
        .await
        .expect("Delete should succeed");
    assert!(deleted, "Should return true for deleted group");

    // Verify it's gone
    let found = repo.get_by_jid(&group.jid).await.expect("GetByJID");
    assert!(found.is_none(), "Should be deleted");

    // Delete non-existent returns false
    let not_deleted = repo.delete("non-existent@g.us").await.expect("Delete");
    assert!(!not_deleted, "Should return false for non-existent group");
}

/// Test UpdateLastMessage
#[tokio::test]
async fn test_update_last_message() {
    let db = TestDb::new().await;
    let repo = PostgresGroupRepo::new(db.pool.clone());

    let mut group = new_test_group();
    group.last_message = None;
    repo.save(&group).await.expect("Save should succeed");

    // Update last message
    repo.update_last_message(&group.jid)
        .await
        .expect("UpdateLastMessage");

    // Verify
    let saved = repo
        .get_by_jid(&group.jid)
        .await
        .expect("GetByJID")
        .unwrap();
    assert!(saved.last_message.is_some(), "LastMessage should be set");
}
