//! Integration tests for ReviewQueueRepository
//! Mirrors: legacy/storage/gorm/review_queue_repo_test.go

use chrono::{Duration, Utc};

use crate::domain::ReviewStatus;
use crate::repository::RawMessageRepository;
use crate::repository::ReviewQueueRepository;
use crate::repository::postgres::PostgresRawMessageRepo;
use crate::repository::postgres::PostgresReviewQueueRepo;
use crate::repository::postgres::testing::{
    TestDb, new_test_raw_message, new_test_review_queue_item,
};

/// Helper to create a review queue item with its required raw message
async fn create_review_item_with_raw_message(db: &TestDb) -> crate::domain::ReviewQueueItem {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let msg = new_test_raw_message();
    raw_repo.save(&msg).await.expect("Save raw message");
    new_test_review_queue_item(&msg.id)
}

/// Test saving a review queue item
/// Mirrors: TestReviewQueueRepo_Save
#[tokio::test]
async fn test_save() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    let item = create_review_item_with_raw_message(&db).await;
    let result = repo.save(&item).await;
    assert!(result.is_ok(), "Save should succeed");

    // Retrieve and verify
    let retrieved = repo
        .get_by_id(&item.id.to_string())
        .await
        .expect("GetByID")
        .unwrap();
    assert_eq!(
        retrieved.raw_message_id, item.raw_message_id,
        "RawMessageID should match"
    );
    assert_eq!(
        retrieved.status,
        ReviewStatus::Pending,
        "Status should be pending"
    );
}

/// Test GetByID not found
/// Mirrors: TestReviewQueueRepo_GetByID_NotFound
#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    let item = repo
        .get_by_id(&uuid::Uuid::new_v4().to_string())
        .await
        .expect("GetByID");
    assert!(item.is_none(), "Should return None for non-existent ID");
}

/// Test GetPending
/// Mirrors: TestReviewQueueRepo_GetPending
#[tokio::test]
async fn test_get_pending() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create 3 pending items
    for i in 0..3 {
        let mut item = create_review_item_with_raw_message(&db).await;
        item.status = ReviewStatus::Pending;
        item.created_at = now + Duration::seconds(i as i64);
        repo.save(&item).await.expect("Save pending");
    }

    // Create one approved item
    let mut approved = create_review_item_with_raw_message(&db).await;
    approved.status = ReviewStatus::Approved;
    repo.save(&approved).await.expect("Save approved");

    // Get pending items
    let pending = repo.get_pending(10, 0).await.expect("GetPending");
    assert_eq!(pending.len(), 3, "Should only return pending items");
}

/// Test CountPending
/// Mirrors: TestReviewQueueRepo_CountPending
#[tokio::test]
async fn test_count_pending() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Initial count should be 0
    let count = repo.count_pending().await.expect("CountPending");
    assert_eq!(count, 0, "Initial count should be 0");

    // Add pending items
    for _ in 0..5 {
        let item = create_review_item_with_raw_message(&db).await;
        repo.save(&item).await.expect("Save");
    }

    let count = repo.count_pending().await.expect("CountPending");
    assert_eq!(count, 5, "Should count 5 pending items");
}

/// Test UpdateStatus to Approved
/// Mirrors: TestReviewQueueRepo_Approve
#[tokio::test]
async fn test_update_status_approve() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Create pending item
    let item = create_review_item_with_raw_message(&db).await;
    repo.save(&item).await.expect("Save");

    // Approve
    repo.update_status(
        &item.id.to_string(),
        ReviewStatus::Approved,
        "admin@test.com",
        Some("Looks correct"),
    )
    .await
    .expect("UpdateStatus");

    // Verify
    let approved = repo
        .get_by_id(&item.id.to_string())
        .await
        .expect("GetByID")
        .unwrap();
    assert_eq!(
        approved.status,
        ReviewStatus::Approved,
        "Status should be approved"
    );
    assert_eq!(
        approved.reviewed_by,
        Some("admin@test.com".to_string()),
        "ReviewedBy should match"
    );
    assert!(approved.reviewed_at.is_some(), "ReviewedAt should be set");
}

/// Test UpdateStatus to Rejected
/// Mirrors: TestReviewQueueRepo_Reject
#[tokio::test]
async fn test_update_status_reject() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Create pending item
    let item = create_review_item_with_raw_message(&db).await;
    repo.save(&item).await.expect("Save");

    // Reject
    repo.update_status(
        &item.id.to_string(),
        ReviewStatus::Rejected,
        "moderator",
        Some("Not a medication message"),
    )
    .await
    .expect("UpdateStatus");

    // Verify
    let rejected = repo
        .get_by_id(&item.id.to_string())
        .await
        .expect("GetByID")
        .unwrap();
    assert_eq!(
        rejected.status,
        ReviewStatus::Rejected,
        "Status should be rejected"
    );
    assert_eq!(
        rejected.review_notes,
        Some("Not a medication message".to_string()),
        "ReviewNotes should match"
    );
}

/// Test GetStats
#[tokio::test]
async fn test_get_stats() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Create items with different statuses
    for _ in 0..3 {
        let item = create_review_item_with_raw_message(&db).await;
        repo.save(&item).await.expect("Save pending");
    }

    let mut approved = create_review_item_with_raw_message(&db).await;
    approved.status = ReviewStatus::Approved;
    repo.save(&approved).await.expect("Save approved");

    let mut rejected = create_review_item_with_raw_message(&db).await;
    rejected.status = ReviewStatus::Rejected;
    repo.save(&rejected).await.expect("Save rejected");

    let stats = repo.get_stats().await.expect("GetStats");
    assert_eq!(stats.total, 5, "Total should be 5");
    assert_eq!(stats.pending, 3, "Pending should be 3");
    assert_eq!(stats.approved, 1, "Approved should be 1");
    assert_eq!(stats.rejected, 1, "Rejected should be 1");
}

/// Test Pagination
/// Mirrors: TestReviewQueueRepo_Pagination
#[tokio::test]
async fn test_pagination() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Create 10 items
    for i in 0..10 {
        let mut item = create_review_item_with_raw_message(&db).await;
        item.created_at = Utc::now() + Duration::seconds(i as i64);
        repo.save(&item).await.expect("Save");
    }

    // Get first page
    let page1 = repo.get_pending(3, 0).await.expect("GetPending page 1");
    assert_eq!(page1.len(), 3, "Should have 3 items on first page");

    // Get second page
    let page2 = repo.get_pending(3, 3).await.expect("GetPending page 2");
    assert_eq!(page2.len(), 3, "Should have 3 items on second page");

    // Ensure different items
    assert_ne!(
        page1[0].id, page2[0].id,
        "Pages should have different items"
    );
}

/// Test ExistsForMessage
#[tokio::test]
async fn test_exists_for_message() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    let item = create_review_item_with_raw_message(&db).await;
    repo.save(&item).await.expect("Save");

    // Should exist
    let exists = repo
        .exists_for_message(&item.raw_message_id)
        .await
        .expect("ExistsForMessage");
    assert!(exists, "Should exist for saved message");

    // Should not exist
    let not_exists = repo
        .exists_for_message("non-existent-msg")
        .await
        .expect("ExistsForMessage");
    assert!(!not_exists, "Should not exist for non-existent message");
}

/// Test GetByStatus
#[tokio::test]
async fn test_get_by_status() {
    let db = TestDb::new().await;
    let repo = PostgresReviewQueueRepo::new(db.pool.clone());

    // Create items with different statuses
    for _ in 0..2 {
        let item = create_review_item_with_raw_message(&db).await;
        repo.save(&item).await.expect("Save pending");
    }

    let mut approved = create_review_item_with_raw_message(&db).await;
    approved.status = ReviewStatus::Approved;
    repo.save(&approved).await.expect("Save approved");

    // Get by status
    let pending = repo
        .get_by_status(ReviewStatus::Pending, 10, 0)
        .await
        .expect("GetByStatus");
    assert_eq!(pending.len(), 2, "Should have 2 pending items");

    let approved_items = repo
        .get_by_status(ReviewStatus::Approved, 10, 0)
        .await
        .expect("GetByStatus");
    assert_eq!(approved_items.len(), 1, "Should have 1 approved item");
}
