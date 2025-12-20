//! Integration tests for FeedbackRecordRepository
//! Mirrors: legacy/storage/gorm/feedback_record_repo_test.go

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::repository::FeedbackRecordRepository;
use crate::repository::postgres::PostgresFeedbackRepo;
use crate::repository::postgres::testing::{TestDb, new_test_feedback_record};

/// Test saving a feedback record
/// Mirrors: TestFeedbackRecordRepo_Save
#[tokio::test]
async fn test_save() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    let match_id = Uuid::new_v4();
    let feedback = new_test_feedback_record(match_id, true);

    let result = repo.save(&feedback).await;
    assert!(result.is_ok(), "Save should succeed");

    // Verify
    let saved = repo
        .get_by_match_id(&match_id.to_string())
        .await
        .expect("GetByMatchID");
    assert!(saved.is_some(), "Saved feedback should not be nil");
    let saved = saved.unwrap();
    assert_eq!(saved.match_id, match_id, "MatchID should match");
}

/// Test GetByDateRange
/// Mirrors: TestFeedbackRecordRepo_GetByDateRange
#[tokio::test]
async fn test_get_by_date_range() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create 3 records
    let mut fb1 = new_test_feedback_record(Uuid::new_v4(), true);
    fb1.created_at = now - Duration::hours(2);
    repo.save(&fb1).await.expect("Save fb1");

    let mut fb2 = new_test_feedback_record(Uuid::new_v4(), true);
    fb2.created_at = now - Duration::hours(25); // Outside 24h range
    repo.save(&fb2).await.expect("Save fb2");

    let mut fb3 = new_test_feedback_record(Uuid::new_v4(), false);
    fb3.created_at = now - Duration::hours(1);
    repo.save(&fb3).await.expect("Save fb3");

    // Query last 24h
    let records = repo
        .get_by_date_range(now - Duration::hours(24), now)
        .await
        .expect("GetByDateRange should succeed");
    assert_eq!(records.len(), 2, "Should return 2 records");
}

/// Test GetFeedbackStats
/// Mirrors: TestFeedbackRecordRepo_GetFeedbackStats
#[tokio::test]
async fn test_get_feedback_stats() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    let now = Utc::now();

    // 1. High score confirmed
    let mut fb1 = new_test_feedback_record(Uuid::new_v4(), true);
    fb1.medication_score = 1.0;
    fb1.created_at = now;
    repo.save(&fb1).await.expect("Save 1");

    // 2. Low score confirmed
    let mut fb2 = new_test_feedback_record(Uuid::new_v4(), true);
    fb2.medication_score = 0.8;
    fb2.created_at = now;
    repo.save(&fb2).await.expect("Save 2");

    // 3. Low score rejected
    let mut fb3 = new_test_feedback_record(Uuid::new_v4(), false);
    fb3.medication_score = 0.2;
    fb3.created_at = now;
    repo.save(&fb3).await.expect("Save 3");

    // Get stats
    let stats = repo
        .get_stats(now - Duration::hours(1), now + Duration::hours(1))
        .await
        .expect("GetFeedbackStats should succeed");

    assert_eq!(stats.total_feedbacks, 3, "Total feedbacks");
    assert_eq!(stats.confirmed_count, 2, "Confirmed count");
    assert_eq!(stats.rejected_count, 1, "Rejected count");

    // Avg Medication score for confirmed: (1.0 + 0.8) / 2 = 0.9
    assert!(
        (stats.confirmed_avg_medication - 0.9).abs() < 0.01,
        "Confirmed Avg Medication"
    );
    // Avg Medication score for rejected: 0.2
    assert!(
        (stats.rejected_avg_medication - 0.2).abs() < 0.01,
        "Rejected Avg Medication"
    );
}

/// Test Count
#[tokio::test]
async fn test_count() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    // Create 3 feedback records
    for _ in 0..3 {
        let fb = new_test_feedback_record(Uuid::new_v4(), true);
        repo.save(&fb).await.expect("Save");
    }

    let count = repo.count().await.expect("Count");
    assert_eq!(count, 3, "Should count 3 records");
}

/// Test GetByMatchID not found
#[tokio::test]
async fn test_get_by_match_id_not_found() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    let result = repo
        .get_by_match_id(&Uuid::new_v4().to_string())
        .await
        .expect("GetByMatchID should not error");
    assert!(
        result.is_none(),
        "Should return None for non-existent match"
    );
}

/// Test confirmation rate calculation
#[tokio::test]
async fn test_confirmation_rate() {
    let db = TestDb::new().await;
    let repo = PostgresFeedbackRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create 3 confirmed, 1 rejected
    for _ in 0..3 {
        let mut fb = new_test_feedback_record(Uuid::new_v4(), true);
        fb.created_at = now;
        repo.save(&fb).await.expect("Save confirmed");
    }

    let mut rejected = new_test_feedback_record(Uuid::new_v4(), false);
    rejected.created_at = now;
    repo.save(&rejected).await.expect("Save rejected");

    let stats = repo
        .get_stats(now - Duration::hours(1), now + Duration::hours(1))
        .await
        .expect("GetStats");

    // Confirmation rate should be 75%
    assert!(
        (stats.confirmation_rate - 0.75).abs() < 0.01,
        "Confirmation rate should be 75%"
    );
}
