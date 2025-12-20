//! Integration tests for OfferRepository
//! Mirrors: legacy/storage/gorm/offer_repo_test.go

use chrono::{Duration, Utc};

use crate::domain::ItemStatus;
use crate::repository::OfferRepository;
use crate::repository::RawMessageRepository;
use crate::repository::postgres::PostgresOfferRepo;
use crate::repository::postgres::PostgresRawMessageRepo;
use crate::repository::postgres::testing::{TestDb, new_test_offer, new_test_raw_message};

/// Helper to create an offer with its required raw message
async fn create_offer_with_raw_message(db: &TestDb) -> crate::domain::Offer {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let msg = new_test_raw_message();
    raw_repo.save(&msg).await.expect("Save raw message");
    new_test_offer(&msg.id)
}

/// Test saving an offer (insert)
/// Mirrors: TestOfferRepo_Save_Insert
#[tokio::test]
async fn test_save_insert() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let offer = create_offer_with_raw_message(&db).await;
    let result = repo.save(&offer).await;
    assert!(result.is_ok(), "Save should succeed");

    // Verify it was saved
    let saved = repo
        .get_by_id(&offer.id)
        .await
        .expect("GetByID should succeed");
    assert!(saved.is_some(), "Saved offer should not be nil");
    let saved = saved.unwrap();
    assert_eq!(
        saved.medication, offer.medication,
        "Medication should match"
    );
    assert_eq!(saved.quantity, offer.quantity, "Quantity should match");
}

/// Test saving an offer (upsert)
/// Mirrors: TestOfferRepo_Save_Upsert
#[tokio::test]
async fn test_save_upsert() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let mut offer = create_offer_with_raw_message(&db).await;
    repo.save(&offer).await.expect("First save should succeed");

    // Update and save again (upsert)
    offer.quantity = 100.0;
    offer.price = 200.0;
    repo.save(&offer).await.expect("Upsert should succeed");

    // Verify update
    let saved = repo.get_by_id(&offer.id).await.expect("GetByID").unwrap();
    assert_eq!(saved.quantity, 100.0, "Quantity should be updated");
}

/// Test GetByID found
/// Mirrors: TestOfferRepo_GetByID_Found
#[tokio::test]
async fn test_get_by_id_found() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let offer = create_offer_with_raw_message(&db).await;
    repo.save(&offer).await.expect("Save should succeed");

    let found = repo
        .get_by_id(&offer.id)
        .await
        .expect("GetByID should succeed");
    assert!(found.is_some(), "Should find the offer");
    assert_eq!(found.unwrap().id, offer.id, "ID should match");
}

/// Test GetByID not found
/// Mirrors: TestOfferRepo_GetByID_NotFound
#[tokio::test]
async fn test_get_by_id_not_found() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let found = repo
        .get_by_id("non-existent-id")
        .await
        .expect("GetByID should not error");
    assert!(found.is_none(), "Should return None for non-existent ID");
}

/// Test GetActive pagination
/// Mirrors: TestOfferRepo_GetActive_Pagination
#[tokio::test]
async fn test_get_active_pagination() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create 5 active offers
    for _ in 0..5 {
        let offer = create_offer_with_raw_message(&db).await;
        repo.save(&offer).await.expect("Save should succeed");
    }

    // Get first page
    let page1 = repo.get_active(2, 0).await.expect("GetActive page 1");
    assert_eq!(page1.len(), 2, "Should have 2 offers on first page");

    // Get second page
    let page2 = repo.get_active(2, 2).await.expect("GetActive page 2");
    assert_eq!(page2.len(), 2, "Should have 2 offers on second page");

    // Get third page
    let page3 = repo.get_active(2, 4).await.expect("GetActive page 3");
    assert_eq!(page3.len(), 1, "Should have 1 offer on third page");
}

/// Test GetActive excludes inactive offers
/// Mirrors: TestOfferRepo_GetActive_ExcludesInactive
#[tokio::test]
async fn test_get_active_excludes_inactive() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create active offer
    let active_offer = create_offer_with_raw_message(&db).await;
    repo.save(&active_offer).await.expect("Save active");

    // Create expired offer
    let mut expired_offer = create_offer_with_raw_message(&db).await;
    expired_offer.status = ItemStatus::Expired;
    repo.save(&expired_offer).await.expect("Save expired");

    // Get active - should only return the active one
    let active = repo.get_active(10, 0).await.expect("GetActive");
    assert_eq!(active.len(), 1, "Should have 1 active offer");
    assert_eq!(active[0].id, active_offer.id, "Should be the active offer");
}

/// Test UpdateStatus
/// Mirrors: TestOfferRepo_UpdateStatus
#[tokio::test]
async fn test_update_status() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let offer = create_offer_with_raw_message(&db).await;
    repo.save(&offer).await.expect("Save should succeed");

    // Update status
    repo.update_status(&offer.id, ItemStatus::Matched)
        .await
        .expect("UpdateStatus");

    // Verify status change
    let saved = repo.get_by_id(&offer.id).await.expect("GetByID").unwrap();
    assert_eq!(
        saved.status,
        ItemStatus::Matched,
        "Status should be updated"
    );
}

/// Test CountActive
/// Mirrors: TestOfferRepo_CountActive
#[tokio::test]
async fn test_count_active() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create 3 active offers
    for _ in 0..3 {
        let offer = create_offer_with_raw_message(&db).await;
        repo.save(&offer).await.expect("Save should succeed");
    }

    // Create 1 expired offer
    let mut expired = create_offer_with_raw_message(&db).await;
    expired.status = ItemStatus::Expired;
    repo.save(&expired).await.expect("Save expired");

    // Count active
    let count = repo.count_active().await.expect("CountActive");
    assert_eq!(count, 3, "Should count only active offers");
}

/// Test GetActive ordering by created_at DESC
/// Mirrors: TestOfferRepo_GetActive_OrderByCreatedAtDesc
#[tokio::test]
async fn test_get_active_order_by_created_at_desc() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create offers with different timestamps
    let mut offer1 = create_offer_with_raw_message(&db).await;
    offer1.created_at = now - Duration::hours(2);

    let mut offer2 = create_offer_with_raw_message(&db).await;
    offer2.created_at = now - Duration::hours(1);

    let mut offer3 = create_offer_with_raw_message(&db).await;
    offer3.created_at = now;

    repo.save(&offer1).await.expect("Save offer1");
    repo.save(&offer2).await.expect("Save offer2");
    repo.save(&offer3).await.expect("Save offer3");

    // Get active - should be ordered by created_at DESC (newest first)
    let active = repo.get_active(10, 0).await.expect("GetActive");
    assert_eq!(active.len(), 3, "Should have 3 offers");
    assert_eq!(active[0].id, offer3.id, "First should be newest");
    assert_eq!(active[2].id, offer1.id, "Last should be oldest");
}

/// Test FindRecentDuplicate found
/// Mirrors: TestOfferRepo_FindRecentDuplicate_Found
#[tokio::test]
async fn test_find_recent_duplicate_found() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create an offer
    let mut offer = create_offer_with_raw_message(&db).await;
    offer.source_phone = "201234567890".to_string();
    offer.medication = "Aspirin".to_string();
    repo.save(&offer).await.expect("Save should succeed");

    // Search for duplicate within 10 minutes - should find it
    let found = repo
        .find_recent_duplicate("201234567890", "Aspirin", Duration::minutes(10))
        .await
        .expect("FindRecentDuplicate should succeed");
    assert!(found.is_some(), "Should find the duplicate");
    assert_eq!(
        found.unwrap().id,
        offer.id,
        "Should return the existing offer"
    );
}

/// Test FindRecentDuplicate not found - different sender
/// Mirrors: TestOfferRepo_FindRecentDuplicate_NotFound_DifferentSender
#[tokio::test]
async fn test_find_recent_duplicate_not_found_different_sender() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create an offer from one sender
    let mut offer = create_offer_with_raw_message(&db).await;
    offer.source_phone = "201234567890".to_string();
    offer.medication = "Aspirin".to_string();
    repo.save(&offer).await.expect("Save should succeed");

    // Search for duplicate from different sender - should NOT find it
    let found = repo
        .find_recent_duplicate("209999999999", "Aspirin", Duration::minutes(10))
        .await
        .expect("FindRecentDuplicate should succeed");
    assert!(
        found.is_none(),
        "Should NOT find duplicate from different sender"
    );
}

/// Test FindRecentDuplicate not found - different medication
/// Mirrors: TestOfferRepo_FindRecentDuplicate_NotFound_DifferentMedication
#[tokio::test]
async fn test_find_recent_duplicate_not_found_different_medication() {
    let db = TestDb::new().await;
    let repo = PostgresOfferRepo::new(db.pool.clone());

    // Create an offer for Aspirin
    let mut offer = create_offer_with_raw_message(&db).await;
    offer.source_phone = "201234567890".to_string();
    offer.medication = "Aspirin".to_string();
    repo.save(&offer).await.expect("Save should succeed");

    // Search for different medication - should NOT find it
    let found = repo
        .find_recent_duplicate("201234567890", "Paracetamol", Duration::minutes(10))
        .await
        .expect("FindRecentDuplicate should succeed");
    assert!(
        found.is_none(),
        "Should NOT find duplicate for different medication"
    );
}
