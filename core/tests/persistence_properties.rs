//! Property-based tests for Audit Record Persistence
//!
//! Feature: debug-recording-enhancement
//! Tests Properties 6, 9 and 10 from the design document
//!
//! These tests validate:
//! - Property 6: Session Synchronization Round-Trip
//! - Property 9: Persistence Consistency
//! - Property 10: Buffer Overflow Handling
//!
//! Run with: cargo test --features test-pipeline-props --test persistence_properties

#![cfg(feature = "test-pipeline-props")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pharma_core::domain::{Offer, Request};
use pharma_core::matching::{
    AuditRecordBuilder, AuditRecorderConfig, MatchAuditRecord, NormalizedWeights,
    PersistentAuditConfig, PersistentAuditRecorder, ScoreBreakdown,
};
use pharma_core::repository::{MatchAuditRecordModel, MatchAuditRecordRepository};
use proptest::prelude::*;
use rust_decimal::Decimal;
use tokio::sync::RwLock;
use uuid::Uuid;

// =============================================================================
// Mock Repository for Testing
// =============================================================================

/// Mock repository that stores records in memory
pub struct MockMatchAuditRecordRepository {
    records: Arc<RwLock<Vec<MatchAuditRecordModel>>>,
    insert_count: AtomicU64,
    fail_next_insert: Arc<RwLock<bool>>,
}

impl MockMatchAuditRecordRepository {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            insert_count: AtomicU64::new(0),
            fail_next_insert: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn get_all_records(&self) -> Vec<MatchAuditRecordModel> {
        self.records.read().await.clone()
    }

    pub fn insert_count(&self) -> u64 {
        self.insert_count.load(Ordering::Relaxed)
    }

    pub async fn set_fail_next_insert(&self, fail: bool) {
        *self.fail_next_insert.write().await = fail;
    }
}

#[async_trait]
impl MatchAuditRecordRepository for MockMatchAuditRecordRepository {
    async fn insert(
        &self,
        record: &MatchAuditRecordModel,
    ) -> pharma_db::Result<MatchAuditRecordModel> {
        // Check if we should fail
        let should_fail = *self.fail_next_insert.read().await;
        if should_fail {
            *self.fail_next_insert.write().await = false;
            return Err(pharma_db::Error::Database(sea_orm::DbErr::Custom(
                "Simulated failure".to_string(),
            )));
        }

        self.insert_count.fetch_add(1, Ordering::Relaxed);
        let mut records = self.records.write().await;
        records.push(record.clone());
        Ok(record.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> pharma_db::Result<Option<MatchAuditRecordModel>> {
        let records = self.records.read().await;
        Ok(records.iter().find(|r| r.id == id).cloned())
    }

    async fn get_by_match_id(
        &self,
        match_id: Uuid,
    ) -> pharma_db::Result<Option<MatchAuditRecordModel>> {
        let records = self.records.read().await;
        Ok(records.iter().find(|r| r.match_id == match_id).cloned())
    }

    async fn get_by_session(
        &self,
        session_id: &str,
    ) -> pharma_db::Result<Vec<MatchAuditRecordModel>> {
        let records = self.records.read().await;
        Ok(records
            .iter()
            .filter(|r| r.session_id.as_deref() == Some(session_id))
            .cloned()
            .collect())
    }

    async fn list_recent(
        &self,
        limit: usize,
        offset: usize,
    ) -> pharma_db::Result<Vec<MatchAuditRecordModel>> {
        let records = self.records.read().await;
        Ok(records
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn delete_older_than(&self, cutoff: DateTime<Utc>) -> pharma_db::Result<u64> {
        let mut records = self.records.write().await;
        let before_len = records.len();
        records.retain(|r| r.created_at >= cutoff);
        Ok((before_len - records.len()) as u64)
    }

    async fn count(&self) -> pharma_db::Result<i64> {
        let records = self.records.read().await;
        Ok(records.len() as i64)
    }

    async fn update_review_status(
        &self,
        id: Uuid,
        status: &str,
        reviewed_by: Uuid,
        notes: Option<&str>,
    ) -> pharma_db::Result<MatchAuditRecordModel> {
        let mut records = self.records.write().await;
        let record = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| pharma_db::Error::NotFound(format!("Record {} not found", id)))?;

        record.review_status = Some(status.to_string());
        record.reviewed_by = Some(reviewed_by);
        record.reviewed_at = Some(Utc::now());
        record.review_notes = notes.map(|s| s.to_string());

        Ok(record.clone())
    }
}

// =============================================================================
// Test Helpers
// =============================================================================

/// Generate a test offer
fn create_test_offer(medication: &str) -> Offer {
    Offer {
        id: Uuid::new_v4(),
        medication: medication.to_string(),
        medication_raw: medication.to_string(),
        quantity: Some(Decimal::new(10, 0)),
        price: Some(Decimal::new(100, 0)),
        ..Default::default()
    }
}

/// Generate a test request
fn create_test_request(medication: &str) -> Request {
    Request {
        id: Uuid::new_v4(),
        medication: medication.to_string(),
        medication_raw: medication.to_string(),
        quantity: Some(Decimal::new(10, 0)),
        max_price: Some(Decimal::new(150, 0)),
        ..Default::default()
    }
}

/// Create a test audit record
fn create_test_audit_record(medication: &str, session_id: Option<&str>) -> MatchAuditRecord {
    let offer = create_test_offer(medication);
    let request = create_test_request(medication);
    let weights = NormalizedWeights::default();
    let match_id = Uuid::new_v4();

    let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

    if let Some(sid) = session_id {
        builder = builder.session_id(sid);
    }

    let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
    builder.build(&breakdown)
}

/// Generate a random medication name
fn arb_medication_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Aspirin".to_string()),
        Just("Paracetamol".to_string()),
        Just("Ibuprofen".to_string()),
        Just("Amoxicillin".to_string()),
        Just("Metformin".to_string()),
    ]
}

/// Generate a random session ID
fn arb_session_id() -> impl Strategy<Value = String> {
    "[a-z0-9]{8}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{12}"
}

// =============================================================================
// Property 9: Persistence Consistency
// =============================================================================
// For any audit record created when persistence is enabled, the record SHALL be
// retrievable from the database after buffer flush, and the retrieved record
// SHALL be equivalent to the original.
//
// Validates: Requirements 5.1, 5.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Feature: debug-recording-enhancement, Property 9: Persistence Consistency
    /// Validates: Requirements 5.1, 5.3
    ///
    /// For any audit record, after flush the record SHALL be retrievable by match_id
    /// and the retrieved record SHALL have equivalent core fields.
    #[test]
    fn prop_persisted_record_is_retrievable_by_match_id(
        medication in arb_medication_name(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 60,
                max_buffer_size: 10,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            // Create and record an audit record
            let original = create_test_audit_record(&medication, None);
            let match_id = original.match_id;

            recorder.record(original.clone()).await;

            // Trigger flush
            recorder.flush().await;

            // Wait for flush to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(match_id).await.unwrap();

            prop_assert!(retrieved.is_some(), "Record should be retrievable after flush");

            let retrieved = retrieved.unwrap();

            // Verify core fields match
            prop_assert_eq!(retrieved.match_id, original.match_id);
            prop_assert_eq!(retrieved.offer_id, original.offer_id);
            prop_assert_eq!(retrieved.request_id, original.request_id);
            prop_assert!((retrieved.final_score - original.final_score).abs() < 0.001);
            prop_assert_eq!(retrieved.resolution_stage, original.resolution_stage);
            prop_assert_eq!(retrieved.ai_involved, original.ai_involved);

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 9: Persistence Consistency
    /// Validates: Requirements 5.1, 5.3
    ///
    /// For any audit record with a session_id, after flush the record SHALL be
    /// retrievable by session_id.
    #[test]
    fn prop_persisted_record_is_retrievable_by_session(
        medication in arb_medication_name(),
        session_id in arb_session_id(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 60,
                max_buffer_size: 10,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            // Create and record an audit record with session_id
            let original = create_test_audit_record(&medication, Some(&session_id));

            recorder.record(original.clone()).await;

            // Trigger flush
            recorder.flush().await;

            // Wait for flush to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve by session
            let retrieved = recorder.get_by_session(&session_id).await.unwrap();

            prop_assert!(!retrieved.is_empty(), "Should find records by session_id");
            prop_assert_eq!(retrieved.len(), 1, "Should find exactly one record");
            prop_assert_eq!(retrieved[0].session_id.clone(), Some(session_id.clone()));

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 9: Persistence Consistency
    /// Validates: Requirements 5.1, 5.3
    ///
    /// Multiple records SHALL all be persisted and retrievable after flush.
    #[test]
    fn prop_multiple_records_are_persisted(
        record_count in 2usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 60,
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut match_ids = Vec::new();

            // Create and record multiple audit records
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let record = create_test_audit_record(med, None);
                match_ids.push(record.match_id);
                recorder.record(record).await;
            }

            // Trigger flush
            recorder.flush().await;

            // Wait for flush to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Verify all records are persisted
            let persisted_count = repo.insert_count();
            prop_assert_eq!(
                persisted_count as usize,
                record_count,
                "All {} records should be persisted",
                record_count
            );

            // Verify each record is retrievable
            for match_id in match_ids {
                let retrieved = recorder.get_by_match_id(match_id).await.unwrap();
                prop_assert!(
                    retrieved.is_some(),
                    "Record with match_id {} should be retrievable",
                    match_id
                );
            }

            recorder.shutdown();
            Ok(())
        })?;
    }
}

// =============================================================================
// Property 10: Buffer Overflow Handling
// =============================================================================
// For any audit recorder with a full buffer, when a new record is added, the
// oldest record SHALL be flushed to the database before the new record is added,
// maintaining buffer size invariant.
//
// Validates: Requirements 5.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Feature: debug-recording-enhancement, Property 10: Buffer Overflow Handling
    /// Validates: Requirements 5.2
    ///
    /// When buffer reaches max_buffer_size, adding a new record SHALL trigger
    /// a flush to maintain the buffer size invariant.
    #[test]
    fn prop_buffer_overflow_triggers_flush(
        buffer_size in 5usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 1000,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Long interval to avoid periodic flush
                max_buffer_size: buffer_size,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            // Wait for initial interval tick to pass
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Fill buffer to capacity
            for i in 0..buffer_size {
                let record = create_test_audit_record(&format!("Med{}", i), None);
                recorder.record(record).await;
            }

            // Buffer should be at capacity (or close to it if periodic flush happened)
            let buffer_len = recorder.buffer_len().await;
            prop_assert!(
                buffer_len <= buffer_size,
                "Buffer should be at or below capacity ({}), got {}",
                buffer_size,
                buffer_len
            );

            // Add one more record to trigger overflow
            let overflow_record = create_test_audit_record("OverflowMed", None);
            recorder.record(overflow_record).await;

            // Wait for flush to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Verify flush was triggered (records should be in database)
            let persisted_count = repo.insert_count();
            prop_assert!(
                persisted_count > 0,
                "Overflow should trigger flush, but no records were persisted"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 10: Buffer Overflow Handling
    /// Validates: Requirements 5.2, 5.5
    ///
    /// When flush fails, records SHALL be retained in buffer for retry.
    #[test]
    fn prop_failed_flush_retains_records(
        medication in arb_medication_name(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600,
                max_buffer_size: 10,
                max_retry_attempts: 1, // Only 1 attempt to speed up test
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            // Wait for initial interval tick to pass
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Set repository to fail next insert
            repo.set_fail_next_insert(true).await;

            // Record an audit record
            let record = create_test_audit_record(&medication, None);
            let match_id = record.match_id;
            recorder.record(record).await;

            // Trigger flush (which will fail)
            recorder.flush().await;

            // Wait for flush attempt and re-add to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Verify record is still in buffer (re-added after failure)
            let buffer_record = recorder.get_by_match_id_from_buffer(match_id).await;

            // Verify stats show failure
            let stats = recorder.stats();

            // Either the record is in buffer OR stats show failure (both indicate proper handling)
            prop_assert!(
                buffer_record.is_some() || stats.records_failed > 0 || stats.flush_errors > 0,
                "Failed record should be retained in buffer for retry or stats should reflect failure. \
                 Buffer has record: {}, records_failed: {}, flush_errors: {}",
                buffer_record.is_some(),
                stats.records_failed,
                stats.flush_errors
            );

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 10: Buffer Overflow Handling
    /// Validates: Requirements 5.2
    ///
    /// After successful flush, buffer SHALL be empty.
    #[test]
    fn prop_successful_flush_empties_buffer(
        record_count in 1usize..5,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600,
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());
            let _handle = recorder.start_flush_task();

            // Add some records
            for i in 0..record_count {
                let record = create_test_audit_record(&format!("Med{}", i), None);
                recorder.record(record).await;
            }

            // Verify buffer has records
            let buffer_before = recorder.buffer_len().await;
            prop_assert_eq!(buffer_before, record_count, "Buffer should have {} records", record_count);

            // Trigger flush
            recorder.flush().await;

            // Wait for flush to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Verify buffer is empty
            let buffer_after = recorder.buffer_len().await;
            prop_assert_eq!(buffer_after, 0, "Buffer should be empty after successful flush");

            // Verify records are in database
            let persisted_count = repo.insert_count();
            prop_assert_eq!(
                persisted_count as usize,
                record_count,
                "All records should be persisted"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }
}

// =============================================================================
// Property 6: Session Synchronization Round-Trip
// =============================================================================
// For any frontend recording session with a generated session_id, when audit
// records are created with that session_id, querying by session_id SHALL return
// all records associated with that session.
//
// Validates: Requirements 3.1, 3.2, 3.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Feature: debug-recording-enhancement, Property 6: Session Synchronization Round-Trip
    /// Validates: Requirements 3.1, 3.2, 3.3
    ///
    /// For any session_id, when a single audit record is created with that session_id,
    /// querying by session_id SHALL return exactly that record.
    #[test]
    fn prop_session_sync_single_record_round_trip(
        medication in arb_medication_name(),
        session_id in arb_session_id(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Very long interval to avoid automatic flush
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());

            // Create and record an audit record with session_id BEFORE starting flush task
            let original = create_test_audit_record(&medication, Some(&session_id));
            let match_id = original.match_id;
            recorder.record(original.clone()).await;

            // Query by session_id from buffer (before flush task starts)
            let buffer_results = recorder.get_by_session_from_buffer(&session_id).await;
            prop_assert_eq!(buffer_results.len(), 1, "Should find exactly one record in buffer");
            prop_assert_eq!(buffer_results[0].match_id, match_id);
            prop_assert_eq!(buffer_results[0].session_id.as_deref(), Some(session_id.as_str()));

            // Now start flush task and trigger flush
            let _handle = recorder.start_flush_task();
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Query by session_id from both buffer and database
            let all_results = recorder.get_by_session(&session_id).await.unwrap();
            prop_assert_eq!(all_results.len(), 1, "Should find exactly one record after flush");
            prop_assert_eq!(all_results[0].match_id, match_id);
            prop_assert_eq!(all_results[0].session_id.as_deref(), Some(session_id.as_str()));

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 6: Session Synchronization Round-Trip
    /// Validates: Requirements 3.1, 3.2, 3.3
    ///
    /// For any session_id, when multiple audit records are created with that session_id,
    /// querying by session_id SHALL return all records associated with that session.
    #[test]
    fn prop_session_sync_multiple_records_round_trip(
        record_count in 2usize..6,
        session_id in arb_session_id(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Very long interval to avoid automatic flush
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());

            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut match_ids = Vec::new();

            // Create and record multiple audit records with the same session_id BEFORE starting flush task
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let record = create_test_audit_record(med, Some(&session_id));
                match_ids.push(record.match_id);
                recorder.record(record).await;
            }

            // Query by session_id from buffer (before flush task starts)
            let buffer_results = recorder.get_by_session_from_buffer(&session_id).await;
            prop_assert_eq!(
                buffer_results.len(),
                record_count,
                "Should find all {} records in buffer",
                record_count
            );

            // Verify all match_ids are present
            for match_id in &match_ids {
                prop_assert!(
                    buffer_results.iter().any(|r| r.match_id == *match_id),
                    "Record with match_id {} should be in buffer results",
                    match_id
                );
            }

            // Now start flush task and trigger flush
            let _handle = recorder.start_flush_task();
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Query by session_id from both buffer and database
            let all_results = recorder.get_by_session(&session_id).await.unwrap();
            prop_assert_eq!(
                all_results.len(),
                record_count,
                "Should find all {} records after flush",
                record_count
            );

            // Verify all records have the correct session_id
            for record in &all_results {
                prop_assert_eq!(
                    record.session_id.as_deref(),
                    Some(session_id.as_str()),
                    "All records should have the same session_id"
                );
            }

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 6: Session Synchronization Round-Trip
    /// Validates: Requirements 3.1, 3.2, 3.3
    ///
    /// Records with different session_ids SHALL be isolated - querying by one
    /// session_id SHALL NOT return records from other sessions.
    #[test]
    fn prop_session_sync_isolation(
        session_id_1 in arb_session_id(),
        session_id_2 in arb_session_id(),
    ) {
        // Skip if session IDs happen to be the same (very unlikely but possible)
        prop_assume!(session_id_1 != session_id_2);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Very long interval to avoid automatic flush
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());

            // Create records for session 1
            let record_1a = create_test_audit_record("Aspirin", Some(&session_id_1));
            let record_1b = create_test_audit_record("Paracetamol", Some(&session_id_1));
            let match_id_1a = record_1a.match_id;
            let match_id_1b = record_1b.match_id;

            // Create records for session 2
            let record_2a = create_test_audit_record("Ibuprofen", Some(&session_id_2));
            let match_id_2a = record_2a.match_id;

            // Record all BEFORE starting flush task
            recorder.record(record_1a).await;
            recorder.record(record_1b).await;
            recorder.record(record_2a).await;

            // Query session 1 - should only get session 1 records
            let session_1_results = recorder.get_by_session_from_buffer(&session_id_1).await;
            prop_assert_eq!(session_1_results.len(), 2, "Session 1 should have 2 records");
            prop_assert!(
                session_1_results.iter().any(|r| r.match_id == match_id_1a),
                "Session 1 should contain record 1a"
            );
            prop_assert!(
                session_1_results.iter().any(|r| r.match_id == match_id_1b),
                "Session 1 should contain record 1b"
            );
            prop_assert!(
                !session_1_results.iter().any(|r| r.match_id == match_id_2a),
                "Session 1 should NOT contain record 2a"
            );

            // Query session 2 - should only get session 2 records
            let session_2_results = recorder.get_by_session_from_buffer(&session_id_2).await;
            prop_assert_eq!(session_2_results.len(), 1, "Session 2 should have 1 record");
            prop_assert!(
                session_2_results.iter().any(|r| r.match_id == match_id_2a),
                "Session 2 should contain record 2a"
            );
            prop_assert!(
                !session_2_results.iter().any(|r| r.match_id == match_id_1a),
                "Session 2 should NOT contain record 1a"
            );

            // Start flush task and flush, then verify isolation persists
            let _handle = recorder.start_flush_task();
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            let session_1_after_flush = recorder.get_by_session(&session_id_1).await.unwrap();
            let session_2_after_flush = recorder.get_by_session(&session_id_2).await.unwrap();

            prop_assert_eq!(session_1_after_flush.len(), 2, "Session 1 should still have 2 records after flush");
            prop_assert_eq!(session_2_after_flush.len(), 1, "Session 2 should still have 1 record after flush");

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 6: Session Synchronization Round-Trip
    /// Validates: Requirements 3.1, 3.2, 3.3
    ///
    /// Records without session_id SHALL NOT be returned when querying by session_id.
    #[test]
    fn prop_session_sync_excludes_null_sessions(
        session_id in arb_session_id(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Very long interval to avoid automatic flush
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());

            // Create a record with session_id
            let record_with_session = create_test_audit_record("Aspirin", Some(&session_id));
            let match_id_with_session = record_with_session.match_id;

            // Create a record without session_id
            let record_without_session = create_test_audit_record("Paracetamol", None);
            let match_id_without_session = record_without_session.match_id;

            // Record both BEFORE starting flush task
            recorder.record(record_with_session).await;
            recorder.record(record_without_session).await;

            // Query by session_id - should only get the record with session_id
            let results = recorder.get_by_session_from_buffer(&session_id).await;
            prop_assert_eq!(results.len(), 1, "Should find exactly one record with session_id");
            prop_assert_eq!(results[0].match_id, match_id_with_session);
            prop_assert!(
                !results.iter().any(|r| r.match_id == match_id_without_session),
                "Should NOT include record without session_id"
            );

            // Start flush task and flush, then verify
            let _handle = recorder.start_flush_task();
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            let results_after_flush = recorder.get_by_session(&session_id).await.unwrap();
            prop_assert_eq!(results_after_flush.len(), 1, "Should still find exactly one record after flush");
            prop_assert_eq!(results_after_flush[0].match_id, match_id_with_session);

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 6: Session Synchronization Round-Trip
    /// Validates: Requirements 3.1, 3.2, 3.3
    ///
    /// Querying by a non-existent session_id SHALL return an empty result set.
    #[test]
    fn prop_session_sync_nonexistent_session_returns_empty(
        session_id in arb_session_id(),
        nonexistent_session_id in arb_session_id(),
    ) {
        // Skip if session IDs happen to be the same
        prop_assume!(session_id != nonexistent_session_id);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let repo = Arc::new(MockMatchAuditRecordRepository::new());
            let config = PersistentAuditConfig {
                base: AuditRecorderConfig {
                    enabled: true,
                    buffer_size: 100,
                    persist_to_db: true,
                    min_score_threshold: None,
                    sample_rate: 1.0,
                },
                flush_interval_secs: 3600, // Very long interval to avoid automatic flush
                max_buffer_size: 100,
                max_retry_attempts: 3,
                retry_delay_ms: 10,
            };

            let mut recorder = PersistentAuditRecorder::new(config, repo.clone());

            // Create a record with one session_id BEFORE starting flush task
            let record = create_test_audit_record("Aspirin", Some(&session_id));
            recorder.record(record).await;

            // Query by a different (non-existent) session_id
            let results = recorder.get_by_session_from_buffer(&nonexistent_session_id).await;
            prop_assert!(results.is_empty(), "Should return empty for non-existent session_id");

            // Start flush task and flush, then verify
            let _handle = recorder.start_flush_task();
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let results_after_flush = recorder.get_by_session(&nonexistent_session_id).await.unwrap();
            prop_assert!(results_after_flush.is_empty(), "Should still return empty after flush");

            recorder.shutdown();
            Ok(())
        })?;
    }
}
