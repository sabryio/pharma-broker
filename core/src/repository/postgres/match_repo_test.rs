//! Integration tests for MatchRepository
//! Mirrors: legacy/storage/gorm/match_repo_test.go

use chrono::Utc;

use crate::domain::MatchStatus;
use crate::repository::postgres::testing::{
    TestDb, new_test_match, new_test_offer, new_test_raw_message, new_test_request,
};
use crate::repository::postgres::{
    PostgresMatchRepo, PostgresOfferRepo, PostgresRawMessageRepo, PostgresRequestRepo,
};
use crate::repository::{
    MatchRepository, OfferRepository, RawMessageRepository, RequestRepository,
};

/// Helper to create offer and request with their raw messages
async fn create_offer_and_request(db: &TestDb) -> (crate::domain::Offer, crate::domain::Request) {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let offer_repo = PostgresOfferRepo::new(db.pool.clone());
    let request_repo = PostgresRequestRepo::new(db.pool.clone());

    // Create raw messages
    let offer_msg = new_test_raw_message();
    let request_msg = new_test_raw_message();
    raw_repo
        .save(&offer_msg)
        .await
        .expect("Save offer raw message");
    raw_repo
        .save(&request_msg)
        .await
        .expect("Save request raw message");

    // Create offer and request
    let offer = new_test_offer(&offer_msg.id);
    let request = new_test_request(&request_msg.id);
    offer_repo.save(&offer).await.expect("Save offer");
    request_repo.save(&request).await.expect("Save request");

    (offer, request)
}

/// Test saving a match (insert)
/// Mirrors: TestMatchRepo_Save_Insert
#[tokio::test]
async fn test_save_insert() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;
    let m = new_test_match(&offer.id, &request.id);

    let result = match_repo.save(&m).await;
    assert!(result.is_ok(), "Save match should succeed");

    // Verify it was saved
    let saved = match_repo.get_by_id(&m.id).await.expect("GetByID").unwrap();
    assert_eq!(saved.offer_id, offer.id, "OfferID should match");
    assert_eq!(saved.request_id, request.id, "RequestID should match");
}

/// Test saving a match (upsert on composite key)
/// Mirrors: TestMatchRepo_Save_Upsert_CompositeKey
#[tokio::test]
async fn test_save_upsert_composite_key() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;

    // Create first match
    let mut m = new_test_match(&offer.id, &request.id);
    match_repo
        .save(&m)
        .await
        .expect("First save should succeed");

    // Update and save again
    m.score = 0.95;
    m.reasoning = "Updated reasoning".to_string();
    match_repo.save(&m).await.expect("Upsert should succeed");

    // Verify update
    let saved = match_repo.get_by_id(&m.id).await.expect("GetByID").unwrap();
    assert!((saved.score - 0.95).abs() < 0.01, "Score should be updated");
}

/// Test GetByID not found
/// Mirrors: TestMatchRepo_GetByID_NotFound
#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let found = match_repo
        .get_by_id("non-existent-id")
        .await
        .expect("GetByID");
    assert!(found.is_none(), "Should return None for non-existent ID");
}

/// Test GetPending
/// Mirrors: TestMatchRepo_GetPending_WithPreload
#[tokio::test]
async fn test_get_pending() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;

    // Create pending match
    let mut m = new_test_match(&offer.id, &request.id);
    m.status = MatchStatus::Pending;
    match_repo.save(&m).await.expect("Save match");

    // Get pending
    let pending = match_repo.get_pending(10, 0).await.expect("GetPending");
    assert_eq!(pending.len(), 1, "Should have 1 pending match");
}

/// Test UpdateStatus to Confirmed
/// Mirrors: TestMatchRepo_UpdateStatus_Confirm
#[tokio::test]
async fn test_update_status_confirm() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;
    let m = new_test_match(&offer.id, &request.id);
    match_repo.save(&m).await.expect("Save match");

    // Confirm match
    match_repo
        .update_status(
            &m.id,
            MatchStatus::Confirmed,
            "OPERATOR",
            "Test confirmation note",
        )
        .await
        .expect("UpdateStatus should succeed");

    // Verify status and confirmed_at
    let saved = match_repo.get_by_id(&m.id).await.expect("GetByID").unwrap();
    assert_eq!(
        saved.status,
        MatchStatus::Confirmed,
        "Status should be CONFIRMED"
    );
    assert!(saved.confirmed_at.is_some(), "ConfirmedAt should be set");
}

/// Test UpdateStatus to Rejected
/// Mirrors: TestMatchRepo_UpdateStatus_Reject
#[tokio::test]
async fn test_update_status_reject() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;
    let m = new_test_match(&offer.id, &request.id);
    match_repo.save(&m).await.expect("Save match");

    // Reject match
    match_repo
        .update_status(
            &m.id,
            MatchStatus::Rejected,
            "OPERATOR",
            "Test rejection reason",
        )
        .await
        .expect("UpdateStatus should succeed");

    // Verify status
    let saved = match_repo.get_by_id(&m.id).await.expect("GetByID").unwrap();
    assert_eq!(
        saved.status,
        MatchStatus::Rejected,
        "Status should be REJECTED"
    );
}

/// Test GetPending excludes confirmed matches
/// Mirrors: TestMatchRepo_GetPending_ExcludesConfirmed
#[tokio::test]
async fn test_get_pending_excludes_confirmed() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    // Create pending match
    let (offer1, request1) = create_offer_and_request(&db).await;
    let mut pending_match = new_test_match(&offer1.id, &request1.id);
    pending_match.status = MatchStatus::Pending;
    match_repo
        .save(&pending_match)
        .await
        .expect("Save pending match");

    // Create confirmed match
    let (offer2, request2) = create_offer_and_request(&db).await;
    let mut confirmed_match = new_test_match(&offer2.id, &request2.id);
    confirmed_match.status = MatchStatus::Confirmed;
    confirmed_match.confirmed_at = Some(Utc::now());
    match_repo
        .save(&confirmed_match)
        .await
        .expect("Save confirmed match");

    // Get pending - should only return pending
    let pending = match_repo.get_pending(10, 0).await.expect("GetPending");
    assert_eq!(pending.len(), 1, "Should have 1 pending match");
    assert_eq!(
        pending[0].id, pending_match.id,
        "Should be the pending match"
    );
}

/// Test CountPending
#[tokio::test]
async fn test_count_pending() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    // Create 2 pending matches
    for _ in 0..2 {
        let (offer, request) = create_offer_and_request(&db).await;
        let m = new_test_match(&offer.id, &request.id);
        match_repo.save(&m).await.expect("Save match");
    }

    // Create 1 confirmed match
    let (offer, request) = create_offer_and_request(&db).await;
    let mut confirmed = new_test_match(&offer.id, &request.id);
    confirmed.status = MatchStatus::Confirmed;
    confirmed.confirmed_at = Some(Utc::now());
    match_repo.save(&confirmed).await.expect("Save confirmed");

    let count = match_repo.count_pending().await.expect("CountPending");
    assert_eq!(count, 2, "Should count only pending matches");
}

/// Test exists
#[tokio::test]
async fn test_exists() {
    let db = TestDb::new().await;
    let match_repo = PostgresMatchRepo::new(db.pool.clone());

    let (offer, request) = create_offer_and_request(&db).await;
    let m = new_test_match(&offer.id, &request.id);
    match_repo.save(&m).await.expect("Save match");

    // Should exist
    let exists = match_repo
        .exists(&offer.id, &request.id)
        .await
        .expect("Exists");
    assert!(exists, "Match should exist");

    // Should not exist
    let not_exists = match_repo
        .exists("fake-offer", "fake-request")
        .await
        .expect("Exists");
    assert!(!not_exists, "Match should not exist");
}
