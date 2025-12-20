//! Integration tests for StatsRepository
//! Mirrors: legacy/storage/gorm/stats_repo_test.go

use chrono::Utc;

use crate::domain::{ItemStatus, MatchStatus};
use crate::repository::postgres::testing::{
    TestDb, new_test_group, new_test_match, new_test_offer, new_test_raw_message, new_test_request,
};
use crate::repository::postgres::{
    PostgresGroupRepo, PostgresMatchRepo, PostgresOfferRepo, PostgresRawMessageRepo,
    PostgresRequestRepo, PostgresStatsRepo,
};
use crate::repository::{
    GroupRepository, MatchRepository, OfferRepository, RawMessageRepository, RequestRepository,
    StatsRepository,
};

/// Helper to create offer with raw message
async fn create_offer(db: &TestDb) -> crate::domain::Offer {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let msg = new_test_raw_message();
    raw_repo.save(&msg).await.expect("Save raw message");
    new_test_offer(&msg.id)
}

/// Helper to create request with raw message
async fn create_request(db: &TestDb) -> crate::domain::Request {
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let msg = new_test_raw_message();
    raw_repo.save(&msg).await.expect("Save raw message");
    new_test_request(&msg.id)
}

/// Test GetStats
/// Mirrors: TestStatsRepo_GetStats
#[tokio::test]
async fn test_get_stats() {
    let db = TestDb::new().await;
    let offer_repo = PostgresOfferRepo::new(db.pool.clone());
    let request_repo = PostgresRequestRepo::new(db.pool.clone());
    let match_repo = PostgresMatchRepo::new(db.pool.clone());
    let group_repo = PostgresGroupRepo::new(db.pool.clone());
    let stats_repo = PostgresStatsRepo::new(db.pool.clone());

    // Create 3 active offers
    for _ in 0..3 {
        let offer = create_offer(&db).await;
        offer_repo.save(&offer).await.expect("Save offer");
    }

    // Create 2 active requests
    for _ in 0..2 {
        let request = create_request(&db).await;
        request_repo.save(&request).await.expect("Save request");
    }

    // Create 1 pending match
    let offer = create_offer(&db).await;
    let request = create_request(&db).await;
    offer_repo.save(&offer).await.expect("Save offer for match");
    request_repo
        .save(&request)
        .await
        .expect("Save request for match");
    let mut m = new_test_match(&offer.id, &request.id);
    m.status = MatchStatus::Pending;
    match_repo.save(&m).await.expect("Save match");

    // Create 2 monitored groups
    for _ in 0..2 {
        let mut group = new_test_group();
        group.monitored = true;
        group_repo.save(&group).await.expect("Save group");
    }

    // Get stats
    let stats = stats_repo
        .get_stats()
        .await
        .expect("GetStats should succeed");

    // Note: Offers include the one used for match (3 + 1 = 4)
    // Requests include the one used for match (2 + 1 = 3)
    assert_eq!(stats.active_offers, 4, "Should have 4 active offers");
    assert_eq!(stats.active_requests, 3, "Should have 3 active requests");
    assert_eq!(stats.pending_matches, 1, "Should have 1 pending match");
    assert_eq!(stats.monitored_groups, 2, "Should have 2 monitored groups");
}

/// Test GetStats on empty database
/// Mirrors: TestStatsRepo_GetStats_Empty
#[tokio::test]
async fn test_get_stats_empty() {
    let db = TestDb::new().await;
    let stats_repo = PostgresStatsRepo::new(db.pool.clone());

    // Get stats on empty DB
    let stats = stats_repo
        .get_stats()
        .await
        .expect("GetStats should succeed on empty DB");

    assert_eq!(stats.active_offers, 0, "Should have 0 active offers");
    assert_eq!(stats.active_requests, 0, "Should have 0 active requests");
    assert_eq!(stats.pending_matches, 0, "Should have 0 pending matches");
    assert_eq!(stats.monitored_groups, 0, "Should have 0 monitored groups");
}

/// Test stats exclude inactive items
#[tokio::test]
async fn test_stats_exclude_inactive() {
    let db = TestDb::new().await;
    let offer_repo = PostgresOfferRepo::new(db.pool.clone());
    let request_repo = PostgresRequestRepo::new(db.pool.clone());
    let stats_repo = PostgresStatsRepo::new(db.pool.clone());

    // Create active offer
    let active_offer = create_offer(&db).await;
    offer_repo
        .save(&active_offer)
        .await
        .expect("Save active offer");

    // Create expired offer
    let mut expired_offer = create_offer(&db).await;
    expired_offer.status = ItemStatus::Expired;
    offer_repo
        .save(&expired_offer)
        .await
        .expect("Save expired offer");

    // Create active request
    let active_request = create_request(&db).await;
    request_repo
        .save(&active_request)
        .await
        .expect("Save active request");

    // Create matched request
    let mut matched_request = create_request(&db).await;
    matched_request.status = ItemStatus::Matched;
    request_repo
        .save(&matched_request)
        .await
        .expect("Save matched request");

    let stats = stats_repo.get_stats().await.expect("GetStats");
    assert_eq!(stats.active_offers, 1, "Should only count active offers");
    assert_eq!(
        stats.active_requests, 1,
        "Should only count active requests"
    );
}

/// Test confirmed today count
#[tokio::test]
async fn test_confirmed_today() {
    let db = TestDb::new().await;
    let offer_repo = PostgresOfferRepo::new(db.pool.clone());
    let request_repo = PostgresRequestRepo::new(db.pool.clone());
    let match_repo = PostgresMatchRepo::new(db.pool.clone());
    let stats_repo = PostgresStatsRepo::new(db.pool.clone());

    // Create a confirmed match
    let offer = create_offer(&db).await;
    let request = create_request(&db).await;
    offer_repo.save(&offer).await.expect("Save offer");
    request_repo.save(&request).await.expect("Save request");

    let mut m = new_test_match(&offer.id, &request.id);
    m.status = MatchStatus::Confirmed;
    m.confirmed_at = Some(Utc::now());
    match_repo.save(&m).await.expect("Save confirmed match");

    let stats = stats_repo.get_stats().await.expect("GetStats");
    assert_eq!(stats.confirmed_today, 1, "Should have 1 confirmed today");
}

/// Test processed today count
#[tokio::test]
async fn test_processed_today() {
    let db = TestDb::new().await;
    let raw_repo = PostgresRawMessageRepo::new(db.pool.clone());
    let stats_repo = PostgresStatsRepo::new(db.pool.clone());

    // Create and process 3 messages
    for _ in 0..3 {
        let msg = new_test_raw_message();
        raw_repo.save(&msg).await.expect("Save message");
        raw_repo
            .mark_processed(&msg.id, None)
            .await
            .expect("Mark processed");
    }

    // Create 1 unprocessed message
    let unprocessed = new_test_raw_message();
    raw_repo.save(&unprocessed).await.expect("Save unprocessed");

    let stats = stats_repo.get_stats().await.expect("GetStats");
    assert_eq!(stats.processed_today, 3, "Should have 3 processed today");
}
