//! Property-based tests for Pipeline Stage Recording
//!
//! Feature: debug-recording-enhancement
//! Tests Properties 1, 2, and 11 from the design document
//!
//! These tests validate:
//! - Property 1: Pipeline Stage Completeness
//! - Property 2: AI Recording Completeness
//! - Property 11: Performance Metrics Capture
//!
//! Run with: cargo test --features test-pipeline-props --test pipeline_stage_properties

#![cfg(feature = "test-pipeline-props")]

use chrono::{Duration, Utc};
use pharma_core::domain::{Offer, Request};
use pharma_core::matching::{
    AiReviewDetails, AuditRecordBuilder, EnhancedConsensusDetails, EnhancedPipelineStageRecord,
    HierarchicalStageDetails, NormalizedWeights, ParsingDetails, PerformanceTracker,
    PipelinePerformanceMetrics, PipelineStageDetails, PipelineStageType, ScoreBreakdown,
    ScoringDetails,
};
use proptest::prelude::*;
use rust_decimal::Decimal;
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

// =============================================================================
// Property 1: Pipeline Stage Completeness
// =============================================================================
// For any match operation that completes successfully, the audit record SHALL
// contain pipeline stage records for all executed stages, and each stage record
// SHALL include stage_type, duration_ms, candidates_in, and candidates_out fields.
//
// Validates: Requirements 1.1, 1.3, 1.4, 1.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 1: Pipeline Stage Completeness
    /// Validates: Requirements 1.1, 1.3, 1.4, 1.5
    ///
    /// For any match operation, all added stages SHALL be present in the final record
    /// with complete field data.
    #[test]
    fn prop_pipeline_stages_are_complete(
        medication in arb_medication_name(),
        stage_count in 1usize..10,
        durations in prop::collection::vec(arb_duration_ms(), 1..10),
        candidates_in_vec in prop::collection::vec(arb_candidate_count(), 1..10),
        candidates_out_vec in prop::collection::vec(arb_candidate_count(), 1..10),
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Add stages with varying data
        let actual_stage_count = stage_count.min(durations.len()).min(candidates_in_vec.len()).min(candidates_out_vec.len());

        for i in 0..actual_stage_count {
            let started_at = Utc::now();
            let completed_at = started_at + Duration::milliseconds(durations[i] as i64);

            let stage = EnhancedPipelineStageRecord::new(
                PipelineStageType::HierarchicalStage { stage_number: (i + 1) as u8 },
                started_at,
                completed_at,
                candidates_in_vec[i],
                candidates_out_vec[i],
                PipelineStageDetails::default(),
            );

            builder.add_enhanced_stage(stage);
        }

        // Build the record
        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify all stages are present
        prop_assert_eq!(
            record.enhanced_stages.len(),
            actual_stage_count,
            "Expected {} stages, got {}",
            actual_stage_count,
            record.enhanced_stages.len()
        );

        // Verify each stage has complete data
        for (i, stage) in record.enhanced_stages.iter().enumerate() {
            // Check stage_type is set
            prop_assert!(
                matches!(stage.stage_type, PipelineStageType::HierarchicalStage { .. }),
                "Stage {} should have HierarchicalStage type",
                i
            );

            // Check duration_ms is set and matches
            prop_assert!(
                stage.duration_ms > 0 || durations[i] == 0,
                "Stage {} duration_ms should be set",
                i
            );

            // Check candidates_in is set
            prop_assert_eq!(
                stage.candidates_in,
                candidates_in_vec[i],
                "Stage {} candidates_in mismatch",
                i
            );

            // Check candidates_out is set
            prop_assert_eq!(
                stage.candidates_out,
                candidates_out_vec[i],
                "Stage {} candidates_out mismatch",
                i
            );

            // Check stage_name is set
            prop_assert!(
                !stage.stage_name.is_empty(),
                "Stage {} should have a non-empty stage_name",
                i
            );
        }

        // Verify legacy stages are also populated for backward compatibility
        prop_assert_eq!(
            record.pipeline_stages.len(),
            actual_stage_count,
            "Legacy pipeline_stages should also have {} stages",
            actual_stage_count
        );
    }

    /// Feature: debug-recording-enhancement, Property 1: Pipeline Stage Completeness
    /// Validates: Requirements 1.3, 1.4
    ///
    /// For any hierarchical matching stage, the record SHALL contain candidate counts
    /// and threshold values.
    #[test]
    fn prop_hierarchical_stages_have_candidate_data(
        medication in arb_medication_name(),
        stage_number in 1u8..=5,
        threshold in arb_score(),
        candidates_in in arb_candidate_count(),
        candidates_out in arb_candidate_count(),
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Create hierarchical stage details
        let mut details = HierarchicalStageDetails::new(
            stage_number,
            format!("stage_{}", stage_number),
            threshold,
        );

        // Add some candidates
        for _ in 0..candidates_out.min(10) {
            details.add_candidate(Uuid::new_v4(), 0.8, true);
        }

        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(100);

        builder.add_hierarchical_stage(
            started_at,
            completed_at,
            candidates_in,
            candidates_out,
            details,
        );

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify hierarchical stage data
        prop_assert_eq!(record.enhanced_stages.len(), 1);

        let stage = &record.enhanced_stages[0];
        match stage.stage_type {
            PipelineStageType::HierarchicalStage { stage_number: n } => {
                prop_assert_eq!(n, stage_number, "Stage number mismatch");
            }
            _ => prop_assert!(false, "Expected HierarchicalStage type"),
        }

        // Verify details contain threshold and candidates
        if let PipelineStageDetails::Hierarchical(h) = &stage.details {
            prop_assert!(
                (h.threshold - threshold).abs() < 0.001,
                "Threshold should be {}, got {}",
                threshold,
                h.threshold
            );
            prop_assert_eq!(h.stage_number, stage_number);
        } else {
            prop_assert!(false, "Expected Hierarchical details");
        }
    }

    /// Feature: debug-recording-enhancement, Property 1: Pipeline Stage Completeness
    /// Validates: Requirements 1.5
    ///
    /// For any score calculation stage, the record SHALL contain the complete
    /// score breakdown with all component weights.
    #[test]
    fn prop_scoring_stage_has_complete_breakdown(
        medication in arb_medication_name(),
        final_score in arb_score(),
        name_score in arb_score(),
        quantity_score in arb_score(),
        price_score in arb_score(),
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Create scoring details
        let mut scoring_details = ScoringDetails::new(final_score, "weighted_sum");
        scoring_details.add_component("name", name_score, 0.4);
        scoring_details.add_component("quantity", quantity_score, 0.3);
        scoring_details.add_component("price", price_score, 0.3);

        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(50);

        builder.add_scoring_stage(started_at, completed_at, 10, scoring_details);

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify scoring stage
        prop_assert_eq!(record.enhanced_stages.len(), 1);

        let stage = &record.enhanced_stages[0];
        prop_assert!(matches!(stage.stage_type, PipelineStageType::ScoreCalculation));

        // Verify scoring details
        if let PipelineStageDetails::Scoring(s) = &stage.details {
            prop_assert!(
                (s.final_score - final_score).abs() < 0.001,
                "Final score should be {}, got {}",
                final_score,
                s.final_score
            );
            prop_assert_eq!(s.components.len(), 3, "Should have 3 score components");
            prop_assert!(s.components.contains_key("name"));
            prop_assert!(s.components.contains_key("quantity"));
            prop_assert!(s.components.contains_key("price"));
        } else {
            prop_assert!(false, "Expected Scoring details");
        }
    }
}

// =============================================================================
// Property 2: AI Recording Completeness
// =============================================================================
// For any match operation where AI parsing or AI review is invoked, the audit
// record SHALL contain an ai_record with model name, latency_ms, and response
// fields, and the ai_involved flag SHALL be true.
//
// Validates: Requirements 1.2, 1.6

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 2: AI Recording Completeness
    /// Validates: Requirements 1.2
    ///
    /// For any AI parsing stage, the record SHALL contain model name, token counts,
    /// latency, and parsed output.
    #[test]
    fn prop_ai_parsing_stage_is_complete(
        medication in arb_medication_name(),
        model_name in arb_model_name(),
        latency_ms in arb_duration_ms(),
        prompt_tokens in 10u32..1000,
        completion_tokens in 10u32..500,
        confidence in arb_score(),
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Create parsing details
        let parsing_details = ParsingDetails::success(
            &model_name,
            latency_ms,
            serde_json::json!({
                "medication": medication,
                "quantity": 10,
            }),
        )
        .with_tokens(prompt_tokens, completion_tokens)
        .with_confidence(confidence);

        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(latency_ms as i64);

        builder.add_parsing_stage(started_at, completed_at, parsing_details);

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify AI parsing stage
        prop_assert_eq!(record.enhanced_stages.len(), 1);

        let stage = &record.enhanced_stages[0];
        prop_assert!(matches!(stage.stage_type, PipelineStageType::AiParsing));
        prop_assert!(stage.involves_ai(), "AI parsing stage should involve AI");

        // Verify parsing details
        if let PipelineStageDetails::Parsing(p) = &stage.details {
            prop_assert_eq!(&p.model_name, &model_name);
            prop_assert_eq!(p.latency_ms, latency_ms);
            prop_assert_eq!(p.prompt_tokens, Some(prompt_tokens));
            prop_assert_eq!(p.completion_tokens, Some(completion_tokens));
            prop_assert_eq!(p.total_tokens, Some(prompt_tokens + completion_tokens));
            prop_assert!(p.success);
            prop_assert!((p.confidence.unwrap() - confidence).abs() < 0.001);
        } else {
            prop_assert!(false, "Expected Parsing details");
        }

        // Verify performance metrics recorded AI timing
        prop_assert!(
            record.performance_metrics.ai_processing_ms.is_some(),
            "AI processing time should be recorded"
        );
    }

    /// Feature: debug-recording-enhancement, Property 2: AI Recording Completeness
    /// Validates: Requirements 1.6
    ///
    /// For any AI review stage, the record SHALL contain model details, decision,
    /// confidence, and reasoning.
    #[test]
    fn prop_ai_review_stage_is_complete(
        medication in arb_medication_name(),
        model_name in arb_model_name(),
        latency_ms in arb_duration_ms(),
        confidence in arb_score(),
        decision in prop_oneof![
            Just("approved".to_string()),
            Just("rejected".to_string()),
            Just("flagged".to_string()),
        ],
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Create AI review details
        let review_details = AiReviewDetails::new(&model_name, &decision, confidence, latency_ms)
            .with_reasoning("Test reasoning for the decision");

        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(latency_ms as i64);

        builder.add_ai_review_stage(started_at, completed_at, 5, review_details);

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify AI review stage
        prop_assert_eq!(record.enhanced_stages.len(), 1);

        let stage = &record.enhanced_stages[0];
        prop_assert!(matches!(stage.stage_type, PipelineStageType::AiReview));
        prop_assert!(stage.involves_ai(), "AI review stage should involve AI");

        // Verify review details
        if let PipelineStageDetails::AiReview(r) = &stage.details {
            prop_assert_eq!(&r.model_name, &model_name);
            prop_assert_eq!(&r.decision, &decision);
            prop_assert!((r.confidence - confidence).abs() < 0.001);
            prop_assert_eq!(r.latency_ms, latency_ms);
            prop_assert!(r.reasoning.is_some());
        } else {
            prop_assert!(false, "Expected AiReview details");
        }
    }

    /// Feature: debug-recording-enhancement, Property 2: AI Recording Completeness
    /// Validates: Requirements 1.6
    ///
    /// For any consensus check stage, the record SHALL contain all auditor decisions,
    /// confidence scores, and the final consensus result.
    #[test]
    fn prop_consensus_stage_is_complete(
        medication in arb_medication_name(),
        total_models in 2usize..5,
        agreement_ratio in 0.5f64..1.0,
        confidence in arb_score(),
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Create consensus details
        let agreeing_models = ((total_models as f64) * agreement_ratio).ceil() as usize;
        let consensus_details = EnhancedConsensusDetails {
            status: "Approved".to_string(),
            confidence,
            agreement_ratio,
            agreeing_models,
            total_models,
            consensus_reached: agreement_ratio >= 0.67,
            model_results: vec![],
            explanation: Some("Test consensus explanation".to_string()),
        };

        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(500);

        builder.add_consensus_stage(started_at, completed_at, 5, consensus_details.clone());

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify consensus stage
        prop_assert_eq!(record.enhanced_stages.len(), 1);

        let stage = &record.enhanced_stages[0];
        prop_assert!(matches!(stage.stage_type, PipelineStageType::ConsensusCheck));

        // Verify consensus details
        if let PipelineStageDetails::Consensus(c) = &stage.details {
            prop_assert_eq!(c.total_models, total_models);
            prop_assert_eq!(c.agreeing_models, agreeing_models);
            prop_assert!((c.agreement_ratio - agreement_ratio).abs() < 0.001);
            prop_assert!((c.confidence - confidence).abs() < 0.001);
        } else {
            prop_assert!(false, "Expected Consensus details");
        }

        // Verify consensus details are also stored at record level
        prop_assert!(record.consensus_details.is_some());
    }

    /// Feature: debug-recording-enhancement, Property 2: AI Recording Completeness
    /// Validates: Requirements 1.2, 1.6
    ///
    /// When AI stages are added, the ai_involved flag SHALL be true in the final record.
    #[test]
    fn prop_ai_involved_flag_is_set(
        medication in arb_medication_name(),
        has_parsing in any::<bool>(),
        has_review in any::<bool>(),
    ) {
        prop_assume!(has_parsing || has_review); // At least one AI stage

        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        if has_parsing {
            let parsing_details = ParsingDetails::success(
                "gpt-4",
                100,
                serde_json::json!({"medication": medication}),
            );
            let started_at = Utc::now();
            let completed_at = started_at + Duration::milliseconds(100);
            builder.add_parsing_stage(started_at, completed_at, parsing_details);
        }

        if has_review {
            let review_details = AiReviewDetails::new("gpt-4", "approved", 0.9, 150);
            let started_at = Utc::now();
            let completed_at = started_at + Duration::milliseconds(150);
            builder.add_ai_review_stage(started_at, completed_at, 5, review_details);
        }

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify ai_involved flag
        prop_assert!(
            record.ai_involved,
            "ai_involved should be true when AI stages are present"
        );

        // Verify at least one stage involves AI
        let ai_stage_count = record.enhanced_stages.iter().filter(|s| s.involves_ai()).count();
        prop_assert!(
            ai_stage_count > 0,
            "Should have at least one AI-involving stage"
        );
    }
}

// =============================================================================
// Property 11: Performance Metrics Capture
// =============================================================================
// For any match operation, the audit record SHALL capture total_latency_ms,
// and when AI processing occurs, ai_queue_wait_ms and ai_processing_ms SHALL
// be recorded separately.
//
// Validates: Requirements 8.1, 8.2, 8.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 11: Performance Metrics Capture
    /// Validates: Requirements 8.1
    ///
    /// For any match operation, memory usage SHALL be tracked and peak memory
    /// SHALL be recorded in performance metrics.
    #[test]
    fn prop_memory_usage_is_tracked(
        memory_samples in prop::collection::vec(1000u64..1000000, 1..10),
    ) {
        let tracker = PerformanceTracker::new();

        let mut expected_peak = 0u64;
        for &sample in &memory_samples {
            tracker.record_memory(sample);
            expected_peak = expected_peak.max(sample);
        }

        prop_assert_eq!(
            tracker.peak_memory_bytes(),
            expected_peak,
            "Peak memory should be the maximum of all samples"
        );

        let metrics = tracker.to_metrics();
        prop_assert_eq!(
            metrics.memory_peak_bytes,
            Some(expected_peak),
            "Metrics should contain peak memory"
        );
    }

    /// Feature: debug-recording-enhancement, Property 11: Performance Metrics Capture
    /// Validates: Requirements 8.2
    ///
    /// When AI processing occurs, ai_queue_wait_ms and ai_processing_ms SHALL
    /// be recorded separately.
    #[test]
    fn prop_ai_timing_is_separated(
        queue_waits in prop::collection::vec(1u64..100, 1..5),
        processing_times in prop::collection::vec(10u64..500, 1..5),
    ) {
        let tracker = PerformanceTracker::new();

        let expected_queue_total: u64 = queue_waits.iter().sum();
        let expected_processing_total: u64 = processing_times.iter().sum();

        for &wait in &queue_waits {
            tracker.record_ai_queue_wait(wait);
        }

        for &processing in &processing_times {
            tracker.record_ai_processing(processing);
        }

        prop_assert_eq!(
            tracker.ai_queue_wait_ms(),
            expected_queue_total,
            "AI queue wait time should be sum of all waits"
        );

        prop_assert_eq!(
            tracker.ai_processing_ms(),
            expected_processing_total,
            "AI processing time should be sum of all processing times"
        );

        prop_assert_eq!(
            tracker.total_ai_time_ms(),
            expected_queue_total + expected_processing_total,
            "Total AI time should be queue + processing"
        );

        let metrics = tracker.to_metrics();
        prop_assert_eq!(
            metrics.ai_queue_wait_ms,
            Some(expected_queue_total),
            "Metrics should contain AI queue wait time"
        );
        prop_assert_eq!(
            metrics.ai_processing_ms,
            Some(expected_processing_total),
            "Metrics should contain AI processing time"
        );
    }

    /// Feature: debug-recording-enhancement, Property 11: Performance Metrics Capture
    /// Validates: Requirements 8.3
    ///
    /// When database queries execute, query count and total duration SHALL be recorded.
    #[test]
    fn prop_db_queries_are_tracked(
        query_durations in prop::collection::vec(1u64..100, 1..20),
    ) {
        let tracker = PerformanceTracker::new();

        let expected_count = query_durations.len() as u64;
        let expected_total: u64 = query_durations.iter().sum();

        for &duration in &query_durations {
            tracker.record_db_query(duration);
        }

        prop_assert_eq!(
            tracker.db_query_count(),
            expected_count,
            "DB query count should match number of queries"
        );

        prop_assert_eq!(
            tracker.db_total_ms(),
            expected_total,
            "DB total time should be sum of all query durations"
        );

        let metrics = tracker.to_metrics();
        prop_assert_eq!(
            metrics.db_query_count,
            expected_count as u32,
            "Metrics should contain DB query count"
        );
        prop_assert_eq!(
            metrics.db_total_ms,
            expected_total,
            "Metrics should contain DB total time"
        );
    }

    /// Feature: debug-recording-enhancement, Property 11: Performance Metrics Capture
    /// Validates: Requirements 8.1, 8.2, 8.3
    ///
    /// Performance metrics in audit record SHALL contain all tracked data.
    #[test]
    fn prop_audit_record_contains_performance_metrics(
        medication in prop_oneof![
            Just("Aspirin".to_string()),
            Just("Paracetamol".to_string()),
        ],
        ai_latency_ms in 10u64..500,
    ) {
        let offer = create_test_offer(&medication);
        let request = create_test_request(&medication);
        let weights = NormalizedWeights::default();
        let match_id = Uuid::new_v4();

        let mut builder = AuditRecordBuilder::new(match_id, offer, request, weights.clone());

        // Record some performance data
        builder.record_memory(50000);
        builder.record_ai_timing(10, ai_latency_ms);
        builder.record_db_query(5);
        builder.record_db_query(10);

        // Add an AI parsing stage
        let parsing_details = ParsingDetails::success(
            "gpt-4",
            ai_latency_ms,
            serde_json::json!({"medication": medication}),
        );
        let started_at = Utc::now();
        let completed_at = started_at + Duration::milliseconds(ai_latency_ms as i64);
        builder.add_parsing_stage(started_at, completed_at, parsing_details);

        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6, 0.5);
        let record = builder.build_enhanced(&breakdown);

        // Verify performance metrics are present
        prop_assert!(
            record.performance_metrics.memory_peak_bytes.is_some(),
            "Memory peak should be recorded"
        );
        prop_assert_eq!(
            record.performance_metrics.memory_peak_bytes,
            Some(50000),
            "Memory peak should be 50000"
        );

        // AI timing should include both explicit recording and stage recording
        prop_assert!(
            record.performance_metrics.ai_processing_ms.is_some(),
            "AI processing time should be recorded"
        );

        // DB queries should be recorded
        prop_assert_eq!(
            record.performance_metrics.db_query_count,
            2,
            "Should have 2 DB queries"
        );
        prop_assert_eq!(
            record.performance_metrics.db_total_ms,
            15,
            "DB total time should be 15ms"
        );

        // Stage latencies should be recorded
        prop_assert!(
            !record.performance_metrics.stage_latencies.is_empty(),
            "Stage latencies should be recorded"
        );
    }

    /// Feature: debug-recording-enhancement, Property 11: Performance Metrics Capture
    /// Validates: Requirements 8.1, 8.2, 8.3
    ///
    /// PipelinePerformanceMetrics methods SHALL correctly aggregate data.
    #[test]
    fn prop_performance_metrics_aggregation(
        memory_values in prop::collection::vec(1000u64..100000, 1..5),
        ai_timing_count in 1usize..5,
        ai_queue_base in 1u64..50,
        ai_processing_base in 10u64..200,
        db_durations in prop::collection::vec(1u64..50, 1..5),
    ) {
        let mut metrics = PipelinePerformanceMetrics::new();

        // Record memory (should track peak)
        let expected_peak = *memory_values.iter().max().unwrap_or(&0);
        for &mem in &memory_values {
            metrics.record_memory(mem);
        }

        // Record AI timing (should accumulate)
        let expected_queue: u64 = ai_queue_base * ai_timing_count as u64;
        let expected_processing: u64 = ai_processing_base * ai_timing_count as u64;
        for _ in 0..ai_timing_count {
            metrics.record_ai_timing(ai_queue_base, ai_processing_base);
        }

        // Record DB queries (should count and accumulate)
        let expected_db_count = db_durations.len() as u32;
        let expected_db_total: u64 = db_durations.iter().sum();
        for &duration in &db_durations {
            metrics.record_db_query(duration);
        }

        // Verify aggregation
        prop_assert_eq!(
            metrics.memory_peak_bytes,
            Some(expected_peak),
            "Memory peak should be maximum"
        );

        prop_assert_eq!(
            metrics.ai_queue_wait_ms,
            Some(expected_queue),
            "AI queue wait should be sum"
        );

        prop_assert_eq!(
            metrics.ai_processing_ms,
            Some(expected_processing),
            "AI processing should be sum"
        );

        prop_assert_eq!(
            metrics.total_ai_time_ms(),
            expected_queue + expected_processing,
            "Total AI time should be queue + processing"
        );

        prop_assert_eq!(
            metrics.db_query_count,
            expected_db_count,
            "DB query count should match"
        );

        prop_assert_eq!(
            metrics.db_total_ms,
            expected_db_total,
            "DB total time should be sum"
        );
    }
}
