//! Integration tests for AuditLogRepository
//! Mirrors: legacy/storage/gorm/audit_repo_test.go

use chrono::{Duration, Utc};

use crate::repository::AuditLogRepository;
use crate::repository::postgres::PostgresAuditLogRepo;
use crate::repository::postgres::testing::{TestDb, new_test_audit_log};

/// Test saving an audit log
/// Mirrors: TestAuditRepo_Log
#[tokio::test]
async fn test_save() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    let log = new_test_audit_log("match_confirmed", "match-123");
    let result = repo.save(&log).await;
    assert!(result.is_ok(), "Save should succeed");
}

/// Test GetRecent
/// Mirrors: TestAuditRepo_GetRecent
#[tokio::test]
async fn test_get_recent() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    // Create 3 logs
    for i in 0..3 {
        let log = new_test_audit_log("report_generated", &format!("report-{}", i));
        repo.save(&log).await.expect("Save should succeed");
    }

    let logs = repo
        .get_recent(2, 0)
        .await
        .expect("GetRecent should succeed");
    assert_eq!(logs.len(), 2, "Should return 2 logs");
}

/// Test GetByAction
/// Mirrors: TestAuditRepo_GetByAction
#[tokio::test]
async fn test_get_by_action() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    // Log different actions
    repo.save(&new_test_audit_log("match_confirmed", "m1"))
        .await
        .expect("Log 1");
    repo.save(&new_test_audit_log("match_rejected", "m2"))
        .await
        .expect("Log 2");
    repo.save(&new_test_audit_log("match_confirmed", "m3"))
        .await
        .expect("Log 3");

    let logs = repo
        .get_by_action("match_confirmed", 10)
        .await
        .expect("GetByAction should succeed");
    assert_eq!(logs.len(), 2, "Should find 2 confirmed logs");
}

/// Test GetByEntity
/// Mirrors: TestAuditRepo_GetByEntity
#[tokio::test]
async fn test_get_by_entity() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    let target_id = "target-entity";
    let other_id = "other-entity";

    repo.save(&new_test_audit_log("group_disabled", target_id))
        .await
        .expect("Log target");
    repo.save(&new_test_audit_log("group_disabled", other_id))
        .await
        .expect("Log other");
    repo.save(&new_test_audit_log("group_enabled", target_id))
        .await
        .expect("Log target again");

    let logs = repo
        .get_by_entity("match", target_id, 10)
        .await
        .expect("GetByEntity should succeed");
    assert_eq!(logs.len(), 2, "Should find 2 logs for target entity");
}

/// Test Count
/// Mirrors: TestAuditRepo_Count
#[tokio::test]
async fn test_count() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    // Log 3 actions
    repo.save(&new_test_audit_log("match_confirmed", "1"))
        .await
        .expect("Log 1");
    repo.save(&new_test_audit_log("match_confirmed", "2"))
        .await
        .expect("Log 2");
    repo.save(&new_test_audit_log("match_confirmed", "3"))
        .await
        .expect("Log 3");

    let count = repo.count().await.expect("Count should succeed");
    assert_eq!(count, 3, "Should count 3 logs");
}

/// Test GetByActor
#[tokio::test]
async fn test_get_by_actor() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    // Create logs with different actors
    let mut log1 = new_test_audit_log("action1", "entity1");
    log1.actor = "user-1".to_string();
    repo.save(&log1).await.expect("Save log1");

    let mut log2 = new_test_audit_log("action2", "entity2");
    log2.actor = "user-2".to_string();
    repo.save(&log2).await.expect("Save log2");

    let mut log3 = new_test_audit_log("action3", "entity3");
    log3.actor = "user-1".to_string();
    repo.save(&log3).await.expect("Save log3");

    let logs = repo.get_by_actor("user-1", 10).await.expect("GetByActor");
    assert_eq!(logs.len(), 2, "Should find 2 logs for user-1");
}

/// Test GetByDateRange
#[tokio::test]
async fn test_get_by_date_range() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create logs at different times
    let mut log1 = new_test_audit_log("action1", "entity1");
    log1.created_at = now - Duration::hours(1);
    repo.save(&log1).await.expect("Save log1");

    let mut log2 = new_test_audit_log("action2", "entity2");
    log2.created_at = now - Duration::hours(25); // Outside 24h range
    repo.save(&log2).await.expect("Save log2");

    let logs = repo
        .get_by_date_range(now - Duration::hours(24), now, 10)
        .await
        .expect("GetByDateRange should succeed");
    assert_eq!(logs.len(), 1, "Should find 1 log in range");
}

/// Test DeleteBefore
#[tokio::test]
async fn test_delete_before() {
    let db = TestDb::new().await;
    let repo = PostgresAuditLogRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create old log
    let mut old_log = new_test_audit_log("old_action", "old_entity");
    old_log.created_at = now - Duration::days(30);
    repo.save(&old_log).await.expect("Save old log");

    // Create recent log
    let recent_log = new_test_audit_log("recent_action", "recent_entity");
    repo.save(&recent_log).await.expect("Save recent log");

    // Delete logs older than 7 days
    let cutoff = now - Duration::days(7);
    let deleted = repo.delete_before(&cutoff).await.expect("DeleteBefore");
    assert_eq!(deleted, 1, "Should delete 1 old log");

    // Verify only recent log remains
    let count = repo.count().await.expect("Count");
    assert_eq!(count, 1, "Should have 1 log remaining");
}
