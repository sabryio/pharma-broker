//! Integration tests for Debug Recording Enhancement
//!
//! Feature: debug-recording-enhancement
//! Tests end-to-end flow for:
//! - Complete match operation with recording
//! - WebSocket event delivery
//! - Frontend-backend synchronization
//!
//! Run with: cargo test --features test-debug-recording --test debug_recording_integration

#![cfg(feature = "test-debug-recording")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pharma_core::domain::{Offer, Request};
use pharma_core::matching::{
    AiOperation, AuditRecordBuilder, AuditRecorderConfig, BroadcastEventEmitter,
    EnhancedPipelineStageRecord, FilteredEventReceiver, MatchOutcome, NormalizedWeights,
    ParsingDetails, PersistentAuditConfig, PersistentAuditRecorder, PipelineEvent,
    PipelineEventEmitter, PipelineStageDetails, PipelineStageType, ScoreBreakdown, ScoringDetails,
};
use pharma_core::repository::{MatchAuditRecordModel, MatchAuditRecordRepository};
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
}

impl MockMatchAuditRecordRepository {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            insert_count: AtomicU64::new(0),
        }
    }

    pub async fn get_all_records(&self) -> Vec<MatchAuditRecordModel> {
        self.records.read().await.clone()
    }

    pub fn insert_count(&self) -> u64 {
        self.insert_count.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl MatchAuditRecordRepository for MockMatchAuditRecordRepository {
    async fn insert(
        &self,
        record: &MatchAuditRecordModel,
    ) -> pharma_db::Result<MatchAuditRecordModel> {
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
        medication_raw: format!("{} raw", medication),
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
        medication_raw: format!("{} raw", medication),
        quantity: Some(Decimal::new(10, 0)),
        max_price: Some(Decimal::new(150, 0)),
        ..Default::default()
    }
}

// =============================================================================
// Integration Test: Complete Match Operation with Recording
// =============================================================================

/// Test that a complete match operation creates a full audit record with all stages
#[tokio::test]
async fn test_complete_match_operation_with_recording() {
    let medication = "Augmentin 1g";
    let offer = create_test_offer(medication);
    let request = create_test_request(medication);
    let weights = NormalizedWeights::default();
    let match_id = Uuid::new_v4();
    let session_id = "test-session-12345";

    // Create audit record builder with session tracking
    let mut builder =
        AuditRecordBuilder::new(match_id, offer.clone(), request.clone(), weights.clone())
            .session_id(session_id);

    // Simulate pipeline stages
    let now = Utc::now();

    // Stage 1: Message Received
    let stage1 = EnhancedPipelineStageRecord::new(
        PipelineStageType::MessageReceived,
        now,
        now + chrono::Duration::milliseconds(10),
        0,
        1,
        PipelineStageDetails::default(),
    );
    builder.add_enhanced_stage(stage1);

    // Stage 2: AI Parsing
    let parsing_details = ParsingDetails::success(
        "gpt-4",
        150,
        serde_json::json!({
            "medication": medication,
            "quantity": 10,
            "price": 100
        }),
    )
    .with_tokens(100, 50)
    .with_confidence(0.95);

    builder.add_parsing_stage(
        now + chrono::Duration::milliseconds(10),
        now + chrono::Duration::milliseconds(160),
        parsing_details,
    );

    // Stage 3: Score Calculation
    let mut scoring_details = ScoringDetails::new(0.85, "weighted_sum");
    scoring_details.add_component("medication", 0.95, 0.4);
    scoring_details.add_component("quantity", 0.80, 0.3);
    scoring_details.add_component("price", 0.75, 0.3);

    builder.add_scoring_stage(
        now + chrono::Duration::milliseconds(160),
        now + chrono::Duration::milliseconds(180),
        10,
        scoring_details,
    );

    // Record performance metrics
    builder.record_memory(50000);
    builder.record_ai_timing(10, 140);
    builder.record_db_query(5);

    // Build the final record
    let breakdown = ScoreBreakdown::new(&weights, 0.95, 0.80, 0.75, 0.70, 0.90);
    let record = builder.build_enhanced(&breakdown);

    // Verify the record has all expected data
    assert_eq!(record.match_id, match_id);
    assert_eq!(record.offer_id, offer.id);
    assert_eq!(record.request_id, request.id);
    assert_eq!(record.session_id, Some(session_id.to_string()));
    assert!(
        record.ai_involved,
        "AI should be involved due to parsing stage"
    );
    assert!(
        !record.enhanced_stages.is_empty(),
        "Should have pipeline stages"
    );
    assert!(
        record.enhanced_stages.len() >= 3,
        "Should have at least 3 stages"
    );

    // Verify performance metrics
    assert!(record.performance_metrics.memory_peak_bytes.is_some());
    assert!(record.performance_metrics.ai_processing_ms.is_some());
    assert!(record.performance_metrics.db_query_count > 0);
}

// =============================================================================
// Integration Test: WebSocket Event Delivery
// =============================================================================

/// Test that WebSocket events are properly emitted and received during a match operation
#[tokio::test]
async fn test_websocket_event_delivery() {
    let emitter = BroadcastEventEmitter::new();
    let match_id = Uuid::new_v4();
    let offer_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    let session_id = Some("ws-test-session".to_string());

    // Subscribe to events for this match
    let receiver = emitter.subscribe(match_id);
    let mut filtered = FilteredEventReceiver::new(match_id, receiver);

    // Emit MatchStarted event
    emitter.emit(PipelineEvent::match_started(
        match_id,
        offer_id,
        request_id,
        session_id.clone(),
    ));

    // Emit stage completion events
    let stages = vec![
        PipelineStageType::MessageReceived,
        PipelineStageType::AiParsing,
        PipelineStageType::MedicationResolution,
        PipelineStageType::ScoreCalculation,
    ];

    for (i, stage) in stages.iter().enumerate() {
        emitter.emit(PipelineEvent::stage_completed(
            match_id,
            stage.clone(),
            (i + 1) as u64 * 50,
            10,
            10 - i,
            format!("Stage {} completed", i + 1),
        ));
    }

    // Emit AI processing events
    emitter.emit(PipelineEvent::ai_processing_started(
        match_id,
        "gpt-4",
        AiOperation::Parsing,
        Some(200),
    ));

    emitter.emit(PipelineEvent::ai_processing_completed(
        match_id,
        "gpt-4",
        AiOperation::Parsing,
        180,
        true,
    ));

    // Emit MatchCompleted event
    let audit_record_id = Uuid::new_v4();
    emitter.emit(PipelineEvent::match_completed(
        match_id,
        audit_record_id,
        0.85,
        MatchOutcome::Approved,
        500,
        stages.len(),
    ));

    // Verify events are received in order
    let mut received_events = Vec::new();
    let expected_count = 8; // 1 start + 4 stages + 2 AI + 1 complete
    for _ in 0..expected_count {
        if let Ok(event) =
            tokio::time::timeout(tokio::time::Duration::from_millis(200), filtered.recv()).await
        {
            if let Ok(e) = event {
                received_events.push(e);
            }
        }
    }

    // Verify we received events
    assert!(!received_events.is_empty(), "Should receive events");

    // Verify first event is MatchStarted
    assert_eq!(
        received_events.first().unwrap().event_type(),
        "match_started"
    );

    // Verify we received a terminal event (MatchCompleted)
    let has_terminal = received_events.iter().any(|e| e.is_terminal());
    assert!(
        has_terminal,
        "Should receive a terminal event (MatchCompleted)"
    );

    // Verify all events have correct match_id
    for event in &received_events {
        assert_eq!(event.match_id(), match_id);
    }
}

/// Test that filtered receiver only receives events for subscribed match_id
#[tokio::test]
async fn test_websocket_event_filtering() {
    let emitter = BroadcastEventEmitter::new();
    let target_match_id = Uuid::new_v4();
    let other_match_id = Uuid::new_v4();

    // Subscribe only to target match
    let receiver = emitter.subscribe(target_match_id);
    let mut filtered = FilteredEventReceiver::new(target_match_id, receiver);

    // Emit events for both matches
    emitter.emit(PipelineEvent::match_started(
        other_match_id,
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
    ));

    emitter.emit(PipelineEvent::match_started(
        target_match_id,
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
    ));

    emitter.emit(PipelineEvent::stage_completed(
        other_match_id,
        PipelineStageType::AiParsing,
        100,
        10,
        5,
        "Other match stage",
    ));

    emitter.emit(PipelineEvent::stage_completed(
        target_match_id,
        PipelineStageType::AiParsing,
        100,
        10,
        5,
        "Target match stage",
    ));

    // Receive events - should only get target match events
    let mut target_events = Vec::new();
    for _ in 0..2 {
        if let Ok(event) =
            tokio::time::timeout(tokio::time::Duration::from_millis(100), filtered.recv()).await
        {
            if let Ok(e) = event {
                target_events.push(e);
            }
        }
    }

    // Verify all received events are for target match
    for event in &target_events {
        assert_eq!(
            event.match_id(),
            target_match_id,
            "Should only receive events for target match"
        );
    }
}

// =============================================================================
// Integration Test: Frontend-Backend Synchronization
// =============================================================================

/// Test that session_id links frontend recordings to backend audit records
#[tokio::test]
async fn test_frontend_backend_session_synchronization() {
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

    // Simulate frontend generating a session_id
    let frontend_session_id = format!("frontend-session-{}", Uuid::new_v4());

    // Create multiple audit records with the same session_id (simulating multiple matches in one session)
    let medications = ["Aspirin", "Paracetamol", "Ibuprofen"];
    let mut match_ids = Vec::new();

    for medication in &medications {
        let offer = create_test_offer(medication);
        let request = create_test_request(medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();
        match_ids.push(match_id);

        let builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone())
            .session_id(&frontend_session_id);

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build(&breakdown);

        recorder.record(record).await;
    }

    // Query by session_id from buffer (before flush)
    let buffer_results = recorder
        .get_by_session_from_buffer(&frontend_session_id)
        .await;
    assert_eq!(
        buffer_results.len(),
        medications.len(),
        "Should find all records in buffer by session_id"
    );

    // Verify all records have correct session_id
    for record in &buffer_results {
        assert_eq!(
            record.session_id.as_deref(),
            Some(frontend_session_id.as_str())
        );
    }

    // Verify all match_ids are present
    for match_id in &match_ids {
        assert!(
            buffer_results.iter().any(|r| r.match_id == *match_id),
            "Record with match_id {} should be in results",
            match_id
        );
    }

    // Start flush task and flush to database
    let _handle = recorder.start_flush_task();
    recorder.flush().await;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query by session_id from both buffer and database
    let all_results = recorder.get_by_session(&frontend_session_id).await.unwrap();
    assert_eq!(
        all_results.len(),
        medications.len(),
        "Should find all records after flush by session_id"
    );

    recorder.shutdown();
}

/// Test that records without session_id are not returned when querying by session
#[tokio::test]
async fn test_session_isolation() {
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

    let recorder = PersistentAuditRecorder::new(config, repo.clone());

    let session_a = "session-a-12345";
    let session_b = "session-b-67890";

    // Create records with different sessions
    let offer_a = create_test_offer("Aspirin");
    let request_a = create_test_request("Aspirin");
    let weights = NormalizedWeights::default();
    let match_id_a = Uuid::new_v4();

    let builder_a = AuditRecordBuilder::new(match_id_a, offer_a, request_a, weights.clone())
        .session_id(session_a);
    let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
    let record_a = builder_a.build(&breakdown);
    recorder.record(record_a).await;

    let offer_b = create_test_offer("Paracetamol");
    let request_b = create_test_request("Paracetamol");
    let match_id_b = Uuid::new_v4();

    let builder_b = AuditRecordBuilder::new(match_id_b, offer_b, request_b, weights.clone())
        .session_id(session_b);
    let record_b = builder_b.build(&breakdown);
    recorder.record(record_b).await;

    // Create a record without session_id
    let offer_c = create_test_offer("Ibuprofen");
    let request_c = create_test_request("Ibuprofen");
    let match_id_c = Uuid::new_v4();

    let builder_c = AuditRecordBuilder::new(match_id_c, offer_c, request_c, weights.clone());
    let record_c = builder_c.build(&breakdown);
    recorder.record(record_c).await;

    // Query session A - should only get record A
    let results_a = recorder.get_by_session_from_buffer(session_a).await;
    assert_eq!(results_a.len(), 1);
    assert_eq!(results_a[0].match_id, match_id_a);

    // Query session B - should only get record B
    let results_b = recorder.get_by_session_from_buffer(session_b).await;
    assert_eq!(results_b.len(), 1);
    assert_eq!(results_b[0].match_id, match_id_b);

    // Query non-existent session - should get empty results
    let results_none = recorder
        .get_by_session_from_buffer("non-existent-session")
        .await;
    assert!(results_none.is_empty());
}

// =============================================================================
// Integration Test: Error Event Handling
// =============================================================================

/// Test that error events are properly emitted and contain required information
#[tokio::test]
async fn test_error_event_handling() {
    let emitter = BroadcastEventEmitter::new();
    let match_id = Uuid::new_v4();

    let mut receiver = emitter.subscribe_all();

    // Emit a stage error
    emitter.emit(PipelineEvent::stage_error(
        match_id,
        PipelineStageType::AiParsing,
        "AI service timeout",
        Some(serde_json::json!({
            "candidates_processed": 5,
            "last_score": 0.75
        })),
        true, // recoverable
    ));

    // Emit a match failure
    emitter.emit(PipelineEvent::match_failed(
        match_id,
        "Critical error in pipeline",
        Some(PipelineStageType::ScoreCalculation),
        None,
    ));

    // Receive and verify error events
    let stage_error = receiver.recv().await.unwrap();
    assert!(stage_error.is_error());
    assert!(!stage_error.is_terminal()); // Stage errors are not terminal
    assert_eq!(stage_error.event_type(), "stage_error");

    let match_failed = receiver.recv().await.unwrap();
    assert!(match_failed.is_error());
    assert!(match_failed.is_terminal()); // Match failures are terminal
    assert_eq!(match_failed.event_type(), "match_failed");
}

// =============================================================================
// Integration Test: Performance Metrics in Audit Records
// =============================================================================

/// Test that performance metrics are properly captured in audit records
#[tokio::test]
async fn test_performance_metrics_capture() {
    let medication = "Metformin";
    let offer = create_test_offer(medication);
    let request = create_test_request(medication);
    let weights = NormalizedWeights::default();
    let match_id = Uuid::new_v4();

    let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

    // Record various performance metrics
    builder.record_memory(100000);
    builder.record_memory(150000); // Peak should be 150000
    builder.record_memory(120000);

    builder.record_ai_timing(20, 300); // 20ms queue wait, 300ms processing
    builder.record_ai_timing(15, 250); // Additional AI call

    builder.record_db_query(10);
    builder.record_db_query(15);
    builder.record_db_query(8);

    let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
    let record = builder.build_enhanced(&breakdown);

    // Verify performance metrics
    let metrics = &record.performance_metrics;

    assert_eq!(
        metrics.memory_peak_bytes,
        Some(150000),
        "Peak memory should be the maximum recorded"
    );

    assert_eq!(
        metrics.ai_queue_wait_ms,
        Some(35), // 20 + 15
        "AI queue wait should be sum of all waits"
    );

    assert_eq!(
        metrics.ai_processing_ms,
        Some(550), // 300 + 250
        "AI processing should be sum of all processing times"
    );

    assert_eq!(
        metrics.db_query_count, 3,
        "DB query count should match number of queries"
    );

    assert_eq!(
        metrics.db_total_ms,
        33, // 10 + 15 + 8
        "DB total time should be sum of all query durations"
    );
}
