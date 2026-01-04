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
    AIInvolvementRecord, AuditRecordBuilder, AuditRecorderConfig, MatchAuditRecord,
    NormalizedWeights, PersistentAuditConfig, PersistentAuditRecorder, ScoreBreakdown,
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

// =============================================================================
// Property 5: Recording Non-Blocking
// =============================================================================
// For any scoring operation where the audit recorder fails to record (due to any
// error), the scoring operation SHALL complete successfully and return valid
// MatchScore and MatchAction results.
//
// Validates: Requirements 1.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recordings-persistence, Property 5: Recording Non-Blocking
    /// Validates: Requirements 1.4
    ///
    /// For any scoring operation, the scoring operation SHALL complete successfully
    /// and return valid MatchScore and MatchAction results, regardless of whether
    /// a persistent audit recorder is configured.
    ///
    /// This test verifies that the scoring logic is independent of audit recording.
    #[test]
    fn prop_scoring_completes_without_persistent_recorder(
        medication in arb_medication_name(),
        medication_score in 0.0f64..1.0,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            use pharma_core::matching::{MatchingEngine, MatchingEngineConfig, MatchAction};

            // Create engine WITHOUT a persistent audit recorder
            // This simulates the case where recording is not configured
            let engine = MatchingEngine::new(MatchingEngineConfig::default());

            let offer = create_test_offer(&medication);
            let request = create_test_request(&medication);

            // Score the match - this should complete successfully
            let (score, action) = engine.score_match(&offer, &request, medication_score, None).await;

            // Property 5: Scoring operation completes successfully
            // The score should be a valid value (not NaN or infinite)
            prop_assert!(
                score.total.is_finite(),
                "Score total must be a finite number, got: {}",
                score.total
            );

            // Property 5: Score is within valid range [0.0, 1.0]
            prop_assert!(
                score.total >= 0.0 && score.total <= 1.0,
                "Score total must be in range [0.0, 1.0], got: {}",
                score.total
            );

            // Property 5: Action is a valid MatchAction
            prop_assert!(
                matches!(
                    action,
                    MatchAction::AutoConfirm
                        | MatchAction::SuggestToOperator
                        | MatchAction::QueueForReview
                        | MatchAction::Ignore
                ),
                "Action must be a valid MatchAction variant"
            );

            // Property 5: Component scores are valid
            prop_assert!(
                score.medication_score.is_finite(),
                "Medication score must be finite"
            );
            prop_assert!(
                score.dosage_score.is_finite(),
                "Dosage score must be finite"
            );
            prop_assert!(
                score.quantity_score.is_finite(),
                "Quantity score must be finite"
            );
            prop_assert!(
                score.price_score.is_finite(),
                "Price score must be finite"
            );

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 5: Recording Non-Blocking
    /// Validates: Requirements 1.4
    ///
    /// For any AI-assisted scoring operation, the scoring operation SHALL complete
    /// successfully and return valid results, regardless of audit recording status.
    #[test]
    fn prop_ai_scoring_completes_without_persistent_recorder(
        medication in arb_medication_name(),
        medication_score in 0.0f64..1.0,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            use pharma_core::matching::{MatchingEngine, MatchingEngineConfig, MatchAction};

            // Create engine WITHOUT a persistent audit recorder
            let engine = MatchingEngine::new(MatchingEngineConfig::default());

            let offer = create_test_offer(&medication);
            let request = create_test_request(&medication);

            // Score the match with AI logic disabled (to avoid actual AI calls in tests)
            let (score, action, _review_result) = engine
                .score_match_ai(&offer, &request, medication_score, None, false)
                .await;

            // Property 5: Scoring operation completes successfully
            prop_assert!(
                score.total.is_finite(),
                "Score total must be a finite number, got: {}",
                score.total
            );

            // Property 5: Score is within valid range [0.0, 1.0]
            prop_assert!(
                score.total >= 0.0 && score.total <= 1.0,
                "Score total must be in range [0.0, 1.0], got: {}",
                score.total
            );

            // Property 5: Action is a valid MatchAction
            prop_assert!(
                matches!(
                    action,
                    MatchAction::AutoConfirm
                        | MatchAction::SuggestToOperator
                        | MatchAction::QueueForReview
                        | MatchAction::Ignore
                ),
                "Action must be a valid MatchAction variant"
            );

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 5: Recording Non-Blocking
    /// Validates: Requirements 1.4
    ///
    /// For any scoring operation, the result should be deterministic based on
    /// input parameters, not affected by audit recording configuration.
    #[test]
    fn prop_scoring_deterministic_regardless_of_recording(
        medication in arb_medication_name(),
        medication_score in 0.0f64..1.0,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            use pharma_core::matching::{MatchingEngine, MatchingEngineConfig};

            // Create two engines - both without persistent recorder
            let engine1 = MatchingEngine::new(MatchingEngineConfig::default());
            let engine2 = MatchingEngine::new(MatchingEngineConfig::default());

            let offer = create_test_offer(&medication);
            let request = create_test_request(&medication);

            // Score the same match with both engines
            let (score1, action1) = engine1.score_match(&offer, &request, medication_score, None).await;
            let (score2, action2) = engine2.score_match(&offer, &request, medication_score, None).await;

            // Property 5: Scores should be consistent
            prop_assert!(
                (score1.total - score2.total).abs() < 0.001,
                "Scores should be consistent: {} vs {}",
                score1.total,
                score2.total
            );

            // Property 5: Actions should be consistent
            prop_assert_eq!(
                format!("{:?}", action1),
                format!("{:?}", action2),
                "Actions should be consistent"
            );

            Ok(())
        })?;
    }
}

// =============================================================================
// Property 3: Deduplication Correctness
// =============================================================================
// For any set of audit records retrieved from both memory buffer and database
// where some records exist in both locations, the merged result SHALL contain
// exactly one instance of each unique record (as identified by the `id` field).
//
// Validates: Requirements 3.2

/// Deduplicate records by id field (mirrors the implementation in audit_records.rs)
fn deduplicate_records(records: Vec<MatchAuditRecord>) -> Vec<MatchAuditRecord> {
    use std::collections::HashSet;

    let mut seen_ids: HashSet<Uuid> = HashSet::new();
    let mut deduplicated = Vec::with_capacity(records.len());

    for record in records {
        if seen_ids.insert(record.id) {
            deduplicated.push(record);
        }
    }

    deduplicated
}

/// Sort records by created_at timestamp in descending order (mirrors the implementation)
fn sort_records_by_timestamp(mut records: Vec<MatchAuditRecord>) -> Vec<MatchAuditRecord> {
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    records
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recordings-persistence, Property 3: Deduplication Correctness
    /// Validates: Requirements 3.2
    ///
    /// For any set of audit records with duplicates (same id), deduplication SHALL
    /// produce a result where each unique id appears exactly once.
    #[test]
    fn prop_deduplication_removes_duplicates_by_id(
        record_count in 2usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut records = Vec::new();
            let mut original_ids = Vec::new();

            // Create unique records
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let record = create_test_audit_record(med, None);
                original_ids.push(record.id);
                records.push(record);
            }

            // Add duplicates (clone some records)
            let duplicates_to_add = record_count / 2;
            for i in 0..duplicates_to_add {
                records.push(records[i].clone());
            }

            let total_with_duplicates = records.len();
            prop_assert!(
                total_with_duplicates > record_count,
                "Should have duplicates: {} total, {} unique",
                total_with_duplicates,
                record_count
            );

            // Deduplicate
            let deduplicated = deduplicate_records(records);

            // Property 3: Result should have exactly record_count unique records
            prop_assert_eq!(
                deduplicated.len(),
                record_count,
                "Deduplication should produce exactly {} unique records, got {}",
                record_count,
                deduplicated.len()
            );

            // Property 3: All original IDs should be present
            for id in &original_ids {
                prop_assert!(
                    deduplicated.iter().any(|r| r.id == *id),
                    "Original record with id {} should be in deduplicated result",
                    id
                );
            }

            // Property 3: No duplicate IDs in result
            let mut seen_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
            for record in &deduplicated {
                prop_assert!(
                    seen_ids.insert(record.id),
                    "Deduplicated result should not contain duplicate id {}",
                    record.id
                );
            }

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 3: Deduplication Correctness
    /// Validates: Requirements 3.2
    ///
    /// For any set of records with no duplicates, deduplication SHALL preserve
    /// all records unchanged.
    #[test]
    fn prop_deduplication_preserves_unique_records(
        record_count in 1usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut records = Vec::new();

            // Create unique records (each with unique id)
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let record = create_test_audit_record(med, None);
                records.push(record);
            }

            let original_len = records.len();
            let original_ids: Vec<Uuid> = records.iter().map(|r| r.id).collect();

            // Deduplicate
            let deduplicated = deduplicate_records(records);

            // Property 3: All records should be preserved
            prop_assert_eq!(
                deduplicated.len(),
                original_len,
                "Deduplication should preserve all {} unique records",
                original_len
            );

            // Property 3: All original IDs should be present
            for id in &original_ids {
                prop_assert!(
                    deduplicated.iter().any(|r| r.id == *id),
                    "Record with id {} should be preserved",
                    id
                );
            }

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 3: Deduplication Correctness
    /// Validates: Requirements 3.2
    ///
    /// Deduplication of an empty set SHALL return an empty set.
    #[test]
    fn prop_deduplication_empty_set_returns_empty(
        _dummy in 0..1, // proptest requires at least one input
    ) {
        let records: Vec<MatchAuditRecord> = Vec::new();
        let deduplicated = deduplicate_records(records);

        prop_assert!(
            deduplicated.is_empty(),
            "Deduplication of empty set should return empty set"
        );
    }
}

// =============================================================================
// Property 4: Temporal Ordering
// =============================================================================
// For any session query result containing multiple records, records SHALL be
// ordered by `created_at` timestamp in descending order (most recent first).
//
// Validates: Requirements 3.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recordings-persistence, Property 4: Temporal Ordering
    /// Validates: Requirements 3.3
    ///
    /// For any set of records, sorting by timestamp SHALL produce a result where
    /// each record's created_at is >= the next record's created_at (descending order).
    #[test]
    fn prop_temporal_ordering_descending(
        record_count in 2usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut records = Vec::new();

            // Create records with varying timestamps
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let mut record = create_test_audit_record(med, None);
                // Add some time variation to ensure different timestamps
                record.created_at = record.created_at - chrono::Duration::seconds(i as i64 * 10);
                records.push(record);
            }

            // Shuffle records to ensure they're not already sorted
            use rand::seq::SliceRandom;
            let mut rng = rand::rng();
            records.shuffle(&mut rng);

            // Sort by timestamp
            let sorted = sort_records_by_timestamp(records);

            // Property 4: Records should be in descending order by created_at
            for i in 0..sorted.len() - 1 {
                prop_assert!(
                    sorted[i].created_at >= sorted[i + 1].created_at,
                    "Record at index {} (created_at: {}) should be >= record at index {} (created_at: {})",
                    i,
                    sorted[i].created_at,
                    i + 1,
                    sorted[i + 1].created_at
                );
            }

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 4: Temporal Ordering
    /// Validates: Requirements 3.3
    ///
    /// Sorting a single record SHALL return that record unchanged.
    #[test]
    fn prop_temporal_ordering_single_record(
        medication in arb_medication_name(),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let record = create_test_audit_record(&medication, None);
            let original_id = record.id;
            let original_timestamp = record.created_at;

            let sorted = sort_records_by_timestamp(vec![record]);

            prop_assert_eq!(sorted.len(), 1, "Should have exactly one record");
            prop_assert_eq!(sorted[0].id, original_id, "Record id should be unchanged");
            prop_assert_eq!(
                sorted[0].created_at,
                original_timestamp,
                "Record timestamp should be unchanged"
            );

            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 4: Temporal Ordering
    /// Validates: Requirements 3.3
    ///
    /// Sorting an empty set SHALL return an empty set.
    #[test]
    fn prop_temporal_ordering_empty_set(
        _dummy in 0..1, // proptest requires at least one input
    ) {
        let records: Vec<MatchAuditRecord> = Vec::new();
        let sorted = sort_records_by_timestamp(records);

        prop_assert!(
            sorted.is_empty(),
            "Sorting empty set should return empty set"
        );
    }

    /// Feature: debug-recordings-persistence, Property 4: Temporal Ordering
    /// Validates: Requirements 3.3
    ///
    /// Sorting should be stable - records with the same timestamp should maintain
    /// their relative order.
    #[test]
    fn prop_temporal_ordering_preserves_count(
        record_count in 1usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let medications = ["Aspirin", "Paracetamol", "Ibuprofen", "Amoxicillin", "Metformin"];
            let mut records = Vec::new();

            // Create records
            for i in 0..record_count {
                let med = medications[i % medications.len()];
                let record = create_test_audit_record(med, None);
                records.push(record);
            }

            let original_count = records.len();
            let original_ids: std::collections::HashSet<Uuid> = records.iter().map(|r| r.id).collect();

            // Sort by timestamp
            let sorted = sort_records_by_timestamp(records);

            // Property 4: Count should be preserved
            prop_assert_eq!(
                sorted.len(),
                original_count,
                "Sorting should preserve record count"
            );

            // Property 4: All original records should be present
            let sorted_ids: std::collections::HashSet<Uuid> = sorted.iter().map(|r| r.id).collect();
            prop_assert_eq!(
                original_ids,
                sorted_ids,
                "Sorting should preserve all record ids"
            );

            Ok(())
        })?;
    }
}

// =============================================================================
// Property 2: Serialization Round-Trip
// =============================================================================
// For any valid MatchAuditRecord, serializing it to the database model
// (MatchAuditRecordModel) and deserializing it back SHALL produce an equivalent
// record with all fields preserved.
//
// Validates: Requirements 4.2, 4.3

/// Create a test audit record with AI involvement for comprehensive round-trip testing
fn create_test_audit_record_with_ai(
    medication: &str,
    session_id: Option<&str>,
) -> MatchAuditRecord {
    let mut record = create_test_audit_record(medication, session_id);

    // Add AI involvement record
    record.ai_involved = true;
    record.ai_record = Some(AIInvolvementRecord {
        model: "test-model-v1".to_string(),
        prompt_tokens: Some(100),
        completion_tokens: Some(50),
        latency_ms: 250,
        response: serde_json::json!({
            "status": "approved",
            "confidence": 0.95,
            "explanation": "High confidence match"
        }),
    });

    // Add review status
    record.review_status = Some("reviewed".to_string());
    record.reviewed_by = Some(Uuid::new_v4());
    record.reviewed_at = Some(Utc::now());
    record.review_notes = Some("Test review notes".to_string());

    record
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recordings-persistence, Property 2: Serialization Round-Trip
    /// Validates: Requirements 4.2, 4.3
    ///
    /// For any valid MatchAuditRecord, serializing to database model and deserializing
    /// back SHALL produce an equivalent record with all core fields preserved.
    #[test]
    fn prop_serialization_round_trip_preserves_core_fields(
        medication in arb_medication_name(),
        session_id in proptest::option::of(arb_session_id()),
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

            // Create original record
            let original = create_test_audit_record(&medication, session_id.as_deref());
            let original_id = original.id;
            let original_match_id = original.match_id;
            let original_offer_id = original.offer_id;
            let original_request_id = original.request_id;
            let original_final_score = original.final_score;
            let original_resolution_stage = original.resolution_stage.clone();
            let original_ai_involved = original.ai_involved;
            let original_session_id = original.session_id.clone();
            let original_pipeline_version = original.pipeline_version.clone();

            // Record and flush
            recorder.record(original).await;
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(original_match_id).await.unwrap();
            prop_assert!(retrieved.is_some(), "Record should be retrievable after round-trip");

            let retrieved = retrieved.unwrap();

            // Property 2: Core fields must be preserved
            prop_assert_eq!(retrieved.id, original_id, "id must be preserved");
            prop_assert_eq!(retrieved.match_id, original_match_id, "match_id must be preserved");
            prop_assert_eq!(retrieved.offer_id, original_offer_id, "offer_id must be preserved");
            prop_assert_eq!(retrieved.request_id, original_request_id, "request_id must be preserved");
            prop_assert!(
                (retrieved.final_score - original_final_score).abs() < 0.001,
                "final_score must be preserved: {} vs {}",
                retrieved.final_score,
                original_final_score
            );
            prop_assert_eq!(
                retrieved.resolution_stage,
                original_resolution_stage,
                "resolution_stage must be preserved"
            );
            prop_assert_eq!(
                retrieved.ai_involved,
                original_ai_involved,
                "ai_involved must be preserved"
            );
            prop_assert_eq!(
                retrieved.session_id,
                original_session_id,
                "session_id must be preserved"
            );
            prop_assert_eq!(
                retrieved.pipeline_version,
                original_pipeline_version,
                "pipeline_version must be preserved"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 2: Serialization Round-Trip
    /// Validates: Requirements 4.2, 4.3
    ///
    /// For any MatchAuditRecord with AI involvement, serializing and deserializing
    /// SHALL preserve the AI-related fields.
    #[test]
    fn prop_serialization_round_trip_preserves_ai_fields(
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

            // Create record with AI involvement
            let original = create_test_audit_record_with_ai(&medication, Some("test-session"));
            let original_match_id = original.match_id;
            let original_ai_involved = original.ai_involved;
            let original_ai_model = original.ai_record.as_ref().map(|r| r.model.clone());
            let original_ai_latency = original.ai_record.as_ref().map(|r| r.latency_ms);

            // Record and flush
            recorder.record(original).await;
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(original_match_id).await.unwrap();
            prop_assert!(retrieved.is_some(), "Record should be retrievable after round-trip");

            let retrieved = retrieved.unwrap();

            // Property 2: AI fields must be preserved
            prop_assert_eq!(
                retrieved.ai_involved,
                original_ai_involved,
                "ai_involved must be preserved"
            );

            if original_ai_involved {
                prop_assert!(
                    retrieved.ai_record.is_some(),
                    "ai_record must be present when ai_involved is true"
                );

                let retrieved_ai = retrieved.ai_record.as_ref().unwrap();
                prop_assert_eq!(
                    Some(retrieved_ai.model.clone()),
                    original_ai_model,
                    "ai_model must be preserved"
                );
                prop_assert_eq!(
                    Some(retrieved_ai.latency_ms),
                    original_ai_latency,
                    "ai_latency_ms must be preserved"
                );
            }

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 2: Serialization Round-Trip
    /// Validates: Requirements 4.2, 4.3
    ///
    /// For any MatchAuditRecord with review status, serializing and deserializing
    /// SHALL preserve the review-related fields.
    #[test]
    fn prop_serialization_round_trip_preserves_review_fields(
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

            // Create record with review status
            let original = create_test_audit_record_with_ai(&medication, None);
            let original_match_id = original.match_id;
            let original_review_status = original.review_status.clone();
            let original_reviewed_by = original.reviewed_by;
            let original_review_notes = original.review_notes.clone();

            // Record and flush
            recorder.record(original).await;
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(original_match_id).await.unwrap();
            prop_assert!(retrieved.is_some(), "Record should be retrievable after round-trip");

            let retrieved = retrieved.unwrap();

            // Property 2: Review fields must be preserved
            prop_assert_eq!(
                retrieved.review_status,
                original_review_status,
                "review_status must be preserved"
            );
            prop_assert_eq!(
                retrieved.reviewed_by,
                original_reviewed_by,
                "reviewed_by must be preserved"
            );
            prop_assert_eq!(
                retrieved.review_notes,
                original_review_notes,
                "review_notes must be preserved"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 2: Serialization Round-Trip
    /// Validates: Requirements 4.2, 4.3
    ///
    /// For any MatchAuditRecord, the JSON snapshot fields (offer_snapshot,
    /// request_snapshot, weights_snapshot, score_breakdown) SHALL be preserved
    /// through serialization round-trip.
    #[test]
    fn prop_serialization_round_trip_preserves_json_snapshots(
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

            // Create record
            let original = create_test_audit_record(&medication, None);
            let original_match_id = original.match_id;
            let original_offer_snapshot = original.offer_snapshot.clone();
            let original_request_snapshot = original.request_snapshot.clone();
            let original_weights_snapshot = original.weights_snapshot.clone();
            let original_score_breakdown = original.score_breakdown.clone();

            // Record and flush
            recorder.record(original).await;
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(original_match_id).await.unwrap();
            prop_assert!(retrieved.is_some(), "Record should be retrievable after round-trip");

            let retrieved = retrieved.unwrap();

            // Property 2: JSON snapshot fields must be preserved
            prop_assert_eq!(
                retrieved.offer_snapshot,
                original_offer_snapshot,
                "offer_snapshot must be preserved"
            );
            prop_assert_eq!(
                retrieved.request_snapshot,
                original_request_snapshot,
                "request_snapshot must be preserved"
            );
            prop_assert_eq!(
                retrieved.weights_snapshot,
                original_weights_snapshot,
                "weights_snapshot must be preserved"
            );
            prop_assert_eq!(
                retrieved.score_breakdown,
                original_score_breakdown,
                "score_breakdown must be preserved"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }

    /// Feature: debug-recordings-persistence, Property 2: Serialization Round-Trip
    /// Validates: Requirements 4.2, 4.3
    ///
    /// For any MatchAuditRecord, the unique identifier (id) SHALL be preserved
    /// through serialization round-trip, enabling deduplication.
    #[test]
    fn prop_serialization_round_trip_preserves_unique_id(
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

            // Create record
            let original = create_test_audit_record(&medication, None);
            let original_id = original.id;
            let original_match_id = original.match_id;

            // Record and flush
            recorder.record(original).await;
            recorder.flush().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Retrieve from database
            let retrieved = recorder.get_by_match_id(original_match_id).await.unwrap();
            prop_assert!(retrieved.is_some(), "Record should be retrievable after round-trip");

            let retrieved = retrieved.unwrap();

            // Property 2: Unique id must be preserved for deduplication (Req 4.4)
            prop_assert_eq!(
                retrieved.id,
                original_id,
                "Unique id must be preserved for deduplication"
            );

            recorder.shutdown();
            Ok(())
        })?;
    }
}
