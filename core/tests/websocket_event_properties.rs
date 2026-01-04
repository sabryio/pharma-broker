//! Property-based tests for WebSocket Pipeline Events
//!
//! Feature: debug-recording-enhancement
//! Tests Properties 4 and 5 from the design document
//!
//! These tests validate:
//! - Property 4: WebSocket Event Emission
//! - Property 5: WebSocket Error Handling
//!
//! Run with: cargo test --features test-pipeline-props --test websocket_event_properties

#![cfg(feature = "test-pipeline-props")]

use pharma_core::matching::{
    AiOperation, BroadcastEventEmitter, FilteredEventReceiver, MatchOutcome, PipelineEvent,
    PipelineEventEmitter, PipelineStageType,
};
use proptest::prelude::*;
use uuid::Uuid;

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate a random pipeline stage type
fn arb_pipeline_stage_type() -> impl Strategy<Value = PipelineStageType> {
    prop_oneof![
        Just(PipelineStageType::MessageReceived),
        Just(PipelineStageType::AiParsing),
        Just(PipelineStageType::ParsingComplete),
        Just(PipelineStageType::MedicationResolution),
        Just(PipelineStageType::OfferCreated),
        Just(PipelineStageType::RequestCreated),
        Just(PipelineStageType::MatchCandidateSearch),
        (1u8..=5).prop_map(|n| PipelineStageType::HierarchicalStage { stage_number: n }),
        Just(PipelineStageType::ScoreCalculation),
        Just(PipelineStageType::AiReview),
        Just(PipelineStageType::ConsensusCheck),
        Just(PipelineStageType::ContrastiveValidation),
        Just(PipelineStageType::Calibration),
        Just(PipelineStageType::MatchCreated),
        Just(PipelineStageType::QueueAdded),
        Just(PipelineStageType::NotificationSent),
    ]
}

/// Generate a random AI operation type
fn arb_ai_operation() -> impl Strategy<Value = AiOperation> {
    prop_oneof![
        Just(AiOperation::Parsing),
        Just(AiOperation::Review),
        Just(AiOperation::ConsensusAudit),
        Just(AiOperation::ContrastiveValidation),
    ]
}

/// Generate a random match outcome
fn arb_match_outcome() -> impl Strategy<Value = MatchOutcome> {
    prop_oneof![
        Just(MatchOutcome::Approved),
        Just(MatchOutcome::Rejected),
        Just(MatchOutcome::PendingReview),
        Just(MatchOutcome::AutoApproved),
        Just(MatchOutcome::Flagged),
        Just(MatchOutcome::NoMatch),
    ]
}

/// Generate a random duration in milliseconds
fn arb_duration_ms() -> impl Strategy<Value = u64> {
    1u64..10000
}

/// Generate a random candidate count
fn arb_candidate_count() -> impl Strategy<Value = usize> {
    0usize..1000
}

/// Generate a random score
fn arb_score() -> impl Strategy<Value = f64> {
    0.0f64..1.0
}

/// Generate a random model name
fn arb_model_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("gpt-4".to_string()),
        Just("gpt-4-turbo".to_string()),
        Just("claude-3-opus".to_string()),
        Just("claude-3-sonnet".to_string()),
    ]
}

/// Generate a random error message
fn arb_error_message() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Connection timeout".to_string()),
        Just("Invalid response format".to_string()),
        Just("Rate limit exceeded".to_string()),
        Just("Internal server error".to_string()),
        Just("Database connection failed".to_string()),
    ]
}

// =============================================================================
// Property 4: WebSocket Event Emission
// =============================================================================
// For any match operation, the system SHALL emit a MatchStarted event at the
// beginning and a MatchCompleted event at the end, and for each pipeline stage
// that completes, a StageCompleted event SHALL be emitted with the stage name
// and duration.
//
// Validates: Requirements 2.1, 2.2, 2.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.1
    ///
    /// For any match operation, a MatchStarted event SHALL be emitted at the beginning
    /// with match_id, offer_id, and request_id.
    #[test]
    fn prop_match_started_event_is_complete(
        session_id in proptest::option::of("[a-z0-9]{8}-[a-z0-9]{4}"),
    ) {
        let match_id = Uuid::new_v4();
        let offer_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();

        let event = PipelineEvent::match_started(
            match_id,
            offer_id,
            request_id,
            session_id.clone(),
        );

        // Verify event contains all required fields
        prop_assert_eq!(event.match_id(), match_id);
        prop_assert!(!event.is_terminal());
        prop_assert!(!event.is_error());
        prop_assert_eq!(event.event_type(), "match_started");

        // Verify serialization works
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("match_started"));
        prop_assert!(json.contains(&match_id.to_string()));
        prop_assert!(json.contains(&offer_id.to_string()));
        prop_assert!(json.contains(&request_id.to_string()));

        if let Some(sid) = &session_id {
            prop_assert!(json.contains(sid));
        }
    }

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.2
    ///
    /// For each pipeline stage that completes, a StageCompleted event SHALL be
    /// emitted with stage_type, duration_ms, candidates_in, and candidates_out.
    #[test]
    fn prop_stage_completed_event_is_complete(
        stage in arb_pipeline_stage_type(),
        duration_ms in arb_duration_ms(),
        candidates_in in arb_candidate_count(),
        candidates_out in arb_candidate_count(),
    ) {
        let match_id = Uuid::new_v4();
        let summary = format!("Processed {} candidates", candidates_out);

        let event = PipelineEvent::stage_completed(
            match_id,
            stage,
            duration_ms,
            candidates_in,
            candidates_out,
            &summary,
        );

        // Verify event contains all required fields
        prop_assert_eq!(event.match_id(), match_id);
        prop_assert!(!event.is_terminal());
        prop_assert!(!event.is_error());
        prop_assert_eq!(event.event_type(), "stage_completed");

        // Verify serialization includes all fields
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("stage_completed"));
        prop_assert!(json.contains(&match_id.to_string()));
        prop_assert!(json.contains(&duration_ms.to_string()));
        prop_assert!(json.contains(&candidates_in.to_string()));
        prop_assert!(json.contains(&candidates_out.to_string()));
    }

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.4
    ///
    /// For any match operation that completes, a MatchCompleted event SHALL be
    /// emitted with audit_record_id, final_score, and outcome.
    #[test]
    fn prop_match_completed_event_is_complete(
        final_score in arb_score(),
        outcome in arb_match_outcome(),
        total_duration_ms in arb_duration_ms(),
        stages_completed in 1usize..20,
    ) {
        let match_id = Uuid::new_v4();
        let audit_record_id = Uuid::new_v4();

        let event = PipelineEvent::match_completed(
            match_id,
            audit_record_id,
            final_score,
            outcome.clone(),
            total_duration_ms,
            stages_completed,
        );

        // Verify event is terminal
        prop_assert!(event.is_terminal());
        prop_assert!(!event.is_error());
        prop_assert_eq!(event.event_type(), "match_completed");

        // Verify serialization includes all fields
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("match_completed"));
        prop_assert!(json.contains(&audit_record_id.to_string()));
        prop_assert!(json.contains(&outcome.to_string()));
    }

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.1, 2.2, 2.4
    ///
    /// Events emitted to a broadcast channel SHALL be received by all subscribers.
    #[test]
    fn prop_broadcast_emitter_delivers_to_all_subscribers(
        subscriber_count in 1usize..5,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let emitter = BroadcastEventEmitter::new();

            // Create multiple subscribers
            let mut receivers: Vec<_> = (0..subscriber_count)
                .map(|_| emitter.subscribe_all())
                .collect();

            prop_assert_eq!(emitter.subscriber_count(), subscriber_count);

            // Emit an event
            let match_id = Uuid::new_v4();
            let event = PipelineEvent::match_started(
                match_id,
                Uuid::new_v4(),
                Uuid::new_v4(),
                None,
            );
            emitter.emit(event);

            // All subscribers should receive the event
            for receiver in &mut receivers {
                let received = receiver.recv().await.unwrap();
                prop_assert_eq!(received.match_id(), match_id);
            }

            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.1, 2.2
    ///
    /// Filtered receivers SHALL only receive events for their subscribed match_id.
    #[test]
    fn prop_filtered_receiver_filters_by_match_id(
        event_count in 1usize..10,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let emitter = BroadcastEventEmitter::new();

            let target_match_id = Uuid::new_v4();
            let other_match_id = Uuid::new_v4();

            let receiver = emitter.subscribe(target_match_id);
            let mut filtered = FilteredEventReceiver::new(target_match_id, receiver);

            // Emit events for both match IDs
            for i in 0..event_count {
                // Emit for other match
                emitter.emit(PipelineEvent::stage_completed(
                    other_match_id,
                    PipelineStageType::HierarchicalStage { stage_number: i as u8 + 1 },
                    100,
                    10,
                    5,
                    "Other match",
                ));

                // Emit for target match
                emitter.emit(PipelineEvent::stage_completed(
                    target_match_id,
                    PipelineStageType::HierarchicalStage { stage_number: i as u8 + 1 },
                    100,
                    10,
                    5,
                    "Target match",
                ));
            }

            // Filtered receiver should only get target match events
            for _ in 0..event_count {
                let received = filtered.recv().await.unwrap();
                prop_assert_eq!(received.match_id(), target_match_id);
            }

            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.1, 2.2
    ///
    /// AI processing events SHALL be emitted when AI operations start and complete.
    #[test]
    fn prop_ai_processing_events_are_paired(
        model in arb_model_name(),
        operation in arb_ai_operation(),
        duration_ms in arb_duration_ms(),
        estimated_duration_ms in proptest::option::of(arb_duration_ms()),
    ) {
        let match_id = Uuid::new_v4();

        // Create start event
        let start_event = PipelineEvent::ai_processing_started(
            match_id,
            &model,
            operation,
            estimated_duration_ms,
        );

        // Create completion event
        let complete_event = PipelineEvent::ai_processing_completed(
            match_id,
            &model,
            operation,
            duration_ms,
            true,
        );

        // Verify both events have same match_id
        prop_assert_eq!(start_event.match_id(), match_id);
        prop_assert_eq!(complete_event.match_id(), match_id);

        // Verify event types
        prop_assert_eq!(start_event.event_type(), "ai_processing_started");
        prop_assert_eq!(complete_event.event_type(), "ai_processing_completed");

        // Neither should be terminal
        prop_assert!(!start_event.is_terminal());
        prop_assert!(!complete_event.is_terminal());
    }
}

// =============================================================================
// Property 5: WebSocket Error Handling
// =============================================================================
// For any pipeline stage that fails with an error, the system SHALL emit a
// StageError event containing the stage name, error message, and any partial
// results available.
//
// Validates: Requirements 2.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 5: WebSocket Error Handling
    /// Validates: Requirements 2.5
    ///
    /// For any pipeline stage that fails, a StageError event SHALL be emitted
    /// with stage_type, error message, and recoverable flag.
    #[test]
    fn prop_stage_error_event_is_complete(
        stage in arb_pipeline_stage_type(),
        error in arb_error_message(),
        recoverable in any::<bool>(),
    ) {
        let match_id = Uuid::new_v4();

        let event = PipelineEvent::stage_error(
            match_id,
            stage,
            &error,
            None,
            recoverable,
        );

        // Verify event is an error
        prop_assert!(event.is_error());
        prop_assert!(!event.is_terminal()); // Stage errors are not terminal
        prop_assert_eq!(event.event_type(), "stage_error");

        // Verify serialization includes error details
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("stage_error"));
        prop_assert!(json.contains(&error));
        prop_assert!(json.contains(&recoverable.to_string()));
    }

    /// Feature: debug-recording-enhancement, Property 5: WebSocket Error Handling
    /// Validates: Requirements 2.5
    ///
    /// For any match operation that fails completely, a MatchFailed event SHALL
    /// be emitted with error message and last completed stage.
    #[test]
    fn prop_match_failed_event_is_complete(
        error in arb_error_message(),
        last_stage in proptest::option::of(arb_pipeline_stage_type()),
    ) {
        let match_id = Uuid::new_v4();
        let partial_audit_id = if last_stage.is_some() {
            Some(Uuid::new_v4())
        } else {
            None
        };

        let event = PipelineEvent::match_failed(
            match_id,
            &error,
            last_stage,
            partial_audit_id,
        );

        // Verify event is both error and terminal
        prop_assert!(event.is_error());
        prop_assert!(event.is_terminal());
        prop_assert_eq!(event.event_type(), "match_failed");

        // Verify serialization includes error details
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("match_failed"));
        prop_assert!(json.contains(&error));
    }

    /// Feature: debug-recording-enhancement, Property 5: WebSocket Error Handling
    /// Validates: Requirements 2.5
    ///
    /// Stage errors with partial results SHALL include those results in the event.
    #[test]
    fn prop_stage_error_includes_partial_results(
        stage in arb_pipeline_stage_type(),
        error in arb_error_message(),
        candidates_processed in arb_candidate_count(),
    ) {
        let match_id = Uuid::new_v4();

        let partial_results = serde_json::json!({
            "candidates_processed": candidates_processed,
            "last_score": 0.75,
            "error_at_index": candidates_processed / 2,
        });

        let event = PipelineEvent::stage_error(
            match_id,
            stage,
            &error,
            Some(partial_results.clone()),
            true, // recoverable
        );

        // Verify serialization includes partial results
        let json = serde_json::to_string(&event).unwrap();
        prop_assert!(json.contains("partial_results"));
        prop_assert!(json.contains(&candidates_processed.to_string()));
    }

    /// Feature: debug-recording-enhancement, Property 5: WebSocket Error Handling
    /// Validates: Requirements 2.5
    ///
    /// Error events SHALL be delivered to subscribers even when other events are pending.
    #[test]
    fn prop_error_events_are_delivered(
        normal_event_count in 1usize..5,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let emitter = BroadcastEventEmitter::new();
            let mut receiver = emitter.subscribe_all();

            let match_id = Uuid::new_v4();

            // Emit some normal events
            for i in 0..normal_event_count {
                emitter.emit(PipelineEvent::stage_completed(
                    match_id,
                    PipelineStageType::HierarchicalStage { stage_number: i as u8 + 1 },
                    100,
                    10,
                    5,
                    "Normal stage",
                ));
            }

            // Emit an error event
            emitter.emit(PipelineEvent::stage_error(
                match_id,
                PipelineStageType::AiParsing,
                "Test error",
                None,
                false,
            ));

            // Receive all events and verify error is included
            let mut received_error = false;
            for _ in 0..=normal_event_count {
                let event = receiver.recv().await.unwrap();
                if event.is_error() {
                    received_error = true;
                }
            }

            prop_assert!(received_error, "Error event should be received");

            Ok(())
        })?;
    }

    /// Feature: debug-recording-enhancement, Property 5: WebSocket Error Handling
    /// Validates: Requirements 2.5
    ///
    /// Progress events SHALL include valid progress percentage (0-100).
    #[test]
    fn prop_progress_events_have_valid_percentage(
        progress in 0u8..=150, // Test values beyond 100 to verify clamping
    ) {
        let match_id = Uuid::new_v4();

        let event = PipelineEvent::stage_progress(
            match_id,
            PipelineStageType::AiParsing,
            progress,
            "Processing...",
        );

        // Verify progress is clamped to 0-100
        if let PipelineEvent::StageProgress { progress_percent, .. } = event {
            prop_assert!(progress_percent <= 100, "Progress should be clamped to 100");
        } else {
            prop_assert!(false, "Expected StageProgress event");
        }
    }
}

// =============================================================================
// Additional Event Sequence Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Feature: debug-recording-enhancement, Property 4: WebSocket Event Emission
    /// Validates: Requirements 2.1, 2.2, 2.4
    ///
    /// A complete match operation sequence SHALL have MatchStarted first and
    /// MatchCompleted or MatchFailed last.
    #[test]
    fn prop_event_sequence_has_correct_bookends(
        stage_count in 1usize..5,
        success in any::<bool>(),
    ) {
        let match_id = Uuid::new_v4();
        let offer_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();

        let mut events = Vec::new();

        // Start event
        events.push(PipelineEvent::match_started(
            match_id,
            offer_id,
            request_id,
            None,
        ));

        // Stage events
        for i in 0..stage_count {
            events.push(PipelineEvent::stage_completed(
                match_id,
                PipelineStageType::HierarchicalStage { stage_number: i as u8 + 1 },
                100,
                10,
                5,
                format!("Stage {}", i + 1),
            ));
        }

        // End event
        if success {
            events.push(PipelineEvent::match_completed(
                match_id,
                Uuid::new_v4(),
                0.95,
                MatchOutcome::Approved,
                1000,
                stage_count,
            ));
        } else {
            events.push(PipelineEvent::match_failed(
                match_id,
                "Test failure",
                Some(PipelineStageType::HierarchicalStage { stage_number: stage_count as u8 }),
                None,
            ));
        }

        // Verify sequence
        prop_assert_eq!(events.first().unwrap().event_type(), "match_started");
        prop_assert!(events.last().unwrap().is_terminal());

        // All events should have same match_id
        for event in &events {
            prop_assert_eq!(event.match_id(), match_id);
        }

        // Only the last event should be terminal
        for (i, event) in events.iter().enumerate() {
            if i < events.len() - 1 {
                prop_assert!(!event.is_terminal(), "Non-final event should not be terminal");
            }
        }
    }
}
