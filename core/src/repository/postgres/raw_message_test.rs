//! Integration tests for RawMessageRepository
//! Mirrors: legacy/storage/gorm/raw_message_repo_test.go

use chrono::{Duration, Utc};

use crate::repository::RawMessageRepository;
use crate::repository::postgres::PostgresRawMessageRepo;
use crate::repository::postgres::testing::{TestDb, new_test_raw_message};

/// Test saving a raw message successfully
/// Mirrors: TestRawMessageRepo_Save_Success
#[tokio::test]
async fn test_save_success() {
    let db = TestDb::new().await;
    let repo = PostgresRawMessageRepo::new(db.pool.clone());

    let msg = new_test_raw_message();
    let result = repo.save(&msg).await;
    assert!(result.is_ok(), "Save should succeed");
}

/// Test duplicate external_id causes an error
/// Mirrors: TestRawMessageRepo_Save_Duplicate_ExternalID
/// Note: external_id has a unique constraint, so duplicate inserts fail
#[tokio::test]
async fn test_save_duplicate_external_id() {
    let db = TestDb::new().await;
    let repo = PostgresRawMessageRepo::new(db.pool.clone());

    let external_id = "shared-external-id".to_string();

    // Save first message
    let mut msg1 = new_test_raw_message();
    msg1.external_id = external_id.clone();
    repo.save(&msg1).await.expect("First save should succeed");

    // Save second message with same ExternalID (should fail due to unique constraint)
    let mut msg2 = new_test_raw_message();
    msg2.external_id = external_id;
    let result = repo.save(&msg2).await;
    assert!(result.is_err(), "Expected error for duplicate ExternalID");
}

/// Test getting unprocessed messages with FIFO ordering
/// Mirrors: TestRawMessageRepo_GetUnprocessed_Ordering
#[tokio::test]
async fn test_get_unprocessed_ordering() {
    let db = TestDb::new().await;
    let repo = PostgresRawMessageRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create messages with different timestamps (oldest first)
    let mut msg1 = new_test_raw_message();
    msg1.timestamp = now - Duration::hours(2);

    let mut msg2 = new_test_raw_message();
    msg2.timestamp = now - Duration::hours(1);

    let mut msg3 = new_test_raw_message();
    msg3.timestamp = now;

    // Save in random order
    repo.save(&msg2).await.expect("Save msg2");
    repo.save(&msg3).await.expect("Save msg3");
    repo.save(&msg1).await.expect("Save msg1");

    // Get unprocessed - should be ordered by timestamp ASC (FIFO)
    let unprocessed = repo
        .get_unprocessed(10)
        .await
        .expect("GetUnprocessed should succeed");
    assert_eq!(unprocessed.len(), 3, "Should have 3 unprocessed messages");

    // Verify FIFO ordering (oldest first)
    assert_eq!(unprocessed[0].id, msg1.id, "First should be oldest");
    assert_eq!(unprocessed[1].id, msg2.id, "Second should be middle");
    assert_eq!(unprocessed[2].id, msg3.id, "Third should be newest");
}

/// Test marking a message as processed
/// Mirrors: TestRawMessageRepo_MarkProcessed_Success
#[tokio::test]
async fn test_mark_processed_success() {
    let db = TestDb::new().await;
    let repo = PostgresRawMessageRepo::new(db.pool.clone());

    let msg = new_test_raw_message();
    repo.save(&msg).await.expect("Save should succeed");

    // Mark as processed
    let result = repo.mark_processed(&msg.id, None).await;
    assert!(result.is_ok(), "MarkProcessed should succeed");

    // Verify it's no longer in unprocessed
    let unprocessed = repo.get_unprocessed(10).await.expect("GetUnprocessed");
    assert!(
        unprocessed.is_empty(),
        "Should have no unprocessed messages"
    );
}

/// Test marking a message as processed with error
/// Mirrors: TestRawMessageRepo_MarkProcessed_WithError
#[tokio::test]
async fn test_mark_processed_with_error() {
    let db = TestDb::new().await;
    let repo = PostgresRawMessageRepo::new(db.pool.clone());

    let msg = new_test_raw_message();
    repo.save(&msg).await.expect("Save should succeed");

    // Mark as processed with error
    let result = repo.mark_processed(&msg.id, Some("parsing failed")).await;
    assert!(result.is_ok(), "MarkProcessed should succeed");

    // Verify it's no longer in unprocessed
    let unprocessed = repo.get_unprocessed(10).await.expect("GetUnprocessed");
    assert!(
        unprocessed.is_empty(),
        "Should have no unprocessed messages"
    );
}
