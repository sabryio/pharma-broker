//! Integration tests for WeightHistoryRepository
//! Mirrors: legacy/storage/gorm/weight_history_repo_test.go

use chrono::{Duration, Utc};

use crate::repository::WeightHistoryRepository;
use crate::repository::postgres::PostgresWeightHistoryRepo;
use crate::repository::postgres::testing::{TestDb, new_test_weight_history};

/// Test saving weight history
/// Mirrors: TestWeightHistoryRepo_Save
#[tokio::test]
async fn test_save() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let history = new_test_weight_history("manual");
    let result = repo.save(&history).await;
    assert!(result.is_ok(), "Save should succeed");

    // Verify it was saved
    let current = repo.get_current().await.expect("GetCurrent");
    assert!(current.is_some(), "Expected weight history, got nil");
    let current = current.unwrap();
    assert_eq!(current.id, history.id, "ID should match");
    assert!(
        (current.medication_weight - 0.35).abs() < 0.01,
        "MedicationWeight mismatch"
    );
    assert_eq!(current.source, "manual", "Source should be manual");
}

/// Test GetCurrent returns most recent
/// Mirrors: TestWeightHistoryRepo_GetCurrent
#[tokio::test]
async fn test_get_current() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create 3 weight configurations
    let mut h1 = new_test_weight_history("default");
    h1.medication_weight = 0.50;
    h1.created_at = now - Duration::hours(48);
    repo.save(&h1).await.expect("Save h1");

    let mut h2 = new_test_weight_history("manual");
    h2.medication_weight = 0.45;
    h2.created_at = now - Duration::hours(24);
    repo.save(&h2).await.expect("Save h2");

    let mut h3 = new_test_weight_history("auto_learned");
    h3.medication_weight = 0.40;
    h3.created_at = now;
    repo.save(&h3).await.expect("Save h3");

    // Get current should return the most recent one
    let current = repo.get_current().await.expect("GetCurrent").unwrap();
    assert_eq!(current.id, h3.id, "Expected most recent weight history");
    assert!(
        (current.medication_weight - 0.40).abs() < 0.01,
        "MedicationWeight mismatch"
    );
    assert_eq!(
        current.source, "auto_learned",
        "Source should be auto_learned"
    );
}

/// Test GetCurrent on empty table
/// Mirrors: TestWeightHistoryRepo_GetCurrent_Empty
#[tokio::test]
async fn test_get_current_empty() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let current = repo.get_current().await.expect("GetCurrent");
    assert!(current.is_none(), "Expected None for empty table");
}

/// Test GetHistory with limit
/// Mirrors: TestWeightHistoryRepo_GetHistory
#[tokio::test]
async fn test_get_history() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let now = Utc::now();

    // Create 5 weight configurations
    for i in 0..5 {
        let mut history = new_test_weight_history("manual");
        history.medication_weight = 0.40 + (i as f64) * 0.01;
        history.created_at = now - Duration::hours(i as i64);
        repo.save(&history).await.expect("Save");
    }

    // Get history with limit
    let histories = repo.get_history(3).await.expect("GetHistory");
    assert_eq!(histories.len(), 3, "Expected 3 histories");

    // Verify ordered by created_at DESC
    if histories.len() >= 2 {
        assert!(
            histories[0].created_at >= histories[1].created_at,
            "History should be ordered by created_at DESC"
        );
    }
}

/// Test GetById
#[tokio::test]
async fn test_get_by_id() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let history = new_test_weight_history("manual");
    repo.save(&history).await.expect("Save");

    // Found
    let found = repo
        .get_by_id(&history.id.to_string())
        .await
        .expect("GetById")
        .unwrap();
    assert_eq!(found.id, history.id, "ID should match");

    // Not found
    let not_found = repo
        .get_by_id(&uuid::Uuid::new_v4().to_string())
        .await
        .expect("GetById");
    assert!(
        not_found.is_none(),
        "Should return None for non-existent ID"
    );
}

/// Test Count
#[tokio::test]
async fn test_count() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    // Create 3 weight histories
    for _ in 0..3 {
        let history = new_test_weight_history("manual");
        repo.save(&history).await.expect("Save");
    }

    let count = repo.count().await.expect("Count");
    assert_eq!(count, 3, "Should count 3 histories");
}

/// Test weight values are preserved correctly
#[tokio::test]
async fn test_weight_values_preserved() {
    let db = TestDb::new().await;
    let repo = PostgresWeightHistoryRepo::new(db.pool.clone());

    let mut history = new_test_weight_history("manual");
    history.medication_weight = 0.40;
    history.dosage_weight = 0.15;
    history.quantity_weight = 0.20;
    history.price_weight = 0.15;
    history.recency_weight = 0.10;
    history.sample_count = 150;

    repo.save(&history).await.expect("Save");

    let saved = repo.get_current().await.expect("GetCurrent").unwrap();
    assert!(
        (saved.medication_weight - 0.40).abs() < 0.001,
        "medication_weight"
    );
    assert!((saved.dosage_weight - 0.15).abs() < 0.001, "dosage_weight");
    assert!(
        (saved.quantity_weight - 0.20).abs() < 0.001,
        "quantity_weight"
    );
    assert!((saved.price_weight - 0.15).abs() < 0.001, "price_weight");
    assert!(
        (saved.recency_weight - 0.10).abs() < 0.001,
        "recency_weight"
    );
    assert_eq!(saved.sample_count, 150, "sample_count");
}
