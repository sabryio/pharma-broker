//! Integration tests for RequestRepository
//! Mirrors: legacy/storage/gorm/request_repo_test.go

use chrono::{Duration, Utc};

use crate::domain::ItemStatus;
use crate::repository::RawMessageRepository;
use crate::repository::RequestRepository;
use crate::repository::postgres::PostgresRawMessageRepo;
use crate::repository::postgres::PostgresRequestRepo;
use crate::repository::postgres::testing::{TestDb, new_test_raw_message, new_test_request};

/// Helper to create a request with its required raw message
async fn create_request_with_raw_message(db: &TestDb) -> crate::domain::Request {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let msg = new_test_raw_message();
    raw_repo.save(&msg).await.expect("Save raw message");
    new_test_request(&msg.id)
}

/// Test saving a request (insert)
/// Mirrors: TestRequestRepo_Save_Insert
#[tokio::test]
async fn test_save_insert() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let request = create_request_with_raw_message(&db).await;
    let result = repo.save(&request).await;
    assert!(result.is_ok(), "Save should succeed");

    // Verify it was saved
    let saved = repo.get_by_id(&request.id).await.expect("GetByID").unwrap();
    assert_eq!(
        saved.medication, request.medication,
        "Medication should match"
    );
}

/// Test saving a request (upsert)
/// Mirrors: TestRequestRepo_Save_Upsert
#[tokio::test]
async fn test_save_upsert() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let mut request = create_request_with_raw_message(&db).await;
    repo.save(&request)
        .await
        .expect("First save should succeed");

    // Update and save again (upsert)
    request.quantity = 50.0;
    request.max_price = 180.0;
    repo.save(&request).await.expect("Upsert should succeed");

    // Verify update
    let saved = repo.get_by_id(&request.id).await.expect("GetByID").unwrap();
    assert_eq!(saved.quantity, 50.0, "Quantity should be updated");
}

/// Test GetByID found
/// Mirrors: TestRequestRepo_GetByID_Found
#[tokio::test]
async fn test_get_by_id_found() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let request = create_request_with_raw_message(&db).await;
    repo.save(&request).await.expect("Save should succeed");

    let found = repo.get_by_id(&request.id).await.expect("GetByID").unwrap();
    assert_eq!(found.id, request.id, "ID should match");
}

/// Test GetByID not found
/// Mirrors: TestRequestRepo_GetByID_NotFound
#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let found = repo.get_by_id("non-existent-id").await.expect("GetByID");
    assert!(found.is_none(), "Should return None for non-existent ID");
}

/// Test GetActive returns urgent requests first
/// Mirrors: TestRequestRepo_GetActive_UrgentFirst
#[tokio::test]
async fn test_get_active_urgent_first() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    // Create non-urgent request first
    let mut non_urgent = create_request_with_raw_message(&db).await;
    non_urgent.urgent = false;
    repo.save(&non_urgent).await.expect("Save non-urgent");

    // Create urgent request second
    let mut urgent = create_request_with_raw_message(&db).await;
    urgent.urgent = true;
    repo.save(&urgent).await.expect("Save urgent");

    // Get active - urgent should come first
    let active = repo.get_active(10, 0).await.expect("GetActive");
    assert_eq!(active.len(), 2, "Should have 2 requests");
    assert!(active[0].urgent, "First should be urgent");
    assert!(!active[1].urgent, "Second should be non-urgent");
}

/// Test GetActive pagination
/// Mirrors: TestRequestRepo_GetActive_Pagination
#[tokio::test]
async fn test_get_active_pagination() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    // Create 5 requests
    for _ in 0..5 {
        let request = create_request_with_raw_message(&db).await;
        repo.save(&request).await.expect("Save should succeed");
    }

    // Get first page
    let page1 = repo.get_active(2, 0).await.expect("GetActive page 1");
    assert_eq!(page1.len(), 2, "Should have 2 requests on first page");

    // Get second page
    let page2 = repo.get_active(2, 2).await.expect("GetActive page 2");
    assert_eq!(page2.len(), 2, "Should have 2 requests on second page");
}

/// Test GetActive excludes inactive requests
/// Mirrors: TestRequestRepo_GetActive_ExcludesInactive
#[tokio::test]
async fn test_get_active_excludes_inactive() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    // Create active request
    let active_req = create_request_with_raw_message(&db).await;
    repo.save(&active_req).await.expect("Save active");

    // Create matched request
    let mut matched_req = create_request_with_raw_message(&db).await;
    matched_req.status = ItemStatus::Matched;
    repo.save(&matched_req).await.expect("Save matched");

    // Get active - should only return the active one
    let active = repo.get_active(10, 0).await.expect("GetActive");
    assert_eq!(active.len(), 1, "Should have 1 active request");
    assert_eq!(active[0].id, active_req.id, "Should be the active request");
}

/// Test UpdateStatus
/// Mirrors: TestRequestRepo_UpdateStatus
#[tokio::test]
async fn test_update_status() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let request = create_request_with_raw_message(&db).await;
    repo.save(&request).await.expect("Save should succeed");

    // Update status
    repo.update_status(&request.id, ItemStatus::Matched)
        .await
        .expect("UpdateStatus");

    // Verify status change
    let saved = repo.get_by_id(&request.id).await.expect("GetByID").unwrap();
    assert_eq!(
        saved.status,
        ItemStatus::Matched,
        "Status should be updated"
    );
}

/// Test CountActive
/// Mirrors: TestRequestRepo_CountActive
#[tokio::test]
async fn test_count_active() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    // Create 3 active requests
    for _ in 0..3 {
        let request = create_request_with_raw_message(&db).await;
        repo.save(&request).await.expect("Save should succeed");
    }

    // Create 1 expired request
    let mut expired = create_request_with_raw_message(&db).await;
    expired.status = ItemStatus::Expired;
    repo.save(&expired).await.expect("Save expired");

    // Count active
    let count = repo.count_active().await.expect("CountActive");
    assert_eq!(count, 3, "Should count only active requests");
}

/// Test GetActive ordering: urgent first, then by created_at DESC
/// Mirrors: TestRequestRepo_GetActive_UrgentThenByCreatedAt
#[tokio::test]
async fn test_get_active_urgent_then_by_created_at() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create older urgent
    let mut older_urgent = create_request_with_raw_message(&db).await;
    older_urgent.urgent = true;
    older_urgent.created_at = now - Duration::hours(2);

    // Create newer urgent
    let mut newer_urgent = create_request_with_raw_message(&db).await;
    newer_urgent.urgent = true;
    newer_urgent.created_at = now;

    // Create non-urgent
    let mut non_urgent = create_request_with_raw_message(&db).await;
    non_urgent.urgent = false;
    non_urgent.created_at = now - Duration::hours(1);

    repo.save(&older_urgent).await.expect("Save older urgent");
    repo.save(&newer_urgent).await.expect("Save newer urgent");
    repo.save(&non_urgent).await.expect("Save non-urgent");

    // Get active - urgent first (newest to oldest), then non-urgent
    let active = repo.get_active(10, 0).await.expect("GetActive");
    assert_eq!(active.len(), 3, "Should have 3 requests");
    assert_eq!(
        active[0].id, newer_urgent.id,
        "First should be newest urgent"
    );
    assert_eq!(
        active[1].id, older_urgent.id,
        "Second should be older urgent"
    );
    assert_eq!(active[2].id, non_urgent.id, "Third should be non-urgent");
}

/// Test FindRecentDuplicate
/// Mirrors: TestOfferRepo_FindRecentDuplicate_Found (adapted for requests)
#[tokio::test]
async fn test_find_recent_duplicate() {
    let db = TestDb::new().await;
    let repo = PostgresRequestRepo::new(db.pool.clone());

    // Create a request
    let mut request = create_request_with_raw_message(&db).await;
    request.source_phone = "201234567890".to_string();
    request.medication = "Aspirin".to_string();
    repo.save(&request).await.expect("Save should succeed");

    // Search for duplicate within 10 minutes - should find it
    let found = repo
        .find_recent_duplicate("201234567890", "Aspirin", Duration::minutes(10))
        .await
        .expect("FindRecentDuplicate should succeed");
    assert!(found.is_some(), "Should find the duplicate");
    assert_eq!(
        found.unwrap().id,
        request.id,
        "Should return the existing request"
    );
}
