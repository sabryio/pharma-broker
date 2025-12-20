//! Integration tests for MedicationMappingRepository
//! Mirrors legacy Go tests using testcontainers

use pgvector::Vector;

use crate::domain::MedicationMapping;
use crate::repository::MedicationMappingRepository;
use crate::repository::postgres::PostgresMedicationMappingRepo;
use crate::repository::postgres::testing::{TestDb, make_embedding, make_uniform_embedding};
use chrono::Utc;

/// Test saving and retrieving a mapping with embedding
/// Mirrors: TestMedicationMappingRepo_Save_Embedding in Go
#[tokio::test]
async fn test_save_embedding() {
    let db = TestDb::new().await;
    let repo = PostgresMedicationMappingRepo::new(db.pool.clone());

    // 1. Create mapping with embedding
    let mapping = MedicationMapping {
        id: uuid::Uuid::new_v4().to_string(),
        arabic_name: "ستربسلس".to_string(),
        english_name: "Strepsils".to_string(),
        synonyms: Some(vec!["Strepsils Honey".to_string()]),
        embedding: Some(Vector::from(make_uniform_embedding(0.1))),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Save
    repo.save(&mapping).await.expect("Save should succeed");

    // 2. Retrieve and verify embedding
    let all = repo.get_all(10, 0).await.expect("GetAll should succeed");
    assert!(!all.is_empty(), "Should have at least 1 mapping");

    let saved = all
        .iter()
        .find(|m| m.arabic_name == "ستربسلس")
        .expect("Should find saved mapping");

    assert!(saved.embedding.is_some(), "Embedding should be present");
    let emb = saved.get_embedding().unwrap();
    assert_eq!(emb.len(), 768, "Expected embedding length 768");
    assert!((emb[0] - 0.1).abs() < 0.01, "Embedding values mismatch");

    // 3. Update embedding
    let updated_mapping = MedicationMapping {
        embedding: Some(Vector::from(make_uniform_embedding(0.9))),
        ..mapping.clone()
    };
    repo.save(&updated_mapping)
        .await
        .expect("Update should succeed");

    let updated = repo.get_all(10, 0).await.expect("GetAll after update");
    let updated_saved = updated
        .iter()
        .find(|m| m.arabic_name == "ستربسلس")
        .expect("Should find updated");

    let updated_emb = updated_saved.get_embedding().unwrap();
    assert!((updated_emb[0] - 0.9).abs() < 0.01, "Update failed");
}

/// Test find_similar returns results ordered by cosine distance
/// Mirrors: TestMedicationMappingRepo_FindSimilar in Go
#[tokio::test]
async fn test_find_similar() {
    let db = TestDb::new().await;
    let repo = PostgresMedicationMappingRepo::new(db.pool.clone());

    // Add items with different embeddings
    // m1: [1, 0, 0...] -> Target
    // m2: [0, 1, 0...] -> Orthogonal (different)
    // m3: [0.9, 0.1, 0...] -> Close to m1
    let m1 = MedicationMapping {
        id: uuid::Uuid::new_v4().to_string(),
        arabic_name: "A".to_string(),
        english_name: "A".to_string(),
        synonyms: None,
        embedding: Some(Vector::from(make_embedding(&[1.0, 0.0, 0.0]))),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let m2 = MedicationMapping {
        id: uuid::Uuid::new_v4().to_string(),
        arabic_name: "B".to_string(),
        english_name: "B".to_string(),
        synonyms: None,
        embedding: Some(Vector::from(make_embedding(&[0.0, 1.0, 0.0]))),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let m3 = MedicationMapping {
        id: uuid::Uuid::new_v4().to_string(),
        arabic_name: "C".to_string(),
        english_name: "C".to_string(),
        synonyms: None,
        embedding: Some(Vector::from(make_embedding(&[0.9, 0.1, 0.0]))),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    repo.save(&m1).await.expect("Save m1");
    repo.save(&m2).await.expect("Save m2");
    repo.save(&m3).await.expect("Save m3");

    let count = repo.count().await.expect("Count");
    assert_eq!(count, 3, "Expected 3 items");

    // Search similar to m1 ([1, 0, 0])
    // Expected Order:
    // 1. A (Distance 0)
    // 2. C (Distance small)
    // 3. B (Distance large/1.0)
    let query_vec = make_embedding(&[1.0, 0.0, 0.0]);
    let results = repo
        .find_similar(&query_vec, 3)
        .await
        .expect("FindSimilar should succeed");

    assert!(
        results.len() >= 3,
        "Expected 3 results, got {}",
        results.len()
    );
    assert_eq!(results[0].arabic_name, "A", "Expected first result to be A");
    assert_eq!(
        results[1].arabic_name, "C",
        "Expected second result to be C"
    );
}

/// Test GetAll
/// Mirrors: TestMedicationMappingRepo_GetAll in Go
#[tokio::test]
async fn test_get_all() {
    let db = TestDb::new().await;
    let repo = PostgresMedicationMappingRepo::new(db.pool.clone());

    repo.save(&MedicationMapping {
        id: "1".to_string(),
        arabic_name: "A".to_string(),
        english_name: "A".to_string(),
        synonyms: None,
        embedding: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
    .await
    .expect("Save 1");

    repo.save(&MedicationMapping {
        id: "2".to_string(),
        arabic_name: "B".to_string(),
        english_name: "B".to_string(),
        synonyms: None,
        embedding: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
    .await
    .expect("Save 2");

    let all = repo.get_all(10, 0).await.expect("GetAll should succeed");
    assert_eq!(all.len(), 2, "Should return 2 items");
}

/// Test Save with Synonyms
/// Mirrors: TestMedicationMappingRepo_Save_Synonyms in Go
#[tokio::test]
async fn test_save_synonyms() {
    let db = TestDb::new().await;
    let repo = PostgresMedicationMappingRepo::new(db.pool.clone());

    let mapping = MedicationMapping {
        id: uuid::Uuid::new_v4().to_string(),
        arabic_name: "باي الكوفان".to_string(),
        english_name: "Bi-Alcofan".to_string(),
        synonyms: Some(vec!["BiAlcofan".to_string(), "Bi Alcofan".to_string()]),
        embedding: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    repo.save(&mapping).await.expect("Save should succeed");

    let all = repo.get_all(10, 0).await.expect("Get should succeed");
    let saved = all
        .iter()
        .find(|m| m.arabic_name == "باي الكوفان")
        .expect("Should find");

    let synonyms = saved.synonyms.as_ref().expect("Should have synonyms");
    assert_eq!(synonyms.len(), 2, "Should have 2 synonyms");
    assert_eq!(synonyms[0], "BiAlcofan", "Synonym mismatch");
}
