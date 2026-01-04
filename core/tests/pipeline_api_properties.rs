//! Property-based tests for Pipeline Visualization API
//!
//! Feature: debug-recording-enhancement
//! Tests Property 8 from the design document
//!
//! Property 8: Pipeline API Response Completeness
//! For any audit record with pipeline stages, the pipeline visualization API
//! SHALL return all stages with timing information, and when hierarchical
//! stages exist, candidate lists with scores SHALL be included.
//!
//! Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5
//!
//! Run with: cargo test --features test-pipeline-api-props --test pipeline_api_properties

#![cfg(feature = "test-pipeline-api-props")]

use chrono::{Duration, Utc};
use pharma_core::api::pipeline_visualization::{
    PipelineVisualizationResponse, transform_to_visualization,
};
use pharma_core::matching::{
    AIInvolvementRecord, AiReviewDetails, CalibrationDetails, CandidateScore,
    EnhancedConsensusDetails, EnhancedContrastiveDetails, HierarchicalStageDetails,
    MatchAuditRecord, ModelAuditDetail, PipelineResolutionDetails, PipelineStageRecord,
    ResolutionStageResult, ScoringDetails,
};
use proptest::prelude::*;
use uuid::Uuid;

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate a random duration in milliseconds
fn arb_duration_ms() -> impl Strategy<Value = u64> {
    1u64..10000
}

/// Generate a random candidate count
fn arb_candidate_count() -> impl Strategy<Value = usize> {
    1usize..100
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

/// Generate a random stage name
fn arb_stage_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("message_received".to_string()),
        Just("ai_parsing".to_string()),
        Just("parsing_complete".to_string()),
        Just("medication_resolution".to_string()),
        Just("hierarchical_stage_1".to_string()),
        Just("hierarchical_stage_2".to_string()),
        Just("score_calculation".to_string()),
        Just("ai_review".to_string()),
        Just("consensus_check".to_string()),
        Just("calibration".to_string()),
    ]
}

/// Generate a pipeline stage record
fn arb_pipeline_stage() -> impl Strategy<Value = PipelineStageRecord> {
    (
        arb_stage_name(),
        arb_duration_ms(),
        arb_candidate_count(),
        arb_candidate_count(),
    )
        .prop_map(
            |(stage, duration_ms, candidates_in, candidates_out)| PipelineStageRecord {
                stage,
                started_at: Utc::now(),
                duration_ms,
                candidates_in,
                candidates_out,
                details: None,
            },
        )
}

/// Generate a hierarchical stage with details
fn arb_hierarchical_stage_with_details() -> impl Strategy<Value = PipelineStageRecord> {
    (
        1u8..=5,
        arb_score(),
        arb_duration_ms(),
        arb_candidate_count(),
        prop::collection::vec((any::<u128>(), arb_score(), any::<bool>()), 1..10),
    )
        .prop_map(
            |(stage_number, threshold, duration_ms, candidates_in, candidates)| {
                let mut details = HierarchicalStageDetails::new(
                    stage_number,
                    format!("stage_{}", stage_number),
                    threshold,
                );
                for (id_bits, score, passed) in candidates {
                    details.add_candidate(Uuid::from_u128(id_bits), score, passed);
                }
                let candidates_out = details.candidates.iter().filter(|c| c.passed).count();

                PipelineStageRecord {
                    stage: format!("hierarchical_stage_{}", stage_number),
                    started_at: Utc::now(),
                    duration_ms,
                    candidates_in,
                    candidates_out,
                    details: Some(serde_json::to_value(&details).unwrap()),
                }
            },
        )
}

/// Generate an AI review stage with details
fn arb_ai_review_stage() -> impl Strategy<Value = PipelineStageRecord> {
    (
        arb_model_name(),
        arb_score(),
        arb_duration_ms(),
        prop_oneof![
            Just("approved".to_string()),
            Just("rejected".to_string()),
            Just("flagged".to_string()),
        ],
    )
        .prop_map(|(model_name, confidence, latency_ms, decision)| {
            let details = AiReviewDetails::new(&model_name, &decision, confidence, latency_ms)
                .with_reasoning("Test reasoning");

            PipelineStageRecord {
                stage: "ai_review".to_string(),
                started_at: Utc::now(),
                duration_ms: latency_ms,
                candidates_in: 5,
                candidates_out: 5,
                details: Some(serde_json::to_value(&details).unwrap()),
            }
        })
}

/// Generate a scoring stage with details
fn arb_scoring_stage() -> impl Strategy<Value = PipelineStageRecord> {
    (
        arb_score(),
        arb_score(),
        arb_score(),
        arb_score(),
        arb_duration_ms(),
    )
        .prop_map(
            |(final_score, name_score, quantity_score, price_score, duration_ms)| {
                let mut details = ScoringDetails::new(final_score, "weighted_sum");
                details.add_component("name", name_score, 0.4);
                details.add_component("quantity", quantity_score, 0.3);
                details.add_component("price", price_score, 0.3);

                PipelineStageRecord {
                    stage: "score_calculation".to_string(),
                    started_at: Utc::now(),
                    duration_ms,
                    candidates_in: 10,
                    candidates_out: 10,
                    details: Some(serde_json::to_value(&details).unwrap()),
                }
            },
        )
}

/// Generate a basic audit record
fn arb_basic_audit_record() -> impl Strategy<Value = MatchAuditRecord> {
    (
        arb_medication_name(),
        arb_score(),
        arb_duration_ms(),
        prop::collection::vec(arb_pipeline_stage(), 1..5),
    )
        .prop_map(
            |(medication, final_score, total_latency_ms, stages)| MatchAuditRecord {
                id: Uuid::new_v4(),
                match_id: Uuid::new_v4(),
                offer_id: Uuid::new_v4(),
                request_id: Uuid::new_v4(),
                pipeline_version: "1.0.0".to_string(),
                offer_snapshot: serde_json::json!({"medication": medication}),
                request_snapshot: serde_json::json!({"medication": medication}),
                weights_snapshot: serde_json::json!({}),
                config_snapshot: None,
                score_breakdown: serde_json::json!({
                    "name": final_score,
                    "quantity": final_score * 0.9,
                    "price": final_score * 0.8
                }),
                final_score,
                pipeline_stages: stages,
                ai_involved: false,
                ai_record: None,
                resolution_stage: "exact_match".to_string(),
                resolution_details: None,
                total_latency_ms,
                created_at: Utc::now(),
                review_status: None,
                reviewed_by: None,
                reviewed_at: None,
                review_notes: None,
                session_id: None,
                client_metadata: None,
            },
        )
}

/// Generate an audit record with hierarchical stages
fn arb_audit_record_with_hierarchical() -> impl Strategy<Value = MatchAuditRecord> {
    (
        arb_medication_name(),
        arb_score(),
        arb_duration_ms(),
        prop::collection::vec(arb_hierarchical_stage_with_details(), 1..4),
    )
        .prop_map(
            |(medication, final_score, total_latency_ms, stages)| MatchAuditRecord {
                id: Uuid::new_v4(),
                match_id: Uuid::new_v4(),
                offer_id: Uuid::new_v4(),
                request_id: Uuid::new_v4(),
                pipeline_version: "1.0.0".to_string(),
                offer_snapshot: serde_json::json!({"medication": medication}),
                request_snapshot: serde_json::json!({"medication": medication}),
                weights_snapshot: serde_json::json!({}),
                config_snapshot: None,
                score_breakdown: serde_json::json!({
                    "name": final_score,
                    "quantity": final_score * 0.9,
                    "price": final_score * 0.8
                }),
                final_score,
                pipeline_stages: stages,
                ai_involved: false,
                ai_record: None,
                resolution_stage: "hierarchical".to_string(),
                resolution_details: None,
                total_latency_ms,
                created_at: Utc::now(),
                review_status: None,
                reviewed_by: None,
                reviewed_at: None,
                review_notes: None,
                session_id: None,
                client_metadata: None,
            },
        )
}

/// Generate an audit record with AI involvement
fn arb_audit_record_with_ai() -> impl Strategy<Value = MatchAuditRecord> {
    (
        arb_medication_name(),
        arb_score(),
        arb_duration_ms(),
        arb_model_name(),
        arb_ai_review_stage(),
    )
        .prop_map(
            |(medication, final_score, total_latency_ms, model_name, ai_stage)| MatchAuditRecord {
                id: Uuid::new_v4(),
                match_id: Uuid::new_v4(),
                offer_id: Uuid::new_v4(),
                request_id: Uuid::new_v4(),
                pipeline_version: "1.0.0".to_string(),
                offer_snapshot: serde_json::json!({"medication": medication}),
                request_snapshot: serde_json::json!({"medication": medication}),
                weights_snapshot: serde_json::json!({}),
                config_snapshot: None,
                score_breakdown: serde_json::json!({
                    "name": final_score,
                    "quantity": final_score * 0.9,
                    "price": final_score * 0.8
                }),
                final_score,
                pipeline_stages: vec![ai_stage],
                ai_involved: true,
                ai_record: Some(AIInvolvementRecord {
                    model: model_name,
                    prompt_tokens: Some(100),
                    completion_tokens: Some(50),
                    latency_ms: 150,
                    response: serde_json::json!({"medication": medication}),
                }),
                resolution_stage: "ai_review".to_string(),
                resolution_details: None,
                total_latency_ms,
                created_at: Utc::now(),
                review_status: None,
                reviewed_by: None,
                reviewed_at: None,
                review_notes: None,
                session_id: None,
                client_metadata: None,
            },
        )
}

// =============================================================================
// Property 8: Pipeline API Response Completeness
// =============================================================================
// For any audit record with pipeline stages, the pipeline visualization API
// SHALL return all stages with timing information, and when hierarchical
// stages exist, candidate lists with scores SHALL be included.
//
// Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 8: Pipeline API Response Completeness
    /// Validates: Requirements 4.1
    ///
    /// For any audit record with pipeline stages, the visualization response
    /// SHALL contain all stages with timing information.
    #[test]
    fn prop_all_stages_have_timing_info(
        record in arb_basic_audit_record(),
    ) {
        let viz = transform_to_visualization(&record);

        // Verify all stages are present
        prop_assert_eq!(
            viz.stages.len(),
            record.pipeline_stages.len(),
            "Visualization should have same number of stages as record"
        );

        // Verify each stage has timing information
        for (i, stage) in viz.stages.iter().enumerate() {
            let original = &record.pipeline_stages[i];

            // Check stage name matches
            prop_assert_eq!(
                &stage.stage_name,
                &original.stage,
                "Stage {} name should match",
                i
            );

            // Check duration is present
            prop_assert_eq!(
                stage.duration_ms,
                original.duration_ms,
                "Stage {} duration should match",
                i
            );

            // Check started_at is present (non-empty string)
            prop_assert!(
                !stage.started_at.is_empty(),
                "Stage {} should have started_at timestamp",
                i
            );

            // Check status is set
            prop_assert!(
                !stage.status.is_empty(),
                "Stage {} should have status",
                i
            );

            // Check candidate counts
            prop_assert_eq!(
                stage.candidates_in,
                original.candidates_in,
                "Stage {} candidates_in should match",
                i
            );
            prop_assert_eq!(
                stage.candidates_out,
                original.candidates_out,
                "Stage {} candidates_out should match",
                i
            );
        }
    }

    /// Feature: debug-recording-enhancement, Property 8: Pipeline API Response Completeness
    /// Validates: Requirements 4.2
    ///
    /// When hierarchical stages exist, candidate lists with scores SHALL be included.
    #[test]
    fn prop_hierarchical_stages_have_candidates(
        record in arb_audit_record_with_hierarchical(),
    ) {
        let viz = transform_to_visualization(&record);

        // Check that hierarchical details are present
        prop_assert!(
            viz.hierarchical_details.is_some(),
            "Hierarchical details should be present when hierarchical stages exist"
        );

        let hierarchical = viz.hierarchical_details.as_ref().unwrap();

        // Verify hierarchical stages have candidate data
        for h_stage in hierarchical {
            // Check stage number is valid
            prop_assert!(
                h_stage.stage_number >= 1 && h_stage.stage_number <= 5,
                "Stage number should be between 1 and 5"
            );

            // Check threshold is valid
            prop_assert!(
                h_stage.threshold >= 0.0 && h_stage.threshold <= 1.0,
                "Threshold should be between 0 and 1"
            );

            // Check candidates have scores
            for candidate in &h_stage.candidates {
                prop_assert!(
                    candidate.score >= 0.0 && candidate.score <= 1.0,
                    "Candidate score should be between 0 and 1"
                );
            }

            // Check has_matches is consistent with candidates
            let passing_count = h_stage.candidates.iter().filter(|c| c.passed).count();
            if passing_count > 0 {
                prop_assert!(
                    h_stage.has_matches,
                    "has_matches should be true when candidates pass"
                );
            }
        }
    }

    /// Feature: debug-recording-enhancement, Property 8: Pipeline API Response Completeness
    /// Validates: Requirements 4.3
    ///
    /// When AI review data exists, the API SHALL return model details,
    /// reasoning, and confidence scores.
    #[test]
    fn prop_ai_review_has_complete_details(
        record in arb_audit_record_with_ai(),
    ) {
        let viz = transform_to_visualization(&record);

        // Check that AI review is present
        prop_assert!(
            viz.ai_review.is_some(),
            "AI review should be present when AI is involved"
        );

        let ai_review = viz.ai_review.as_ref().unwrap();

        // Check model name is present
        prop_assert!(
            !ai_review.model_name.is_empty(),
            "AI review should have model name"
        );

        // Check latency is present
        prop_assert!(
            ai_review.latency_ms > 0,
            "AI review should have latency"
        );

        // Check ai_involved flag is set
        prop_assert!(
            viz.ai_involved,
            "ai_involved should be true when AI review exists"
        );
    }

    /// Feature: debug-recording-enhancement, Property 8: Pipeline API Response Completeness
    /// Validates: Requirements 4.4
    ///
    /// When score breakdown is requested, the API SHALL return all component
    /// scores with their weights and formulas.
    #[test]
    fn prop_score_breakdown_has_components(
        record in arb_basic_audit_record(),
    ) {
        let viz = transform_to_visualization(&record);

        // Check that score breakdown is present
        prop_assert!(
            viz.score_breakdown.is_some(),
            "Score breakdown should be present"
        );

        let breakdown = viz.score_breakdown.as_ref().unwrap();

        // Check final score matches
        prop_assert!(
            (breakdown.final_score - record.final_score).abs() < 0.001,
            "Final score should match record"
        );

        // Check formula is present
        prop_assert!(
            !breakdown.formula.is_empty(),
            "Score breakdown should have formula"
        );

        // Check components are present
        prop_assert!(
            !breakdown.components.is_empty(),
            "Score breakdown should have components"
        );

        // Check each component has required fields
        for component in &breakdown.components {
            prop_assert!(
                !component.name.is_empty(),
                "Component should have name"
            );
            prop_assert!(
                component.raw_score >= 0.0 && component.raw_score <= 1.0,
                "Component raw_score should be between 0 and 1"
            );
            prop_assert!(
                component.weight >= 0.0,
                "Component weight should be non-negative"
            );
        }
    }

    /// Feature: debug-recording-enhancement, Property 8: Pipeline API Response Completeness
    /// Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5
    ///
    /// The visualization response SHALL contain all required metadata fields.
    #[test]
    fn prop_response_has_required_metadata(
        record in arb_basic_audit_record(),
    ) {
        let viz = transform_to_visualization(&record);

        // Check match_id matches
        prop_assert_eq!(
            viz.match_id,
            record.match_id,
            "match_id should match"
        );

        // Check offer_id matches
        prop_assert_eq!(
            viz.offer_id,
            record.offer_id,
            "offer_id should match"
        );

        // Check request_id matches
        prop_assert_eq!(
            viz.request_id,
            record.request_id,
            "request_id should match"
        );

        // Check pipeline_version is present
        prop_assert!(
            !viz.pipeline_version.is_empty(),
            "pipeline_version should be present"
        );

        // Check final_score matches
        prop_assert!(
            (viz.final_score - record.final_score).abs() < 0.001,
            "final_score should match"
        );

        // Check total_latency_ms matches
        prop_assert_eq!(
            viz.total_latency_ms,
            record.total_latency_ms,
            "total_latency_ms should match"
        );

        // Check resolution_stage matches
        prop_assert_eq!(
            &viz.resolution_stage,
            &record.resolution_stage,
            "resolution_stage should match"
        );

        // Check created_at is present
        prop_assert!(
            !viz.created_at.is_empty(),
            "created_at should be present"
        );

        // Check performance_metrics has stage_latencies
        // Note: stage_latencies uses stage name as key, so duplicates are merged
        let unique_stage_names: std::collections::HashSet<_> = record
            .pipeline_stages
            .iter()
            .map(|s| s.stage.clone())
            .collect();
        prop_assert_eq!(
            viz.performance_metrics.stage_latencies.len(),
            unique_stage_names.len(),
            "stage_latencies should have entry for each unique stage name"
        );
    }
}
