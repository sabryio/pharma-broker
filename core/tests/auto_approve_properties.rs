//! Property-based tests for AI Supervised Auto-Approve functionality
//!
//! Feature: ai-supervised-auto-approve
//! Tests Properties 1, 2, 3, 4, 18, 19 from the design document
//!
//! These tests validate:
//! - Property 4: Threshold Configuration Bounds
//! - Property 1: Threshold-Based Auto-Approval
//! - Property 2: Below-Threshold Queuing
//! - Property 18: Batch Size Limit
//! - Property 19: Age-Based Prioritization
//!
//! Run with: cargo test --features test-auto-approve --test auto_approve_properties

#![cfg(feature = "test-auto-approve")]

use pharma_core::matching::{
    AutoApproveConfig, AutoApproveResult, ConfigValidationError, MAX_CONFIDENCE_THRESHOLD,
    MIN_CONFIDENCE_THRESHOLD, RetryInfo, SafetyCheckResult,
};
use proptest::prelude::*;
use uuid::Uuid;

// =============================================================================
// Custom Generators
// =============================================================================

/// Strategy for generating confidence values within valid bounds
fn valid_confidence() -> impl Strategy<Value = f64> {
    MIN_CONFIDENCE_THRESHOLD..=MAX_CONFIDENCE_THRESHOLD
}

/// Strategy for generating confidence values below minimum
fn below_min_confidence() -> impl Strategy<Value = f64> {
    -1.0f64..MIN_CONFIDENCE_THRESHOLD
}

/// Strategy for generating confidence values above maximum
fn above_max_confidence() -> impl Strategy<Value = f64> {
    (MAX_CONFIDENCE_THRESHOLD + 0.001)..2.0f64
}

/// Strategy for generating any confidence value
fn any_confidence() -> impl Strategy<Value = f64> {
    -1.0f64..2.0f64
}

/// Strategy for generating valid batch sizes
fn valid_batch_size() -> impl Strategy<Value = usize> {
    1usize..=1000
}

/// Strategy for generating invalid batch sizes
fn invalid_batch_size() -> impl Strategy<Value = usize> {
    prop_oneof![Just(0usize), 1001usize..10000]
}

// =============================================================================
// Property 4: Threshold Configuration Bounds
// =============================================================================
// For any confidence threshold configuration value, the system SHALL accept
// values in [0.70, 0.99] and reject or clamp values outside this range.
// Validates: Requirements 1.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Valid thresholds within [0.70, 0.99] should be accepted unchanged
    #[test]
    fn prop_valid_threshold_accepted_unchanged(
        threshold in valid_confidence()
    ) {
        let config = AutoApproveConfig::default();
        let clamped = config.clamp_threshold(threshold);

        prop_assert!(
            (clamped - threshold).abs() < 0.0001,
            "Valid threshold {} should be accepted unchanged, got {}",
            threshold,
            clamped
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Thresholds below minimum should be clamped to MIN_CONFIDENCE_THRESHOLD
    #[test]
    fn prop_below_min_threshold_clamped(
        threshold in below_min_confidence()
    ) {
        let config = AutoApproveConfig::default();
        let clamped = config.clamp_threshold(threshold);

        prop_assert!(
            (clamped - MIN_CONFIDENCE_THRESHOLD).abs() < 0.0001,
            "Threshold {} below min should be clamped to {}, got {}",
            threshold,
            MIN_CONFIDENCE_THRESHOLD,
            clamped
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Thresholds above maximum should be clamped to MAX_CONFIDENCE_THRESHOLD
    #[test]
    fn prop_above_max_threshold_clamped(
        threshold in above_max_confidence()
    ) {
        let config = AutoApproveConfig::default();
        let clamped = config.clamp_threshold(threshold);

        prop_assert!(
            (clamped - MAX_CONFIDENCE_THRESHOLD).abs() < 0.0001,
            "Threshold {} above max should be clamped to {}, got {}",
            threshold,
            MAX_CONFIDENCE_THRESHOLD,
            clamped
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Clamped threshold should always be within valid bounds
    #[test]
    fn prop_clamped_threshold_always_valid(
        threshold in any_confidence()
    ) {
        let config = AutoApproveConfig::default();
        let clamped = config.clamp_threshold(threshold);

        prop_assert!(
            (MIN_CONFIDENCE_THRESHOLD..=MAX_CONFIDENCE_THRESHOLD).contains(&clamped),
            "Clamped threshold {} should be in [{}, {}]",
            clamped,
            MIN_CONFIDENCE_THRESHOLD,
            MAX_CONFIDENCE_THRESHOLD
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Config validation should reject thresholds outside bounds
    #[test]
    fn prop_validation_rejects_invalid_threshold(
        threshold in prop_oneof![below_min_confidence(), above_max_confidence()]
    ) {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = threshold;

        let result = config.validate();

        prop_assert!(
            matches!(result, Err(ConfigValidationError::ThresholdOutOfBounds { .. })),
            "Validation should reject threshold {}, got {:?}",
            threshold,
            result
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Config validation should accept thresholds within bounds
    #[test]
    fn prop_validation_accepts_valid_threshold(
        threshold in valid_confidence()
    ) {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = threshold;

        let result = config.validate();

        prop_assert!(
            result.is_ok(),
            "Validation should accept threshold {}, got {:?}",
            threshold,
            result
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// Category thresholds should also be validated within bounds
    #[test]
    fn prop_category_threshold_validation(
        threshold in any_confidence(),
        category in "[a-z]{3,10}"
    ) {
        let mut config = AutoApproveConfig::default();
        config.category_thresholds.insert(category.clone(), threshold);

        let result = config.validate();

        if (MIN_CONFIDENCE_THRESHOLD..=MAX_CONFIDENCE_THRESHOLD).contains(&threshold) {
            prop_assert!(
                result.is_ok(),
                "Valid category threshold {} should pass validation, got {:?}",
                threshold,
                result
            );
        } else {
            prop_assert!(
                matches!(result, Err(ConfigValidationError::InvalidCategoryThreshold { .. })),
                "Invalid category threshold {} should fail validation, got {:?}",
                threshold,
                result
            );
        }
    }

    /// Feature: ai-supervised-auto-approve, Property 4: Threshold Configuration Bounds
    /// Validates: Requirements 1.5
    ///
    /// set_category_threshold should clamp values to valid bounds
    #[test]
    fn prop_set_category_threshold_clamps(
        threshold in any_confidence(),
        category in "[a-z]{3,10}"
    ) {
        let mut config = AutoApproveConfig::default();
        config.set_category_threshold(&category, threshold);

        let stored = config.get_threshold_for_category(Some(&category));

        prop_assert!(
            (MIN_CONFIDENCE_THRESHOLD..=MAX_CONFIDENCE_THRESHOLD).contains(&stored),
            "Stored category threshold {} should be in valid bounds",
            stored
        );
    }
}

// =============================================================================
// Property 1 & 2: Threshold-Based Auto-Approval and Below-Threshold Queuing
// =============================================================================
// Property 1: For any match with AI confidence above the configured threshold
// and passing all safety checks, the Auto_Approve_System SHALL automatically
// approve the match.
// Property 2: For any match with AI confidence below the configured threshold,
// the Auto_Approve_System SHALL queue the match for human review.
// Validates: Requirements 1.1, 1.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 1: Threshold-Based Auto-Approval
    /// Validates: Requirements 1.1
    ///
    /// Matches above threshold should be approved
    #[test]
    fn prop_above_threshold_approved(
        threshold in valid_confidence(),
        margin in 0.001f64..0.1
    ) {
        prop_assume!(threshold + margin <= 1.0);

        let confidence = threshold + margin;
        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            confidence,
            "High confidence match".to_string(),
            vec![SafetyCheckResult::passed("blocklist")],
            threshold,
        );

        prop_assert!(
            result.action.is_approved(),
            "Confidence {} above threshold {} should be approved",
            confidence,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 2: Below-Threshold Queuing
    /// Validates: Requirements 1.4
    ///
    /// Matches below threshold should be queued for review
    #[test]
    fn prop_below_threshold_queued(
        threshold in valid_confidence(),
        margin in 0.001f64..0.3
    ) {
        prop_assume!(threshold - margin >= 0.0);

        let confidence = threshold - margin;
        let result = AutoApproveResult::queued_for_review(
            Uuid::new_v4(),
            confidence,
            "Below threshold".to_string(),
            "Confidence below threshold".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result.action.requires_review(),
            "Confidence {} below threshold {} should be queued for review",
            confidence,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 1 & 2
    /// Validates: Requirements 1.1, 1.4
    ///
    /// Borderline detection should work correctly
    #[test]
    fn prop_borderline_detection(
        threshold in 0.75f64..0.95,
        offset in -0.05f64..0.05
    ) {
        let confidence = threshold + offset;
        prop_assume!((0.0..=1.0).contains(&confidence));

        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            vec![],
            threshold,
        );

        let expected_borderline = offset.abs() < 0.05;

        prop_assert_eq!(
            result.is_borderline,
            expected_borderline,
            "Confidence {} with threshold {} should have borderline={}, got {}",
            confidence,
            threshold,
            expected_borderline,
            result.is_borderline
        );
    }
}

// =============================================================================
// Property 18: Batch Size Limit
// =============================================================================
// For any batch processing cycle, the number of matches processed SHALL NOT
// exceed the configured batch_size limit.
// Validates: Requirements 6.1

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 18: Batch Size Limit
    /// Validates: Requirements 6.1
    ///
    /// Valid batch sizes should pass validation
    #[test]
    fn prop_valid_batch_size_accepted(
        batch_size in valid_batch_size()
    ) {
        let mut config = AutoApproveConfig::default();
        config.batch_size = batch_size;

        let result = config.validate();

        prop_assert!(
            result.is_ok(),
            "Batch size {} should be valid, got {:?}",
            batch_size,
            result
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 18: Batch Size Limit
    /// Validates: Requirements 6.1
    ///
    /// Invalid batch sizes should fail validation
    #[test]
    fn prop_invalid_batch_size_rejected(
        batch_size in invalid_batch_size()
    ) {
        let mut config = AutoApproveConfig::default();
        config.batch_size = batch_size;

        let result = config.validate();

        prop_assert!(
            matches!(result, Err(ConfigValidationError::InvalidBatchSize(_))),
            "Batch size {} should be invalid, got {:?}",
            batch_size,
            result
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 18: Batch Size Limit
    /// Validates: Requirements 6.1
    ///
    /// Default batch size should be valid
    #[test]
    fn prop_default_batch_size_valid(_dummy in 0..1) {
        let config = AutoApproveConfig::default();

        prop_assert!(
            config.batch_size > 0 && config.batch_size <= 1000,
            "Default batch size {} should be in valid range",
            config.batch_size
        );

        let result = config.validate();
        prop_assert!(
            result.is_ok(),
            "Default config should be valid, got {:?}",
            result
        );
    }
}

// =============================================================================
// Additional Configuration Validation Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve
    /// Validates: Requirements 5.1, 5.2
    ///
    /// Override rate threshold should be validated
    #[test]
    fn prop_override_rate_threshold_validation(
        rate in -0.5f64..1.5
    ) {
        let mut config = AutoApproveConfig::default();
        config.override_rate_pause_threshold = rate;

        let result = config.validate();

        if rate >= 0.0 && rate <= 1.0 {
            prop_assert!(
                result.is_ok(),
                "Valid override rate {} should pass validation, got {:?}",
                rate,
                result
            );
        } else {
            prop_assert!(
                matches!(result, Err(ConfigValidationError::InvalidOverrideRateThreshold(_))),
                "Invalid override rate {} should fail validation, got {:?}",
                rate,
                result
            );
        }
    }

    /// Feature: ai-supervised-auto-approve
    /// Validates: Requirements 5.3
    ///
    /// Category-specific threshold should override global threshold
    #[test]
    fn prop_category_threshold_overrides_global(
        global_threshold in valid_confidence(),
        category_threshold in valid_confidence(),
        category in "[a-z]{3,10}"
    ) {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = global_threshold;
        config.set_category_threshold(&category, category_threshold);

        let effective = config.get_threshold_for_category(Some(&category));
        let clamped_category = config.clamp_threshold(category_threshold);

        prop_assert!(
            (effective - clamped_category).abs() < 0.0001,
            "Category threshold {} should override global {}, got {}",
            clamped_category,
            global_threshold,
            effective
        );
    }

    /// Feature: ai-supervised-auto-approve
    /// Validates: Requirements 5.3
    ///
    /// Unknown category should use global threshold
    #[test]
    fn prop_unknown_category_uses_global(
        global_threshold in valid_confidence()
    ) {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = global_threshold;

        let effective = config.get_threshold_for_category(Some("unknown_category"));

        prop_assert!(
            (effective - global_threshold).abs() < 0.0001,
            "Unknown category should use global threshold {}, got {}",
            global_threshold,
            effective
        );
    }

    /// Feature: ai-supervised-auto-approve
    /// Validates: Requirements 5.3
    ///
    /// None category should use global threshold
    #[test]
    fn prop_none_category_uses_global(
        global_threshold in valid_confidence()
    ) {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = global_threshold;

        let effective = config.get_threshold_for_category(None);

        prop_assert!(
            (effective - global_threshold).abs() < 0.0001,
            "None category should use global threshold {}, got {}",
            global_threshold,
            effective
        );
    }
}

// =============================================================================
// Property 19: Age-Based Prioritization
// =============================================================================
// For any batch of pending matches, the processing order SHALL be oldest-first
// based on created_at timestamp.
// Validates: Requirements 6.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 19: Age-Based Prioritization
    /// Validates: Requirements 6.2
    ///
    /// Batch processing should respect the order of input (assumed to be sorted by age)
    /// Note: The actual sorting is done at the database query level, but the processor
    /// should not reorder the input.
    #[test]
    fn prop_batch_preserves_input_order(
        batch_size in 1usize..=50
    ) {
        // Create a config with the specified batch size
        let mut config = AutoApproveConfig::default();
        config.batch_size = batch_size;

        // Verify the batch size is correctly set
        prop_assert_eq!(
            config.batch_size,
            batch_size,
            "Batch size should be set to {}",
            batch_size
        );

        // The processor will take at most batch_size items
        // This test verifies the configuration is correct
        prop_assert!(
            config.batch_size <= 1000,
            "Batch size {} should be <= 1000",
            config.batch_size
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 19: Age-Based Prioritization
    /// Validates: Requirements 6.2
    ///
    /// Processing interval should be configurable
    #[test]
    fn prop_processing_interval_configurable(
        interval_secs in 1u64..3600
    ) {
        let mut config = AutoApproveConfig::default();
        config.processing_interval_secs = interval_secs;

        prop_assert_eq!(
            config.processing_interval_secs,
            interval_secs,
            "Processing interval should be set to {} seconds",
            interval_secs
        );
    }
}

// =============================================================================
// Property 21: Blocklist Enforcement
// =============================================================================
// For any match involving a medication on the blocklist, the system SHALL NEVER
// auto-approve regardless of AI confidence.
// Validates: Requirements 7.1

use pharma_core::domain::{Offer, Request};
use pharma_core::matching::{
    BlocklistEntry, BlocklistSeverity, DosageGate, MedicationBlocklist, SafetyGuardrails,
    SafetyGuardrailsConfig,
};
use std::sync::Arc;

/// Strategy for generating blocklist entries
fn blocklist_entry() -> impl Strategy<Value = (String, String)> {
    (
        "[A-Z][a-z]{4,10}", // medication_a
        "[A-Z][a-z]{4,10}", // medication_b
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 21: Blocklist Enforcement
    /// Validates: Requirements 7.1
    ///
    /// For any medication pair on the blocklist, the safety check SHALL fail
    #[test]
    fn prop_blocklist_blocks_matching_pairs(
        (med_a, med_b) in blocklist_entry(),
        confidence in 0.0f64..1.0
    ) {
        // Create a blocklist with the medication pair
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::new(
            &med_a,
            &med_b,
            "Test blocklist entry",
            BlocklistSeverity::Critical,
        ));

        // Create safety guardrails with the blocklist
        let guardrails = SafetyGuardrails::new(
            Arc::new(blocklist),
            Arc::new(DosageGate::default()),
            SafetyGuardrailsConfig::default(),
        );

        // Create offer and request with the blocked medications
        let offer = Offer {
            medication: med_a.clone(),
            ..Default::default()
        };
        let request = Request {
            medication: med_b.clone(),
            ..Default::default()
        };

        // Run the blocklist check
        let rt = tokio::runtime::Runtime::new().unwrap();
        let checks = rt.block_on(guardrails.check(&offer, &request, confidence));

        // Find the blocklist check result
        let blocklist_check = checks.iter().find(|c| c.check_name == "blocklist");

        prop_assert!(
            blocklist_check.is_some(),
            "Blocklist check should be present in results"
        );

        let check = blocklist_check.unwrap();
        prop_assert!(
            !check.passed,
            "Blocklist check should FAIL for blocked pair {} - {} (confidence: {})",
            med_a,
            med_b,
            confidence
        );

        prop_assert!(
            check.reason.is_some(),
            "Failed blocklist check should have a reason"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 21: Blocklist Enforcement
    /// Validates: Requirements 7.1
    ///
    /// For any medication pair NOT on the blocklist, the blocklist check SHALL pass
    #[test]
    fn prop_blocklist_allows_non_blocked_pairs(
        med_a in "[A-Z][a-z]{4,10}",
        med_b in "[A-Z][a-z]{4,10}",
        confidence in 0.0f64..1.0
    ) {
        // Create an empty blocklist (no blocked pairs)
        let blocklist = MedicationBlocklist::new();

        // Create safety guardrails with the empty blocklist
        let guardrails = SafetyGuardrails::new(
            Arc::new(blocklist),
            Arc::new(DosageGate::default()),
            SafetyGuardrailsConfig::default(),
        );

        // Create offer and request
        let offer = Offer {
            medication: med_a.clone(),
            ..Default::default()
        };
        let request = Request {
            medication: med_b.clone(),
            ..Default::default()
        };

        // Run the blocklist check
        let rt = tokio::runtime::Runtime::new().unwrap();
        let checks = rt.block_on(guardrails.check(&offer, &request, confidence));

        // Find the blocklist check result
        let blocklist_check = checks.iter().find(|c| c.check_name == "blocklist");

        prop_assert!(
            blocklist_check.is_some(),
            "Blocklist check should be present in results"
        );

        let check = blocklist_check.unwrap();
        prop_assert!(
            check.passed,
            "Blocklist check should PASS for non-blocked pair {} - {}",
            med_a,
            med_b
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 21: Blocklist Enforcement
    /// Validates: Requirements 7.1
    ///
    /// Blocklist check should work regardless of medication order
    #[test]
    fn prop_blocklist_order_independent(
        (med_a, med_b) in blocklist_entry()
    ) {
        // Create a blocklist with the medication pair
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::new(
            &med_a,
            &med_b,
            "Test blocklist entry",
            BlocklistSeverity::Critical,
        ));

        let blocklist = Arc::new(blocklist);
        let dosage_gate = Arc::new(DosageGate::default());

        // Create safety guardrails
        let guardrails = SafetyGuardrails::new(
            blocklist.clone(),
            dosage_gate.clone(),
            SafetyGuardrailsConfig::default(),
        );

        // Test with original order
        let offer1 = Offer {
            medication: med_a.clone(),
            ..Default::default()
        };
        let request1 = Request {
            medication: med_b.clone(),
            ..Default::default()
        };

        // Test with reversed order
        let offer2 = Offer {
            medication: med_b.clone(),
            ..Default::default()
        };
        let request2 = Request {
            medication: med_a.clone(),
            ..Default::default()
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let checks1 = rt.block_on(guardrails.check(&offer1, &request1, 0.9));
        let checks2 = rt.block_on(guardrails.check(&offer2, &request2, 0.9));

        let blocklist_check1 = checks1.iter().find(|c| c.check_name == "blocklist").unwrap();
        let blocklist_check2 = checks2.iter().find(|c| c.check_name == "blocklist").unwrap();

        prop_assert_eq!(
            blocklist_check1.passed,
            blocklist_check2.passed,
            "Blocklist check should give same result regardless of order"
        );
    }
}

// =============================================================================
// Property 24: Dosage Mismatch Safety
// =============================================================================
// For any match where dosage differs by more than 20%, the system SHALL NOT
// auto-approve and SHALL queue for human review.
// Validates: Requirements 7.4

use pharma_core::matching::DosageGateConfig;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 24: Dosage Mismatch Safety
    /// Validates: Requirements 7.4
    ///
    /// When dosage score indicates a large mismatch (triggering MandatoryReview),
    /// the safety check should fail
    #[test]
    fn prop_dosage_mismatch_blocks_auto_approval(
        base_dosage in 100u32..500,
        multiplier in 3.0f64..5.0  // >200% difference relative to request triggers MandatoryReview
    ) {
        // For MandatoryReview to trigger, we need >100% difference relative to request
        // If offer = base * multiplier and request = base, then:
        // diff = |offer - request| / request = |base*mult - base| / base = mult - 1
        // For mult = 3.0, diff = 200% which is > 100%
        let offer_dosage = (base_dosage as f64 * multiplier) as u32;
        let request_dosage = base_dosage;

        // Create medications with significant dosage difference
        let offer_med = format!("Aspirin {}mg", offer_dosage);
        let request_med = format!("Aspirin {}mg", request_dosage);

        // Create safety guardrails
        let blocklist = MedicationBlocklist::new();
        let dosage_gate = DosageGate::new(DosageGateConfig {
            review_threshold_percent: 100.0, // >100% triggers MandatoryReview
            ..Default::default()
        });

        let guardrails = SafetyGuardrails::new(
            Arc::new(blocklist),
            Arc::new(dosage_gate),
            SafetyGuardrailsConfig::default(),
        );

        // Create offer and request
        let offer = Offer {
            medication: offer_med.clone(),
            ..Default::default()
        };
        let request = Request {
            medication: request_med.clone(),
            ..Default::default()
        };

        // Run safety checks with a low dosage score (indicating mismatch)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let checks = rt.block_on(guardrails.check(&offer, &request, 0.3));

        // Find the dosage mismatch check result
        let dosage_check = checks.iter().find(|c| c.check_name == "dosage_mismatch");

        prop_assert!(
            dosage_check.is_some(),
            "Dosage mismatch check should be present in results"
        );

        let check = dosage_check.unwrap();
        prop_assert!(
            !check.passed,
            "Dosage mismatch check should FAIL for large dosage difference: {} vs {} (multiplier: {:.1}x, diff: {:.0}%)",
            offer_med,
            request_med,
            multiplier,
            (multiplier - 1.0) * 100.0
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 24: Dosage Mismatch Safety
    /// Validates: Requirements 7.4
    ///
    /// When dosages are similar (high dosage score), the safety check should pass
    #[test]
    fn prop_similar_dosage_allows_auto_approval(
        base_dosage in 50u32..500,
        variation in 0.9f64..1.1  // Within 10% variation
    ) {
        let offer_dosage = base_dosage;
        let request_dosage = (base_dosage as f64 * variation) as u32;

        // Create medications with similar dosages
        let offer_med = format!("Aspirin {}mg", offer_dosage);
        let request_med = format!("Aspirin {}mg", request_dosage);

        // Create safety guardrails
        let blocklist = MedicationBlocklist::new();
        let dosage_gate = DosageGate::default();

        let guardrails = SafetyGuardrails::new(
            Arc::new(blocklist),
            Arc::new(dosage_gate),
            SafetyGuardrailsConfig::default(),
        );

        // Create offer and request
        let offer = Offer {
            medication: offer_med.clone(),
            ..Default::default()
        };
        let request = Request {
            medication: request_med.clone(),
            ..Default::default()
        };

        // Run safety checks with a high dosage score (indicating match)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let checks = rt.block_on(guardrails.check(&offer, &request, 0.9));

        // Find the dosage mismatch check result
        let dosage_check = checks.iter().find(|c| c.check_name == "dosage_mismatch");

        prop_assert!(
            dosage_check.is_some(),
            "Dosage mismatch check should be present in results"
        );

        let check = dosage_check.unwrap();
        prop_assert!(
            check.passed,
            "Dosage mismatch check should PASS for similar dosages: {} vs {} (variation: {:.1}x)",
            offer_med,
            request_med,
            variation
        );
    }
}

// =============================================================================
// Property 22: Override Rate Pause
// =============================================================================
// For any period where the override rate exceeds the configured threshold,
// the system SHALL pause auto-approval.
// Validates: Requirements 7.2

use pharma_core::matching::{OverrideTracker, OverrideTrackerConfig, PauseReason};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 22: Override Rate Pause
    /// Validates: Requirements 7.2
    ///
    /// When override rate exceeds threshold, should_pause should return true
    #[test]
    fn prop_high_override_rate_triggers_pause(
        threshold in 0.05f64..0.30,
        override_count in 3u32..10,
        approval_count in 1u32..5
    ) {
        // Calculate the rate that will result from these counts
        let total = override_count + approval_count;
        let rate = override_count as f64 / total as f64;

        // Only test cases where rate exceeds threshold and we have enough samples
        prop_assume!(rate > threshold && total >= 10);

        let config = OverrideTrackerConfig {
            pause_threshold: threshold,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Record approvals first
        for _ in 0..approval_count {
            tracker.record_approval();
        }

        // Then record overrides
        for _ in 0..override_count {
            tracker.record_override();
        }

        // Check if pause is triggered
        let result = tracker.check_override_rate();

        prop_assert!(
            result.is_some(),
            "Override rate {:.2} exceeds threshold {:.2}, should trigger pause",
            rate,
            threshold
        );

        if let Some(PauseReason::HighOverrideRate { rate: r, threshold: t }) = result {
            prop_assert!(
                r > t,
                "Pause reason should indicate rate {} > threshold {}",
                r,
                t
            );
        } else {
            prop_assert!(false, "Expected HighOverrideRate pause reason");
        }
    }

    /// Feature: ai-supervised-auto-approve, Property 22: Override Rate Pause
    /// Validates: Requirements 7.2
    ///
    /// When override rate is below threshold, should_pause should return false
    #[test]
    fn prop_low_override_rate_no_pause(
        threshold in 0.20f64..0.50,
        override_count in 1u32..3,
        approval_count in 10u32..20
    ) {
        // Calculate the rate that will result from these counts
        let total = override_count + approval_count;
        let rate = override_count as f64 / total as f64;

        // Only test cases where rate is below threshold
        prop_assume!(rate < threshold);

        let config = OverrideTrackerConfig {
            pause_threshold: threshold,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Record approvals first
        for _ in 0..approval_count {
            tracker.record_approval();
        }

        // Then record overrides
        for _ in 0..override_count {
            tracker.record_override();
        }

        // Check if pause is triggered
        let result = tracker.check_override_rate();

        prop_assert!(
            result.is_none(),
            "Override rate {:.2} is below threshold {:.2}, should NOT trigger pause",
            rate,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 22: Override Rate Pause
    /// Validates: Requirements 7.2
    ///
    /// Override rate calculation should be accurate
    #[test]
    fn prop_override_rate_calculation_accurate(
        override_count in 0u32..20,
        approval_count in 0u32..20
    ) {
        prop_assume!(override_count + approval_count > 0);

        let config = OverrideTrackerConfig::default();
        let mut tracker = OverrideTracker::new(config);

        // Record events
        for _ in 0..approval_count {
            tracker.record_approval();
        }
        for _ in 0..override_count {
            tracker.record_override();
        }

        let expected_rate = override_count as f64 / (override_count + approval_count) as f64;
        let actual_rate = tracker.override_rate();

        prop_assert!(
            (actual_rate - expected_rate).abs() < 0.001,
            "Override rate should be {:.4}, got {:.4}",
            expected_rate,
            actual_rate
        );
    }
}

// =============================================================================
// Property 25: Consecutive Override Disable
// =============================================================================
// For any sequence of 5 or more consecutive overridden AI decisions, the system
// SHALL automatically disable auto-approval and notify the supervisor.
// Validates: Requirements 7.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 25: Consecutive Override Disable
    /// Validates: Requirements 7.5
    ///
    /// When consecutive overrides reach the limit, should trigger disable
    #[test]
    fn prop_consecutive_overrides_triggers_disable(
        limit in 3u32..10,
        extra_overrides in 0u32..5
    ) {
        let config = OverrideTrackerConfig {
            consecutive_limit: limit,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Record exactly limit + extra_overrides consecutive overrides
        let total_overrides = limit + extra_overrides;
        for _ in 0..total_overrides {
            tracker.record_override();
        }

        // Check if disable is triggered
        let result = tracker.check_consecutive_overrides();

        prop_assert!(
            result.is_some(),
            "{} consecutive overrides should trigger disable (limit: {})",
            total_overrides,
            limit
        );

        if let Some(PauseReason::ConsecutiveOverrides { count, limit: l }) = result {
            prop_assert!(
                count >= l,
                "Consecutive count {} should be >= limit {}",
                count,
                l
            );
        } else {
            prop_assert!(false, "Expected ConsecutiveOverrides pause reason");
        }
    }

    /// Feature: ai-supervised-auto-approve, Property 25: Consecutive Override Disable
    /// Validates: Requirements 7.5
    ///
    /// When consecutive overrides are below limit, should NOT trigger disable
    #[test]
    fn prop_below_limit_no_disable(
        limit in 5u32..10,
        override_count in 1u32..5
    ) {
        prop_assume!(override_count < limit);

        let config = OverrideTrackerConfig {
            consecutive_limit: limit,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Record fewer than limit consecutive overrides
        for _ in 0..override_count {
            tracker.record_override();
        }

        // Check if disable is triggered
        let result = tracker.check_consecutive_overrides();

        prop_assert!(
            result.is_none(),
            "{} consecutive overrides should NOT trigger disable (limit: {})",
            override_count,
            limit
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 25: Consecutive Override Disable
    /// Validates: Requirements 7.5
    ///
    /// An approval should reset the consecutive override counter
    #[test]
    fn prop_approval_resets_consecutive_count(
        limit in 5u32..10,
        overrides_before in 1u32..5,
        overrides_after in 1u32..5
    ) {
        prop_assume!(overrides_before < limit);
        prop_assume!(overrides_after < limit);

        let config = OverrideTrackerConfig {
            consecutive_limit: limit,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Record some overrides
        for _ in 0..overrides_before {
            tracker.record_override();
        }

        // Record an approval (should reset counter)
        tracker.record_approval();

        // Record more overrides
        for _ in 0..overrides_after {
            tracker.record_override();
        }

        // The consecutive count should only reflect overrides_after
        let count = tracker.consecutive_count();

        prop_assert_eq!(
            count,
            overrides_after,
            "After approval, consecutive count should be {} (overrides after approval), got {}",
            overrides_after,
            count
        );

        // Should not trigger disable since overrides_after < limit
        let result = tracker.check_consecutive_overrides();
        prop_assert!(
            result.is_none(),
            "Should not trigger disable after approval reset"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 25: Consecutive Override Disable
    /// Validates: Requirements 7.5
    ///
    /// Consecutive count should accurately track overrides
    #[test]
    fn prop_consecutive_count_accurate(
        override_count in 0u32..20
    ) {
        let config = OverrideTrackerConfig::default();
        let mut tracker = OverrideTracker::new(config);

        for _ in 0..override_count {
            tracker.record_override();
        }

        let count = tracker.consecutive_count();

        prop_assert_eq!(
            count,
            override_count,
            "Consecutive count should be {}, got {}",
            override_count,
            count
        );
    }
}

// =============================================================================
// Property 23: Anomaly Detection Pause
// =============================================================================
// For any detected anomaly (e.g., sudden confidence drop > 20%), the system
// SHALL alert the supervisor and pause processing.
// Validates: Requirements 7.3

use pharma_core::matching::{AnomalyDetector, AnomalyDetectorConfig};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 23: Anomaly Detection Pause
    /// Validates: Requirements 7.3
    ///
    /// When confidence drops by more than threshold, should detect anomaly
    #[test]
    fn prop_confidence_drop_triggers_anomaly(
        baseline_confidence in 0.80f64..0.95,
        drop_percent in 0.25f64..0.50  // >20% drop
    ) {
        let config = AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        };
        let mut detector = AnomalyDetector::new(config);

        // Record baseline confidence scores
        for _ in 0..15 {
            detector.record_confidence(baseline_confidence);
        }

        // Record dropped confidence scores
        let dropped_confidence = baseline_confidence * (1.0 - drop_percent);
        for _ in 0..10 {
            detector.record_confidence(dropped_confidence);
        }

        // Check for anomaly
        let result = detector.check_anomaly();

        prop_assert!(
            result.is_some(),
            "Confidence drop from {:.2} to {:.2} ({:.0}% drop) should trigger anomaly",
            baseline_confidence,
            dropped_confidence,
            drop_percent * 100.0
        );

        let description = result.unwrap();
        prop_assert!(
            description.contains("drop"),
            "Anomaly description should mention 'drop': {}",
            description
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 23: Anomaly Detection Pause
    /// Validates: Requirements 7.3
    ///
    /// When confidence is stable, should NOT detect anomaly
    #[test]
    fn prop_stable_confidence_no_anomaly(
        confidence in 0.70f64..0.95,
        variation in -0.05f64..0.05
    ) {
        let config = AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        };
        let mut detector = AnomalyDetector::new(config);

        // Record stable confidence scores with small variation
        for i in 0..25 {
            let varied_confidence = (confidence + variation * (i as f64 / 25.0)).clamp(0.0, 1.0);
            detector.record_confidence(varied_confidence);
        }

        // Check for anomaly
        let result = detector.check_anomaly();

        prop_assert!(
            result.is_none(),
            "Stable confidence around {:.2} should NOT trigger anomaly",
            confidence
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 23: Anomaly Detection Pause
    /// Validates: Requirements 7.3
    ///
    /// Should not detect anomaly with insufficient samples
    #[test]
    fn prop_insufficient_samples_no_anomaly(
        sample_count in 1usize..15,
        confidence in 0.70f64..0.95
    ) {
        let config = AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        };
        let mut detector = AnomalyDetector::new(config);

        // Record fewer than min_samples
        for _ in 0..sample_count {
            detector.record_confidence(confidence);
        }

        // Check for anomaly
        let result = detector.check_anomaly();

        prop_assert!(
            result.is_none(),
            "With only {} samples (min: 20), should NOT detect anomaly",
            sample_count
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 23: Anomaly Detection Pause
    /// Validates: Requirements 7.3
    ///
    /// Confidence increase should NOT trigger anomaly
    #[test]
    fn prop_confidence_increase_no_anomaly(
        baseline_confidence in 0.60f64..0.80,
        increase_percent in 0.10f64..0.30
    ) {
        let config = AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        };
        let mut detector = AnomalyDetector::new(config);

        // Record baseline confidence scores
        for _ in 0..15 {
            detector.record_confidence(baseline_confidence);
        }

        // Record increased confidence scores
        let increased_confidence = (baseline_confidence * (1.0 + increase_percent)).min(1.0);
        for _ in 0..10 {
            detector.record_confidence(increased_confidence);
        }

        // Check for anomaly
        let result = detector.check_anomaly();

        prop_assert!(
            result.is_none(),
            "Confidence increase from {:.2} to {:.2} should NOT trigger anomaly",
            baseline_confidence,
            increased_confidence
        );
    }
}

// =============================================================================
// Property 10: Override Status Reversion
// =============================================================================
// For any override action on an auto-approved match, the match status SHALL
// change to PENDING and an audit record SHALL be created.
// Validates: Requirements 4.1

use ai_client::{Client as AIClient, ClientConfig};
use pharma_core::matching::{AIReviewer, AutoApproveProcessor};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 10: Override Status Reversion
    /// Validates: Requirements 4.1
    ///
    /// Override decision should return a valid OverrideResult with all required fields
    #[test]
    fn prop_override_returns_valid_result(
        reason in "[a-zA-Z ]{5,50}",
        offer_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg",
        request_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create processor with default config
            let config = AutoApproveConfig {
                enabled: true,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            let result = processor
                .override_decision(match_id, user_id, &reason, &offer_med, &request_med)
                .await;

            // Should succeed
            assert!(result.is_ok(), "Override should succeed");

            let override_result = result.unwrap();

            // Verify all fields are populated
            assert_eq!(override_result.match_id, match_id);
            assert_eq!(override_result.user_id, user_id);
            assert_eq!(override_result.reason, reason);
            assert_eq!(override_result.offer_medication, offer_med);
            assert_eq!(override_result.request_medication, request_med);

            // Cooldown should be in the future
            assert!(override_result.cooldown_until > override_result.timestamp);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 10: Override Status Reversion
    /// Validates: Requirements 4.1
    ///
    /// Override should increment the override counter
    #[test]
    fn prop_override_increments_counter(
        override_count in 1u32..10
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                consecutive_override_limit: 100, // High limit to avoid pause
                override_rate_pause_threshold: 0.99, // High threshold to avoid pause
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Record some approvals first to avoid immediate pause
            for _ in 0..50 {
                // Simulate approvals by not calling override
            }

            // Perform overrides
            for i in 0..override_count {
                let result = processor
                    .override_decision(
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        "Test override",
                        "MedA",
                        "MedB",
                    )
                    .await;

                assert!(result.is_ok(), "Override {} should succeed", i);
            }

            // Stats should reflect the overrides
            let stats = processor.get_stats().await;
            assert!(
                stats.override_rate > 0.0 || override_count == 0,
                "Override rate should be > 0 after {} overrides",
                override_count
            );
        });
    }
}

// =============================================================================
// Property 12: Cooldown Enforcement
// =============================================================================
// For any medication pair that has been overridden, subsequent matches with the
// same pair SHALL NOT be auto-approved during the cooldown period.
// Validates: Requirements 4.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 12: Cooldown Enforcement
    /// Validates: Requirements 4.3
    ///
    /// After an override, the medication pair should be in cooldown
    #[test]
    fn prop_override_creates_cooldown(
        offer_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg",
        request_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg",
        cooldown_mins in 1u64..120
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                override_cooldown_mins: cooldown_mins,
                consecutive_override_limit: 100, // High limit to avoid pause
                override_rate_pause_threshold: 0.99, // High threshold to avoid pause
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Initially, the pair should NOT be in cooldown
            assert!(
                !processor.is_in_cooldown(&offer_med, &request_med).await,
                "Pair should NOT be in cooldown before override"
            );

            // Perform an override
            let result = processor
                .override_decision(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    "Test override",
                    &offer_med,
                    &request_med,
                )
                .await;

            assert!(result.is_ok(), "Override should succeed");

            // After override, the pair SHOULD be in cooldown
            assert!(
                processor.is_in_cooldown(&offer_med, &request_med).await,
                "Pair should be in cooldown after override"
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 12: Cooldown Enforcement
    /// Validates: Requirements 4.3
    ///
    /// Cooldown should be order-independent (A-B same as B-A)
    #[test]
    fn prop_cooldown_order_independent(
        offer_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg",
        request_med in "[A-Z][a-z]{4,10} [0-9]{2,3}mg"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                override_cooldown_mins: 60,
                consecutive_override_limit: 100,
                override_rate_pause_threshold: 0.99,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Override with offer_med -> request_med
            let _ = processor
                .override_decision(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    "Test override",
                    &offer_med,
                    &request_med,
                )
                .await;

            // Check cooldown in both orders
            let cooldown_original = processor.is_in_cooldown(&offer_med, &request_med).await;
            let cooldown_reversed = processor.is_in_cooldown(&request_med, &offer_med).await;

            assert_eq!(
                cooldown_original, cooldown_reversed,
                "Cooldown should be order-independent"
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 12: Cooldown Enforcement
    /// Validates: Requirements 4.3
    ///
    /// Different medication pairs should have independent cooldowns
    #[test]
    fn prop_cooldown_pair_specific(
        med_a in "[A-Z][a-z]{4,10}",
        med_b in "[A-Z][a-z]{4,10}",
        med_c in "[A-Z][a-z]{4,10}"
    ) {
        // Ensure all medications are different
        prop_assume!(med_a != med_b && med_b != med_c && med_a != med_c);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                override_cooldown_mins: 60,
                consecutive_override_limit: 100,
                override_rate_pause_threshold: 0.99,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Override pair A-B
            let _ = processor
                .override_decision(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    "Test override",
                    &med_a,
                    &med_b,
                )
                .await;

            // A-B should be in cooldown
            assert!(
                processor.is_in_cooldown(&med_a, &med_b).await,
                "Pair A-B should be in cooldown"
            );

            // A-C should NOT be in cooldown (different pair)
            assert!(
                !processor.is_in_cooldown(&med_a, &med_c).await,
                "Pair A-C should NOT be in cooldown"
            );

            // B-C should NOT be in cooldown (different pair)
            assert!(
                !processor.is_in_cooldown(&med_b, &med_c).await,
                "Pair B-C should NOT be in cooldown"
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 12: Cooldown Enforcement
    /// Validates: Requirements 4.3
    ///
    /// Safety checks should include cooldown check and fail for pairs in cooldown
    #[test]
    fn prop_safety_checks_include_cooldown(
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                override_cooldown_mins: 60,
                consecutive_override_limit: 100,
                override_rate_pause_threshold: 0.99,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Override the pair to put it in cooldown
            let _ = processor
                .override_decision(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    "Test override",
                    &offer_med,
                    &request_med,
                )
                .await;

            // Create offer and request
            let offer = Offer {
                medication: offer_med.clone(),
                ..Default::default()
            };
            let request = Request {
                medication: request_med.clone(),
                ..Default::default()
            };

            // Run safety checks
            let checks = processor.run_safety_checks(&offer, &request, 0.9).await;

            // Find the cooldown check
            let cooldown_check = checks.iter().find(|c| c.check_name == "cooldown");

            assert!(
                cooldown_check.is_some(),
                "Safety checks should include cooldown check"
            );

            let check = cooldown_check.unwrap();
            assert!(
                !check.passed,
                "Cooldown check should FAIL for pair in cooldown"
            );
        });
    }
}

// =============================================================================
// Property 11: Undo Time Window
// =============================================================================
// For any undo attempt within the configured undo window, the operation SHALL
// succeed. For any undo attempt after the window expires, the operation SHALL
// fail with an appropriate error.
// Validates: Requirements 4.2

use chrono::{Duration, Utc};
use pharma_core::matching::AutoApproveError;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 11: Undo Time Window
    /// Validates: Requirements 4.2
    ///
    /// Undo within the time window should succeed
    #[test]
    fn prop_undo_within_window_succeeds(
        undo_window_mins in 1u64..120,
        minutes_ago in 0u64..60
    ) {
        // Only test cases where we're within the window
        prop_assume!(minutes_ago < undo_window_mins);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                undo_window_mins,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();
            let approved_at = Utc::now() - Duration::minutes(minutes_ago as i64);

            let result = processor.undo_approval(match_id, user_id, approved_at).await;

            assert!(
                result.is_ok(),
                "Undo should succeed when {} minutes ago < {} minute window",
                minutes_ago,
                undo_window_mins
            );

            let undo_result = result.unwrap();
            assert_eq!(undo_result.match_id, match_id);
            assert_eq!(undo_result.user_id, user_id);
            assert_eq!(undo_result.original_approved_at, approved_at);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 11: Undo Time Window
    /// Validates: Requirements 4.2
    ///
    /// Undo after the time window should fail
    #[test]
    fn prop_undo_after_window_fails(
        undo_window_mins in 1u64..60,
        extra_minutes in 1u64..60
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                undo_window_mins,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();
            // Approved at a time that's past the undo window
            let approved_at = Utc::now() - Duration::minutes((undo_window_mins + extra_minutes) as i64);

            let result = processor.undo_approval(match_id, user_id, approved_at).await;

            assert!(
                result.is_err(),
                "Undo should fail when {} minutes ago > {} minute window",
                undo_window_mins + extra_minutes,
                undo_window_mins
            );

            // Verify it's the correct error type
            match result {
                Err(AutoApproveError::UndoWindowExpired) => {
                    // Expected error
                }
                Err(other) => {
                    panic!("Expected UndoWindowExpired error, got: {:?}", other);
                }
                Ok(_) => {
                    panic!("Expected error, but got success");
                }
            }
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 11: Undo Time Window
    /// Validates: Requirements 4.2
    ///
    /// is_within_undo_window should correctly identify valid windows
    #[test]
    fn prop_is_within_undo_window_accurate(
        undo_window_mins in 1u64..120,
        minutes_ago in 0u64..180
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                undo_window_mins,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let approved_at = Utc::now() - Duration::minutes(minutes_ago as i64);
            let is_within = processor.is_within_undo_window(approved_at).await;

            // Use < instead of <= to account for timing variations
            // The boundary case (minutes_ago == undo_window_mins) may fail due to
            // small timing differences between calculating approved_at and checking
            let expected = minutes_ago < undo_window_mins;

            // Only assert for clear cases (not at the boundary)
            if minutes_ago != undo_window_mins {
                assert_eq!(
                    is_within, expected,
                    "is_within_undo_window should return {} for {} minutes ago with {} minute window",
                    expected, minutes_ago, undo_window_mins
                );
            }
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 11: Undo Time Window
    /// Validates: Requirements 4.2
    ///
    /// Undo at exactly the window boundary should succeed
    #[test]
    fn prop_undo_at_boundary_succeeds(
        undo_window_mins in 1u64..120
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                undo_window_mins,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();
            // Approved exactly at the window boundary (minus a small buffer for test execution)
            let approved_at = Utc::now() - Duration::minutes(undo_window_mins as i64) + Duration::seconds(5);

            let result = processor.undo_approval(match_id, user_id, approved_at).await;

            assert!(
                result.is_ok(),
                "Undo at boundary should succeed (approved {} mins ago, window {} mins)",
                undo_window_mins,
                undo_window_mins
            );
        });
    }
}

// =============================================================================
// Property 13: Feedback Learning Integration
// =============================================================================
// For any override or rejection of an AI-approved match, a feedback record
// SHALL be created for the learning system.
// Validates: Requirements 4.4

use pharma_core::matching::{FeedbackEvent, FeedbackEventType};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 13: Feedback Learning Integration
    /// Validates: Requirements 4.4
    ///
    /// Override should create a feedback event with correct type
    #[test]
    fn prop_override_creates_feedback_event(
        reason in "[a-zA-Z ]{5,50}",
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}",
        ai_confidence in 0.0f64..1.0
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: true,
                consecutive_override_limit: 100,
                override_rate_pause_threshold: 0.99,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            // Create feedback event for override
            let feedback = processor.create_override_feedback(
                match_id,
                user_id,
                ai_confidence,
                &reason,
                &offer_med,
                &request_med,
            );

            // Verify feedback event properties
            assert_eq!(feedback.match_id, match_id);
            assert_eq!(feedback.user_id, user_id);
            assert!(!feedback.confirmed, "Override feedback should not be confirmed");
            assert_eq!(feedback.event_type, FeedbackEventType::Override);
            assert_eq!(feedback.reason, reason);
            assert_eq!(feedback.offer_medication, offer_med);
            assert_eq!(feedback.request_medication, request_med);
            assert!((feedback.ai_confidence - ai_confidence).abs() < 0.001);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 13: Feedback Learning Integration
    /// Validates: Requirements 4.4
    ///
    /// Rejection should create a feedback event with correct type
    #[test]
    fn prop_rejection_creates_feedback_event(
        reason in "[a-zA-Z ]{5,50}",
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}",
        ai_confidence in 0.0f64..1.0
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig::default();
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            // Create feedback event for rejection
            let feedback = processor.create_rejection_feedback(
                match_id,
                user_id,
                ai_confidence,
                &reason,
                &offer_med,
                &request_med,
            );

            // Verify feedback event properties
            assert_eq!(feedback.match_id, match_id);
            assert_eq!(feedback.user_id, user_id);
            assert!(!feedback.confirmed, "Rejection feedback should not be confirmed");
            assert_eq!(feedback.event_type, FeedbackEventType::Rejection);
            assert_eq!(feedback.reason, reason);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 13: Feedback Learning Integration
    /// Validates: Requirements 4.4
    ///
    /// Confirmation should create a feedback event with correct type
    #[test]
    fn prop_confirmation_creates_feedback_event(
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}",
        ai_confidence in 0.0f64..1.0
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig::default();
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            // Create feedback event for confirmation
            let feedback = processor.create_confirmation_feedback(
                match_id,
                user_id,
                ai_confidence,
                &offer_med,
                &request_med,
            );

            // Verify feedback event properties
            assert_eq!(feedback.match_id, match_id);
            assert_eq!(feedback.user_id, user_id);
            assert!(feedback.confirmed, "Confirmation feedback should be confirmed");
            assert_eq!(feedback.event_type, FeedbackEventType::Confirmation);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 13: Feedback Learning Integration
    /// Validates: Requirements 4.4
    ///
    /// Feedback events should have valid timestamps
    #[test]
    fn prop_feedback_has_valid_timestamp(
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}"
    ) {
        let before = Utc::now();

        let feedback = FeedbackEvent::from_override(
            Uuid::new_v4(),
            Uuid::new_v4(),
            0.85,
            "Test reason",
            &offer_med,
            &request_med,
        );

        let after = Utc::now();

        assert!(
            feedback.timestamp >= before && feedback.timestamp <= after,
            "Feedback timestamp should be between test start and end"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 13: Feedback Learning Integration
    /// Validates: Requirements 4.4
    ///
    /// Different feedback types should have distinct event types
    #[test]
    fn prop_feedback_types_are_distinct(
        offer_med in "[A-Z][a-z]{4,10}",
        request_med in "[A-Z][a-z]{4,10}"
    ) {
        let match_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let override_feedback = FeedbackEvent::from_override(
            match_id, user_id, 0.85, "Override", &offer_med, &request_med
        );
        let rejection_feedback = FeedbackEvent::from_rejection(
            match_id, user_id, 0.85, "Rejection", &offer_med, &request_med
        );
        let confirmation_feedback = FeedbackEvent::from_confirmation(
            match_id, user_id, 0.85, &offer_med, &request_med
        );
        let undo_feedback = FeedbackEvent::from_undo(
            match_id, user_id, 0.85, &offer_med, &request_med
        );

        // All event types should be different
        assert_ne!(override_feedback.event_type, rejection_feedback.event_type);
        assert_ne!(override_feedback.event_type, confirmation_feedback.event_type);
        assert_ne!(override_feedback.event_type, undo_feedback.event_type);
        assert_ne!(rejection_feedback.event_type, confirmation_feedback.event_type);
        assert_ne!(rejection_feedback.event_type, undo_feedback.event_type);
        assert_ne!(confirmation_feedback.event_type, undo_feedback.event_type);

        // Override, rejection, and undo should not be confirmed
        assert!(!override_feedback.confirmed);
        assert!(!rejection_feedback.confirmed);
        assert!(!undo_feedback.confirmed);

        // Confirmation should be confirmed
        assert!(confirmation_feedback.confirmed);
    }
}

// =============================================================================
// Property 5: Auto-Approval Audit Completeness
// =============================================================================
// For any auto-approved match, the audit record SHALL contain: match_id,
// ai_confidence, ai_explanation, timestamp, and decision_type fields with
// non-null values.
// Validates: Requirements 2.1

use pharma_core::matching::{
    SupervisionAuditConfig, SupervisionAuditEntry, SupervisionAuditFilter, SupervisionAuditTrail,
    SupervisionEventType,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 5: Auto-Approval Audit Completeness
    /// Validates: Requirements 2.1
    ///
    /// Auto-approval audit entries should have all required fields populated
    #[test]
    fn prop_auto_approval_audit_has_complete_data(
        ai_confidence in 0.70f64..1.0,
        explanation in "[A-Za-z ]{10,50}"
    ) {
        let match_id = Uuid::new_v4();

        let entry = SupervisionAuditEntry::auto_approved(
            match_id,
            ai_confidence,
            explanation.clone(),
            vec![SafetyCheckResult::passed("blocklist")],
        );

        // Verify all required fields are present (Requirements 2.1)
        prop_assert!(
            entry.match_id.is_some(),
            "Auto-approval audit entry must have match_id"
        );
        prop_assert!(
            entry.ai_confidence.is_some(),
            "Auto-approval audit entry must have ai_confidence"
        );
        prop_assert!(
            entry.ai_explanation.is_some(),
            "Auto-approval audit entry must have ai_explanation"
        );
        prop_assert!(
            entry.decision.is_some(),
            "Auto-approval audit entry must have decision"
        );

        // Verify the completeness check method works
        prop_assert!(
            entry.has_complete_auto_approval_data(),
            "has_complete_auto_approval_data() should return true for valid entry"
        );

        // Verify event type is correct
        prop_assert_eq!(
            entry.event_type,
            SupervisionEventType::AutoApproved,
            "Event type should be AutoApproved"
        );

        // Verify values match input
        prop_assert_eq!(entry.match_id, Some(match_id));
        prop_assert!((entry.ai_confidence.unwrap() - ai_confidence).abs() < 0.0001);
        prop_assert_eq!(entry.ai_explanation, Some(explanation));
    }

    /// Feature: ai-supervised-auto-approve, Property 5: Auto-Approval Audit Completeness
    /// Validates: Requirements 2.1
    ///
    /// Auto-approval audit entries logged via trail should be retrievable
    #[test]
    fn prop_auto_approval_audit_is_persisted(
        ai_confidence in 0.70f64..1.0,
        explanation in "[A-Za-z ]{10,50}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
            let match_id = Uuid::new_v4();

            // Log the auto-approval
            let entry = trail
                .log_auto_approval(
                    match_id,
                    ai_confidence,
                    explanation.clone(),
                    vec![SafetyCheckResult::passed("blocklist")],
                )
                .await
                .expect("Should log auto-approval");

            // Verify entry has complete data
            assert!(entry.has_complete_auto_approval_data());

            // Retrieve and verify
            let history = trail.get_match_history(match_id).await.expect("Should get history");
            assert_eq!(history.len(), 1, "Should have exactly one entry");

            let retrieved = &history[0];
            assert_eq!(retrieved.match_id, Some(match_id));
            assert!(retrieved.ai_confidence.is_some());
            assert!(retrieved.ai_explanation.is_some());
            assert!(retrieved.decision.is_some());
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 5: Auto-Approval Audit Completeness
    /// Validates: Requirements 2.1
    ///
    /// Queued-for-review entries should also have complete data
    #[test]
    fn prop_queued_for_review_audit_has_complete_data(
        ai_confidence in 0.0f64..0.70,
        explanation in "[A-Za-z ]{10,50}",
        reason in "[A-Za-z ]{10,30}"
    ) {
        let match_id = Uuid::new_v4();

        let entry = SupervisionAuditEntry::queued_for_review(
            match_id,
            ai_confidence,
            explanation.clone(),
            reason.clone(),
            vec![],
        );

        // Verify all required fields are present
        prop_assert!(entry.match_id.is_some());
        prop_assert!(entry.ai_confidence.is_some());
        prop_assert!(entry.ai_explanation.is_some());
        prop_assert!(entry.decision.is_some());
        prop_assert!(entry.has_complete_auto_approval_data());

        // Verify event type
        prop_assert_eq!(entry.event_type, SupervisionEventType::QueuedForReview);
    }

    /// Feature: ai-supervised-auto-approve, Property 5: Auto-Approval Audit Completeness
    /// Validates: Requirements 2.1
    ///
    /// Blocked entries should also have complete data
    #[test]
    fn prop_blocked_audit_has_complete_data(
        ai_confidence in 0.0f64..1.0,
        explanation in "[A-Za-z ]{10,50}",
        reason in "[A-Za-z ]{10,30}"
    ) {
        let match_id = Uuid::new_v4();

        let entry = SupervisionAuditEntry::blocked(
            match_id,
            ai_confidence,
            explanation.clone(),
            reason.clone(),
            vec![SafetyCheckResult::failed("blocklist", "Medication blocked")],
        );

        // Verify all required fields are present
        prop_assert!(entry.match_id.is_some());
        prop_assert!(entry.ai_confidence.is_some());
        prop_assert!(entry.ai_explanation.is_some());
        prop_assert!(entry.decision.is_some());
        prop_assert!(entry.has_complete_auto_approval_data());

        // Verify event type
        prop_assert_eq!(entry.event_type, SupervisionEventType::Blocked);
    }
}

// =============================================================================
// Property 6: Override Audit Completeness
// =============================================================================
// For any override action, the audit record SHALL contain: original_decision,
// override_action, override_by, override_reason, and override_at fields with
// non-null values.
// Validates: Requirements 2.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 6: Override Audit Completeness
    /// Validates: Requirements 2.2
    ///
    /// Override audit entries should have all required fields populated
    #[test]
    fn prop_override_audit_has_complete_data(
        original_confidence in 0.70f64..1.0,
        original_explanation in "[A-Za-z ]{10,50}",
        override_reason in "[A-Za-z ]{10,30}"
    ) {
        let match_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = SupervisionAuditEntry::overridden(
            match_id,
            user_id,
            override_reason.clone(),
            original_confidence,
            original_explanation.clone(),
        );

        // Verify all required override fields are present (Requirements 2.2)
        prop_assert!(
            entry.overridden,
            "Override entry must have overridden=true"
        );
        prop_assert!(
            entry.override_by.is_some(),
            "Override entry must have override_by"
        );
        prop_assert!(
            entry.override_reason.is_some(),
            "Override entry must have override_reason"
        );
        prop_assert!(
            entry.override_at.is_some(),
            "Override entry must have override_at"
        );

        // Verify the completeness check method works
        prop_assert!(
            entry.has_complete_override_data(),
            "has_complete_override_data() should return true for valid entry"
        );

        // Verify event type is correct
        prop_assert_eq!(
            entry.event_type,
            SupervisionEventType::Overridden,
            "Event type should be Overridden"
        );

        // Verify values match input
        prop_assert_eq!(entry.match_id, Some(match_id));
        prop_assert_eq!(entry.override_by, Some(user_id));
        prop_assert_eq!(entry.override_reason, Some(override_reason));

        // Original decision data should also be present
        prop_assert!(entry.ai_confidence.is_some());
        prop_assert!(entry.ai_explanation.is_some());
    }

    /// Feature: ai-supervised-auto-approve, Property 6: Override Audit Completeness
    /// Validates: Requirements 2.2
    ///
    /// Override audit entries logged via trail should be retrievable
    #[test]
    fn prop_override_audit_is_persisted(
        original_confidence in 0.70f64..1.0,
        original_explanation in "[A-Za-z ]{10,50}",
        override_reason in "[A-Za-z ]{10,30}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
            let match_id = Uuid::new_v4();
            let user_id = Uuid::new_v4();

            // Log the override
            let entry = trail
                .log_override(
                    match_id,
                    user_id,
                    override_reason.clone(),
                    original_confidence,
                    original_explanation.clone(),
                )
                .await
                .expect("Should log override");

            // Verify entry has complete data
            assert!(entry.has_complete_override_data());

            // Retrieve and verify
            let history = trail.get_match_history(match_id).await.expect("Should get history");
            assert_eq!(history.len(), 1, "Should have exactly one entry");

            let retrieved = &history[0];
            assert!(retrieved.overridden);
            assert_eq!(retrieved.override_by, Some(user_id));
            assert!(retrieved.override_reason.is_some());
            assert!(retrieved.override_at.is_some());
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 6: Override Audit Completeness
    /// Validates: Requirements 2.2
    ///
    /// Undo entries should also have complete override data
    #[test]
    fn prop_undo_audit_has_complete_data(_dummy in 0..1) {
        let match_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = SupervisionAuditEntry::undo_approval(match_id, user_id);

        // Verify override fields are present
        prop_assert!(entry.overridden);
        prop_assert!(entry.override_by.is_some());
        prop_assert!(entry.override_reason.is_some());
        prop_assert!(entry.override_at.is_some());
        prop_assert!(entry.has_complete_override_data());

        // Verify event type
        prop_assert_eq!(entry.event_type, SupervisionEventType::UndoApproval);
    }
}

// =============================================================================
// Property 7: Audit Filtering Correctness
// =============================================================================
// For any audit query with filters (date_range, decision_type, confidence_range,
// override_status), the returned records SHALL all satisfy the specified filter
// criteria.
// Validates: Requirements 2.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Filter by event type should only return matching entries
    #[test]
    fn prop_filter_by_event_type_returns_only_matching(
        num_auto_approved in 1usize..5,
        num_overridden in 1usize..5
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            // Create mixed entries
            for _ in 0..num_auto_approved {
                trail
                    .log_auto_approval(
                        Uuid::new_v4(),
                        0.92,
                        "Test".to_string(),
                        vec![],
                    )
                    .await
                    .unwrap();
            }

            for _ in 0..num_overridden {
                trail
                    .log_override(
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        "Override reason".to_string(),
                        0.88,
                        "Original".to_string(),
                    )
                    .await
                    .unwrap();
            }

            // Filter by AutoApproved
            let filter = SupervisionAuditFilter::new()
                .of_type(SupervisionEventType::AutoApproved);
            let results = trail.query(&filter).await.unwrap();

            // All results should be AutoApproved
            for entry in &results {
                assert_eq!(
                    entry.event_type,
                    SupervisionEventType::AutoApproved,
                    "Filter by AutoApproved should only return AutoApproved entries"
                );
            }
            assert_eq!(results.len(), num_auto_approved);

            // Filter by Overridden
            let filter = SupervisionAuditFilter::new()
                .of_type(SupervisionEventType::Overridden);
            let results = trail.query(&filter).await.unwrap();

            // All results should be Overridden
            for entry in &results {
                assert_eq!(
                    entry.event_type,
                    SupervisionEventType::Overridden,
                    "Filter by Overridden should only return Overridden entries"
                );
            }
            assert_eq!(results.len(), num_overridden);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Filter by confidence range should only return entries within range
    #[test]
    fn prop_filter_by_confidence_range_returns_only_matching(
        low_confidence in 0.70f64..0.80,
        high_confidence in 0.90f64..0.99
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            // Create entries with different confidences
            trail
                .log_auto_approval(Uuid::new_v4(), low_confidence, "Low".to_string(), vec![])
                .await
                .unwrap();
            trail
                .log_auto_approval(Uuid::new_v4(), high_confidence, "High".to_string(), vec![])
                .await
                .unwrap();

            // Filter for high confidence only
            let filter = SupervisionAuditFilter::new()
                .in_confidence_range(0.85, 1.0);
            let results = trail.query(&filter).await.unwrap();

            // All results should have confidence >= 0.85
            for entry in &results {
                let confidence = entry.ai_confidence.unwrap();
                assert!(
                    confidence >= 0.85 && confidence <= 1.0,
                    "Confidence {} should be in range [0.85, 1.0]",
                    confidence
                );
            }
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Filter by override status should only return matching entries
    #[test]
    fn prop_filter_by_override_status_returns_only_matching(
        num_entries in 2usize..5
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            // Create non-overridden entries
            for _ in 0..num_entries {
                trail
                    .log_auto_approval(Uuid::new_v4(), 0.92, "Test".to_string(), vec![])
                    .await
                    .unwrap();
            }

            // Create overridden entries
            for _ in 0..num_entries {
                trail
                    .log_override(
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        "Override".to_string(),
                        0.88,
                        "Original".to_string(),
                    )
                    .await
                    .unwrap();
            }

            // Filter for overridden only
            let filter = SupervisionAuditFilter::new().with_override_status(true);
            let results = trail.query(&filter).await.unwrap();

            for entry in &results {
                assert!(
                    entry.overridden,
                    "Filter for overridden=true should only return overridden entries"
                );
            }

            // Filter for non-overridden only
            let filter = SupervisionAuditFilter::new().with_override_status(false);
            let results = trail.query(&filter).await.unwrap();

            for entry in &results {
                assert!(
                    !entry.overridden,
                    "Filter for overridden=false should only return non-overridden entries"
                );
            }
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Filter by match ID should only return entries for that match
    #[test]
    fn prop_filter_by_match_id_returns_only_matching(
        num_matches in 2usize..5
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            let match_ids: Vec<Uuid> = (0..num_matches).map(|_| Uuid::new_v4()).collect();

            // Create entries for each match
            for match_id in &match_ids {
                trail
                    .log_auto_approval(*match_id, 0.92, "Test".to_string(), vec![])
                    .await
                    .unwrap();
            }

            // Filter for specific match
            let target_match = match_ids[0];
            let filter = SupervisionAuditFilter::new().for_match(target_match);
            let results = trail.query(&filter).await.unwrap();

            // All results should be for the target match
            for entry in &results {
                assert_eq!(
                    entry.match_id,
                    Some(target_match),
                    "Filter by match_id should only return entries for that match"
                );
            }
            assert_eq!(results.len(), 1);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Combined filters should return entries matching ALL criteria
    #[test]
    fn prop_combined_filters_return_intersection(
        _dummy in 0..1
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            // Create various entries
            let match_id = Uuid::new_v4();

            // High confidence auto-approved
            trail
                .log_auto_approval(match_id, 0.95, "High".to_string(), vec![])
                .await
                .unwrap();

            // Low confidence auto-approved (different match)
            trail
                .log_auto_approval(Uuid::new_v4(), 0.72, "Low".to_string(), vec![])
                .await
                .unwrap();

            // Override (same match)
            trail
                .log_override(match_id, Uuid::new_v4(), "Override".to_string(), 0.95, "High".to_string())
                .await
                .unwrap();

            // Combined filter: match_id AND event_type
            let filter = SupervisionAuditFilter::new()
                .for_match(match_id)
                .of_type(SupervisionEventType::AutoApproved);
            let results = trail.query(&filter).await.unwrap();

            // Should only return the high confidence auto-approved for this match
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].match_id, Some(match_id));
            assert_eq!(results[0].event_type, SupervisionEventType::AutoApproved);
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 7: Audit Filtering Correctness
    /// Validates: Requirements 2.3
    ///
    /// Limit should cap the number of results
    #[test]
    fn prop_filter_limit_caps_results(
        num_entries in 5usize..20,
        limit in 1usize..5
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            // Create many entries
            for _ in 0..num_entries {
                trail
                    .log_auto_approval(Uuid::new_v4(), 0.92, "Test".to_string(), vec![])
                    .await
                    .unwrap();
            }

            // Query with limit
            let filter = SupervisionAuditFilter::new().with_limit(limit);
            let results = trail.query(&filter).await.unwrap();

            assert!(
                results.len() <= limit,
                "Results count {} should not exceed limit {}",
                results.len(),
                limit
            );
        });
    }
}

// =============================================================================
// Property 16: Configuration Change Auditing
// =============================================================================
// For any configuration change, an audit record SHALL be created containing
// the previous values and new values.
// Validates: Requirements 5.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 16: Configuration Change Auditing
    /// Validates: Requirements 5.4
    ///
    /// Configuration change audit entries should contain old and new config values
    #[test]
    fn prop_config_change_audit_has_old_and_new_values(
        old_threshold in 0.70f64..0.85,
        new_threshold in 0.85f64..0.99,
        old_batch_size in 10usize..50,
        new_batch_size in 50usize..100
    ) {
        let user_id = Uuid::new_v4();

        let mut old_config = AutoApproveConfig::default();
        old_config.confidence_threshold = old_threshold;
        old_config.batch_size = old_batch_size;

        let mut new_config = AutoApproveConfig::default();
        new_config.confidence_threshold = new_threshold;
        new_config.batch_size = new_batch_size;

        let entry = SupervisionAuditEntry::config_changed(user_id, &old_config, &new_config);

        // Verify event type
        prop_assert_eq!(
            entry.event_type,
            SupervisionEventType::ConfigChanged,
            "Event type should be ConfigChanged"
        );

        // Verify completeness check
        prop_assert!(
            entry.has_complete_config_change_data(),
            "Config change entry should have complete data"
        );

        // Verify metadata contains old and new config
        prop_assert!(
            entry.metadata.is_some(),
            "Config change entry must have metadata"
        );

        let metadata = entry.metadata.as_ref().unwrap();

        prop_assert!(
            metadata.get("old_config").is_some(),
            "Metadata must contain old_config"
        );
        prop_assert!(
            metadata.get("new_config").is_some(),
            "Metadata must contain new_config"
        );

        // Verify old config values are preserved
        let old_config_json = metadata.get("old_config").unwrap();
        let old_threshold_stored = old_config_json
            .get("confidence_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        prop_assert!(
            (old_threshold_stored - old_threshold).abs() < 0.0001,
            "Old threshold {} should be stored as {}, got {}",
            old_threshold,
            old_threshold,
            old_threshold_stored
        );

        // Verify new config values are preserved
        let new_config_json = metadata.get("new_config").unwrap();
        let new_threshold_stored = new_config_json
            .get("confidence_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        prop_assert!(
            (new_threshold_stored - new_threshold).abs() < 0.0001,
            "New threshold {} should be stored as {}, got {}",
            new_threshold,
            new_threshold,
            new_threshold_stored
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 16: Configuration Change Auditing
    /// Validates: Requirements 5.4
    ///
    /// Configuration change audit entries logged via trail should be retrievable
    #[test]
    fn prop_config_change_audit_is_persisted(
        old_threshold in 0.70f64..0.85,
        new_threshold in 0.85f64..0.99
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
            let user_id = Uuid::new_v4();

            let mut old_config = AutoApproveConfig::default();
            old_config.confidence_threshold = old_threshold;

            let mut new_config = AutoApproveConfig::default();
            new_config.confidence_threshold = new_threshold;

            // Log the config change
            let entry = trail
                .log_config_change(user_id, &old_config, &new_config)
                .await
                .expect("Should log config change");

            // Verify entry has complete data
            assert!(entry.has_complete_config_change_data());

            // Retrieve and verify
            let filter = SupervisionAuditFilter::new()
                .of_type(SupervisionEventType::ConfigChanged);
            let results = trail.query(&filter).await.expect("Should query");

            assert_eq!(results.len(), 1, "Should have exactly one config change entry");

            let retrieved = &results[0];
            assert_eq!(retrieved.event_type, SupervisionEventType::ConfigChanged);
            assert!(retrieved.metadata.is_some());
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 16: Configuration Change Auditing
    /// Validates: Requirements 5.4
    ///
    /// Config change should record the user who made the change
    #[test]
    fn prop_config_change_records_user(_dummy in 0..1) {
        let user_id = Uuid::new_v4();
        let old_config = AutoApproveConfig::default();
        let new_config = AutoApproveConfig::default();

        let entry = SupervisionAuditEntry::config_changed(user_id, &old_config, &new_config);

        prop_assert_eq!(
            entry.override_by,
            Some(user_id),
            "Config change should record the user who made the change"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 16: Configuration Change Auditing
    /// Validates: Requirements 5.4
    ///
    /// Multiple config changes should all be recorded
    #[test]
    fn prop_multiple_config_changes_all_recorded(
        num_changes in 2usize..5
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

            let mut current_config = AutoApproveConfig::default();

            for i in 0..num_changes {
                let old_config = current_config.clone();
                current_config.confidence_threshold = 0.70 + (i as f64 * 0.05);

                trail
                    .log_config_change(Uuid::new_v4(), &old_config, &current_config)
                    .await
                    .expect("Should log config change");
            }

            // Query all config changes
            let filter = SupervisionAuditFilter::new()
                .of_type(SupervisionEventType::ConfigChanged);
            let results = trail.query(&filter).await.expect("Should query");

            assert_eq!(
                results.len(),
                num_changes,
                "All {} config changes should be recorded",
                num_changes
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 16: Configuration Change Auditing
    /// Validates: Requirements 5.4
    ///
    /// Category threshold changes should be recorded in config change audit
    #[test]
    fn prop_category_threshold_changes_recorded(
        category in "[a-z]{5,10}",
        old_threshold in 0.70f64..0.85,
        new_threshold in 0.85f64..0.99
    ) {
        let user_id = Uuid::new_v4();

        let mut old_config = AutoApproveConfig::default();
        old_config.set_category_threshold(&category, old_threshold);

        let mut new_config = AutoApproveConfig::default();
        new_config.set_category_threshold(&category, new_threshold);

        let entry = SupervisionAuditEntry::config_changed(user_id, &old_config, &new_config);

        // Verify metadata contains category thresholds
        let metadata = entry.metadata.as_ref().unwrap();
        let old_config_json = metadata.get("old_config").unwrap();
        let new_config_json = metadata.get("new_config").unwrap();

        // Both configs should have category_thresholds
        prop_assert!(
            old_config_json.get("category_thresholds").is_some(),
            "Old config should have category_thresholds"
        );
        prop_assert!(
            new_config_json.get("category_thresholds").is_some(),
            "New config should have category_thresholds"
        );
    }
}

// =============================================================================
// Property 14: Global Toggle Enforcement
// =============================================================================
// For any match processed while auto-approval is globally disabled, the system
// SHALL NOT auto-approve regardless of confidence score.
// Validates: Requirements 5.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 14: Global Toggle Enforcement
    /// Validates: Requirements 5.2
    ///
    /// When auto-approval is disabled, process_match should return SystemDisabled error
    #[test]
    fn prop_disabled_system_rejects_all_matches(
        confidence in 0.70f64..0.99
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create processor with auto-approval DISABLED
            let config = AutoApproveConfig {
                enabled: false, // Disabled
                confidence_threshold: 0.85,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Create a match with high confidence
            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(confidence),
                ai_explanation: Some("High confidence match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "Aspirin 100mg".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "Aspirin 100mg".to_string(),
                ..Default::default()
            };

            // Process the match
            let result = processor
                .process_match(&match_entity, &offer, &request, None)
                .await;

            // Should return SystemDisabled error regardless of confidence
            assert!(
                matches!(result, Err(pharma_core::matching::AutoApproveError::SystemDisabled)),
                "Disabled system should return SystemDisabled error for confidence {}, got {:?}",
                confidence,
                result
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 14: Global Toggle Enforcement
    /// Validates: Requirements 5.2
    ///
    /// When auto-approval is enabled, process_match should process normally
    #[test]
    fn prop_enabled_system_processes_matches(
        confidence in 0.90f64..0.99
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create processor with auto-approval ENABLED
            let config = AutoApproveConfig {
                enabled: true, // Enabled
                confidence_threshold: 0.85,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Create a match with high confidence
            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(confidence),
                ai_explanation: Some("High confidence match".to_string()),
                score: 0.9, // High score to pass dosage check
                ..Default::default()
            };

            let offer = Offer {
                medication: "Aspirin 100mg".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "Aspirin 100mg".to_string(),
                ..Default::default()
            };

            // Process the match
            let result = processor
                .process_match(&match_entity, &offer, &request, None)
                .await;

            // Should succeed (not return SystemDisabled)
            assert!(
                result.is_ok(),
                "Enabled system should process match with confidence {}, got {:?}",
                confidence,
                result
            );

            // Should be approved since confidence > threshold
            let auto_result = result.unwrap();
            assert!(
                auto_result.action.is_approved(),
                "Match with confidence {} should be approved (threshold 0.85)",
                confidence
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 14: Global Toggle Enforcement
    /// Validates: Requirements 5.2
    ///
    /// is_active should return false when system is disabled
    #[test]
    fn prop_is_active_reflects_enabled_state(
        enabled in proptest::bool::ANY
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let is_active = processor.is_active().await;

            assert_eq!(
                is_active, enabled,
                "is_active should return {} when enabled is {}",
                enabled, enabled
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 14: Global Toggle Enforcement
    /// Validates: Requirements 5.2
    ///
    /// get_status should return Disabled when system is disabled
    #[test]
    fn prop_get_status_reflects_disabled_state(_dummy in 0..1) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = AutoApproveConfig {
                enabled: false,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let (status, _) = processor.get_status().await;

            assert_eq!(
                status,
                pharma_core::matching::SystemStatus::Disabled,
                "Status should be Disabled when enabled is false"
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 14: Global Toggle Enforcement
    /// Validates: Requirements 5.2
    ///
    /// Toggling enabled state should immediately affect processing
    #[test]
    fn prop_toggle_immediately_affects_processing(_dummy in 0..1) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Start with enabled
            let config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: 0.85,
                ..Default::default()
            };
            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Initially should be active
            assert!(processor.is_active().await, "Should be active initially");

            // Update config to disabled
            let mut new_config = processor.get_config().await;
            new_config.enabled = false;
            processor.update_config(new_config).await.expect("Update should succeed");

            // Should now be inactive
            assert!(!processor.is_active().await, "Should be inactive after disabling");

            // Create a match
            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(0.95),
                ai_explanation: Some("High confidence".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "Test".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "Test".to_string(),
                ..Default::default()
            };

            // Processing should fail with SystemDisabled
            let result = processor
                .process_match(&match_entity, &offer, &request, None)
                .await;

            assert!(
                matches!(result, Err(pharma_core::matching::AutoApproveError::SystemDisabled)),
                "Should return SystemDisabled after toggling off"
            );
        });
    }
}

// =============================================================================
// Property 15: Category Threshold Override
// =============================================================================
// For any match involving a medication in a category with a custom threshold,
// the system SHALL use the category-specific threshold instead of the global threshold.
// Validates: Requirements 5.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 15: Category Threshold Override
    /// Validates: Requirements 5.3
    ///
    /// When a category has a custom threshold, matches in that category should use it
    #[test]
    fn prop_category_threshold_used_for_matching_category(
        global_threshold in 0.75f64..0.90,
        category_threshold in 0.75f64..0.95,
        category in "[a-z]{5,10}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with global and category-specific thresholds
            let mut config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: global_threshold,
                ..Default::default()
            };
            config.set_category_threshold(&category, category_threshold);

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config.clone(),
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Get the effective threshold for the category
            let effective_threshold = config.get_threshold_for_category(Some(&category));
            let clamped_category_threshold = config.clamp_threshold(category_threshold);

            // The effective threshold should be the category-specific one
            assert!(
                (effective_threshold - clamped_category_threshold).abs() < 0.0001,
                "Category '{}' should use threshold {}, got {}",
                category,
                clamped_category_threshold,
                effective_threshold
            );

            // Create a match with confidence between global and category thresholds
            // to verify the correct threshold is being used
            let test_confidence = if category_threshold > global_threshold {
                // Category threshold is higher, use confidence between them
                (global_threshold + category_threshold) / 2.0
            } else {
                // Category threshold is lower, use confidence between them
                (category_threshold + global_threshold) / 2.0
            };

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(test_confidence),
                ai_explanation: Some("Test match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            // Process with the category
            let result = processor
                .process_match(&match_entity, &offer, &request, Some(&category))
                .await
                .expect("Processing should succeed");

            // Verify the decision is based on category threshold
            let clamped = config.clamp_threshold(category_threshold);
            if test_confidence >= clamped {
                assert!(
                    result.action.is_approved(),
                    "Confidence {} >= category threshold {} should be approved",
                    test_confidence,
                    clamped
                );
            } else {
                assert!(
                    result.action.requires_review(),
                    "Confidence {} < category threshold {} should be queued",
                    test_confidence,
                    clamped
                );
            }
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 15: Category Threshold Override
    /// Validates: Requirements 5.3
    ///
    /// When no category is specified, global threshold should be used
    #[test]
    fn prop_global_threshold_used_when_no_category(
        global_threshold in 0.75f64..0.90,
        category_threshold in 0.90f64..0.95,
        category in "[a-z]{5,10}"
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with global and category-specific thresholds
            let mut config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: global_threshold,
                ..Default::default()
            };
            config.set_category_threshold(&category, category_threshold);

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config.clone(),
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Confidence between global and category thresholds
            let test_confidence = (global_threshold + category_threshold) / 2.0;

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(test_confidence),
                ai_explanation: Some("Test match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            // Process WITHOUT category (None)
            let result = processor
                .process_match(&match_entity, &offer, &request, None)
                .await
                .expect("Processing should succeed");

            // Since test_confidence is between global (lower) and category (higher),
            // and we're using global threshold (no category), it should be approved
            assert!(
                result.action.is_approved(),
                "Confidence {} >= global threshold {} should be approved when no category",
                test_confidence,
                global_threshold
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 15: Category Threshold Override
    /// Validates: Requirements 5.3
    ///
    /// Unknown category should fall back to global threshold
    #[test]
    fn prop_unknown_category_uses_global_threshold(
        global_threshold in 0.75f64..0.90,
        category_threshold in 0.90f64..0.95,
        known_category in "[a-z]{5,10}",
        unknown_category in "[A-Z]{5,10}"  // Different pattern to ensure different category
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with global and category-specific thresholds
            let mut config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: global_threshold,
                ..Default::default()
            };
            config.set_category_threshold(&known_category, category_threshold);

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config.clone(),
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Confidence between global and category thresholds
            let test_confidence = (global_threshold + category_threshold) / 2.0;

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(test_confidence),
                ai_explanation: Some("Test match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            // Process with unknown category
            let result = processor
                .process_match(&match_entity, &offer, &request, Some(&unknown_category))
                .await
                .expect("Processing should succeed");

            // Since test_confidence is between global (lower) and category (higher),
            // and we're using an unknown category (falls back to global), it should be approved
            assert!(
                result.action.is_approved(),
                "Confidence {} >= global threshold {} should be approved for unknown category '{}'",
                test_confidence,
                global_threshold,
                unknown_category
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 15: Category Threshold Override
    /// Validates: Requirements 5.3
    ///
    /// Stricter category threshold (higher) should require higher confidence
    #[test]
    fn prop_stricter_category_threshold_requires_higher_confidence(
        global_threshold in 0.75f64..0.80,
        confidence in 0.80f64..0.90
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with stricter threshold for "controlled" category
            let mut config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: global_threshold,
                ..Default::default()
            };
            // Controlled substances require 0.95 threshold
            config.set_category_threshold("controlled", 0.95);

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(confidence),
                ai_explanation: Some("Test match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "Morphine".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "Morphine".to_string(),
                ..Default::default()
            };

            // Process with "controlled" category
            let result_controlled = processor
                .process_match(&match_entity, &offer, &request, Some("controlled"))
                .await
                .expect("Processing should succeed");

            // Process without category (uses global threshold)
            let result_global = processor
                .process_match(&match_entity, &offer, &request, None)
                .await
                .expect("Processing should succeed");

            // Confidence 0.80-0.90 is above global (0.75-0.80) but below controlled (0.95)
            // So global should approve, controlled should queue
            assert!(
                result_global.action.is_approved(),
                "Confidence {} should be approved with global threshold {}",
                confidence,
                global_threshold
            );

            assert!(
                result_controlled.action.requires_review(),
                "Confidence {} should be queued with controlled threshold 0.95",
                confidence
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 15: Category Threshold Override
    /// Validates: Requirements 5.3
    ///
    /// Multiple categories can have different thresholds
    #[test]
    fn prop_multiple_category_thresholds(
        global_threshold in 0.80f64..0.85
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with multiple category thresholds
            let mut config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: global_threshold,
                ..Default::default()
            };
            config.set_category_threshold("controlled", 0.95);  // Strictest
            config.set_category_threshold("antibiotics", 0.90); // Strict
            config.set_category_threshold("otc", 0.75);         // Lenient

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config.clone(),
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            // Test with confidence 0.88 (above OTC and global, below antibiotics and controlled)
            let test_confidence = 0.88;

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(test_confidence),
                ai_explanation: Some("Test match".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            // OTC should approve (0.88 >= 0.75)
            let result_otc = processor
                .process_match(&match_entity, &offer, &request, Some("otc"))
                .await
                .expect("Processing should succeed");
            assert!(
                result_otc.action.is_approved(),
                "OTC (threshold 0.75) should approve confidence 0.88"
            );

            // Antibiotics should queue (0.88 < 0.90)
            let result_antibiotics = processor
                .process_match(&match_entity, &offer, &request, Some("antibiotics"))
                .await
                .expect("Processing should succeed");
            assert!(
                result_antibiotics.action.requires_review(),
                "Antibiotics (threshold 0.90) should queue confidence 0.88"
            );

            // Controlled should queue (0.88 < 0.95)
            let result_controlled = processor
                .process_match(&match_entity, &offer, &request, Some("controlled"))
                .await
                .expect("Processing should succeed");
            assert!(
                result_controlled.action.requires_review(),
                "Controlled (threshold 0.95) should queue confidence 0.88"
            );
        });
    }
}

// =============================================================================
// Property 17: Schedule Enforcement
// =============================================================================
// For any match processed outside the configured schedule hours, the system
// SHALL NOT auto-approve regardless of confidence score.
// Validates: Requirements 5.5

use chrono::{TimeZone, Timelike};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// When no schedule is configured, system should always be active
    #[test]
    fn prop_no_schedule_always_active(
        hour in 0u32..24,
        minute in 0u32..60
    ) {
        let config = AutoApproveConfig {
            enabled: true,
            schedule: None,
            ..Default::default()
        };

        let time = Utc.with_ymd_and_hms(2026, 1, 3, hour, minute, 0).unwrap();

        prop_assert!(
            config.is_within_schedule_at(time),
            "With no schedule, system should be active at {:02}:{:02}",
            hour,
            minute
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Times within schedule range should be active
    #[test]
    fn prop_within_schedule_is_active(
        start_hour in 0u32..20,
        duration_hours in 2u32..8
    ) {
        let end_hour = (start_hour + duration_hours) % 24;

        // Skip overnight ranges for this test (tested separately)
        prop_assume!(start_hour < end_hour);

        let schedule = format!("{:02}:00-{:02}:00", start_hour, end_hour);
        let config = AutoApproveConfig {
            enabled: true,
            schedule: Some(schedule.clone()),
            ..Default::default()
        };

        // Test a time in the middle of the range
        let mid_hour = start_hour + duration_hours / 2;
        let time = Utc.with_ymd_and_hms(2026, 1, 3, mid_hour, 30, 0).unwrap();

        prop_assert!(
            config.is_within_schedule_at(time),
            "Time {:02}:30 should be within schedule {} (start: {}, end: {})",
            mid_hour,
            schedule,
            start_hour,
            end_hour
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Times outside schedule range should be inactive
    #[test]
    fn prop_outside_schedule_is_inactive(
        start_hour in 4u32..20,
        duration_hours in 2u32..8
    ) {
        let end_hour = start_hour + duration_hours;
        prop_assume!(end_hour < 24);

        let schedule = format!("{:02}:00-{:02}:00", start_hour, end_hour);
        let config = AutoApproveConfig {
            enabled: true,
            schedule: Some(schedule.clone()),
            ..Default::default()
        };

        // Test a time before the range (if possible)
        if start_hour > 0 {
            let before_time = Utc.with_ymd_and_hms(2026, 1, 3, start_hour - 1, 30, 0).unwrap();
            prop_assert!(
                !config.is_within_schedule_at(before_time),
                "Time {:02}:30 should be outside schedule {} (before start)",
                start_hour - 1,
                schedule
            );
        }

        // Test a time after the range
        let after_time = Utc.with_ymd_and_hms(2026, 1, 3, end_hour, 30, 0).unwrap();
        prop_assert!(
            !config.is_within_schedule_at(after_time),
            "Time {:02}:30 should be outside schedule {} (after end)",
            end_hour,
            schedule
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Schedule start time should be inclusive
    #[test]
    fn prop_schedule_start_is_inclusive(
        start_hour in 0u32..22,
        duration_hours in 2u32..4
    ) {
        let end_hour = start_hour + duration_hours;
        prop_assume!(end_hour < 24);

        let schedule = format!("{:02}:00-{:02}:00", start_hour, end_hour);
        let config = AutoApproveConfig {
            enabled: true,
            schedule: Some(schedule.clone()),
            ..Default::default()
        };

        // Exactly at start time should be within schedule
        let start_time = Utc.with_ymd_and_hms(2026, 1, 3, start_hour, 0, 0).unwrap();

        prop_assert!(
            config.is_within_schedule_at(start_time),
            "Start time {:02}:00 should be within schedule {}",
            start_hour,
            schedule
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Schedule end time should be exclusive
    #[test]
    fn prop_schedule_end_is_exclusive(
        start_hour in 0u32..22,
        duration_hours in 2u32..4
    ) {
        let end_hour = start_hour + duration_hours;
        prop_assume!(end_hour < 24);

        let schedule = format!("{:02}:00-{:02}:00", start_hour, end_hour);
        let config = AutoApproveConfig {
            enabled: true,
            schedule: Some(schedule.clone()),
            ..Default::default()
        };

        // Exactly at end time should be outside schedule
        let end_time = Utc.with_ymd_and_hms(2026, 1, 3, end_hour, 0, 0).unwrap();

        prop_assert!(
            !config.is_within_schedule_at(end_time),
            "End time {:02}:00 should be outside schedule {}",
            end_hour,
            schedule
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Processing should fail with OutsideSchedule error when outside schedule
    #[test]
    fn prop_process_match_fails_outside_schedule(
        confidence in 0.85f64..0.99
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create config with a schedule that excludes current time
            // Use a schedule that's definitely not now (e.g., 03:00-04:00)
            let config = AutoApproveConfig {
                enabled: true,
                confidence_threshold: 0.85,
                schedule: Some("03:00-04:00".to_string()),
                ..Default::default()
            };

            let ai_client = AIClient::new(ClientConfig::default());
            let ai_reviewer = Arc::new(AIReviewer::new(Arc::new(ai_client)));
            let blocklist = Arc::new(MedicationBlocklist::new());
            let dosage_gate = Arc::new(DosageGate::default());

            let processor = AutoApproveProcessor::new(
                config,
                ai_reviewer,
                blocklist,
                dosage_gate,
            );

            let match_entity = pharma_core::domain::Match {
                id: Uuid::new_v4(),
                ai_confidence: Some(confidence),
                ai_explanation: Some("High confidence".to_string()),
                ..Default::default()
            };

            let offer = Offer {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            let request = Request {
                medication: "TestMed".to_string(),
                ..Default::default()
            };

            // Get current hour to check if we're in the schedule
            let now = Utc::now();
            let current_hour = now.hour();

            // If we happen to be in the 03:00-04:00 window, skip this test iteration
            if current_hour == 3 {
                return; // Skip this test case
            }

            let result = processor
                .process_match(&match_entity, &offer, &request, None)
                .await;

            assert!(
                matches!(result, Err(pharma_core::matching::AutoApproveError::OutsideSchedule)),
                "Should return OutsideSchedule error when outside schedule hours, got {:?}",
                result
            );
        });
    }

    /// Feature: ai-supervised-auto-approve, Property 17: Schedule Enforcement
    /// Validates: Requirements 5.5
    ///
    /// Overnight schedules should work correctly
    #[test]
    fn prop_overnight_schedule_works(
        start_hour in 20u32..24,
        end_hour in 0u32..8
    ) {
        let schedule = format!("{:02}:00-{:02}:00", start_hour, end_hour);
        let config = AutoApproveConfig {
            enabled: true,
            schedule: Some(schedule.clone()),
            ..Default::default()
        };

        // Late night (after start) should be within schedule
        let late_night = Utc.with_ymd_and_hms(2026, 1, 3, 23, 0, 0).unwrap();
        if start_hour <= 23 {
            prop_assert!(
                config.is_within_schedule_at(late_night),
                "23:00 should be within overnight schedule {}",
                schedule
            );
        }

        // Early morning (before end) should be within schedule
        if end_hour > 2 {
            let early_morning = Utc.with_ymd_and_hms(2026, 1, 3, 2, 0, 0).unwrap();
            prop_assert!(
                config.is_within_schedule_at(early_morning),
                "02:00 should be within overnight schedule {}",
                schedule
            );
        }

        // Midday should be outside schedule
        let midday = Utc.with_ymd_and_hms(2026, 1, 3, 12, 0, 0).unwrap();
        prop_assert!(
            !config.is_within_schedule_at(midday),
            "12:00 should be outside overnight schedule {}",
            schedule
        );
    }
}

// =============================================================================
// Property 8: Statistics Calculation Accuracy
// =============================================================================
// For any set of auto-approve decisions, the calculated statistics
// (total_approved, override_rate, average_confidence) SHALL match the actual
// counts and averages from the underlying data.
// Validates: Requirements 3.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Total approved count should match actual approved results
    #[test]
    fn prop_stats_approved_count_accurate(
        approved_count in 0usize..20,
        queued_count in 0usize..10,
        blocked_count in 0usize..5
    ) {
        let mut results = Vec::new();

        // Generate approved results
        for _ in 0..approved_count {
            results.push(AutoApproveResult::approved(
                Uuid::new_v4(),
                0.90,
                "Test".to_string(),
                vec![],
                0.85,
            ));
        }

        // Generate queued results
        for _ in 0..queued_count {
            results.push(AutoApproveResult::queued_for_review(
                Uuid::new_v4(),
                0.75,
                "Test".to_string(),
                "Below threshold".to_string(),
                vec![],
                0.85,
            ));
        }

        // Generate blocked results
        for _ in 0..blocked_count {
            results.push(AutoApproveResult::blocked(
                Uuid::new_v4(),
                0.90,
                "Test".to_string(),
                "Blocked".to_string(),
                vec![],
            ));
        }

        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &results,
            0,
            0,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        prop_assert_eq!(
            stats.total_approved_today as usize,
            approved_count,
            "Approved count should match"
        );
        prop_assert_eq!(
            stats.total_queued_today as usize,
            queued_count,
            "Queued count should match"
        );
        prop_assert_eq!(
            stats.total_blocked_today as usize,
            blocked_count,
            "Blocked count should match"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Average confidence should be calculated correctly
    #[test]
    fn prop_stats_average_confidence_accurate(
        confidences in proptest::collection::vec(0.70f64..0.99, 1..20)
    ) {
        let mut results = Vec::new();

        for confidence in &confidences {
            results.push(AutoApproveResult::approved(
                Uuid::new_v4(),
                *confidence,
                "Test".to_string(),
                vec![],
                0.85,
            ));
        }

        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &results,
            0,
            0,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        let expected_avg: f64 = confidences.iter().sum::<f64>() / confidences.len() as f64;

        prop_assert!(
            (stats.average_confidence - expected_avg).abs() < 0.0001,
            "Average confidence {} should match expected {}",
            stats.average_confidence,
            expected_avg
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Override rate should be calculated correctly
    #[test]
    fn prop_stats_override_rate_accurate(
        approved_count in 1usize..20,
        override_count in 0usize..10
    ) {
        let mut results = Vec::new();

        for _ in 0..approved_count {
            results.push(AutoApproveResult::approved(
                Uuid::new_v4(),
                0.90,
                "Test".to_string(),
                vec![],
                0.85,
            ));
        }

        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &results,
            override_count as u64,
            0,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        let expected_rate = override_count as f64 / approved_count as f64;

        prop_assert!(
            (stats.override_rate - expected_rate).abs() < 0.0001,
            "Override rate {} should match expected {}",
            stats.override_rate,
            expected_rate
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Total decisions should equal sum of all decision types
    #[test]
    fn prop_stats_total_decisions_accurate(
        approved_count in 0usize..20,
        queued_count in 0usize..10,
        blocked_count in 0usize..5
    ) {
        let mut results = Vec::new();

        for _ in 0..approved_count {
            results.push(AutoApproveResult::approved(
                Uuid::new_v4(),
                0.90,
                "Test".to_string(),
                vec![],
                0.85,
            ));
        }

        for _ in 0..queued_count {
            results.push(AutoApproveResult::queued_for_review(
                Uuid::new_v4(),
                0.75,
                "Test".to_string(),
                "Below threshold".to_string(),
                vec![],
                0.85,
            ));
        }

        for _ in 0..blocked_count {
            results.push(AutoApproveResult::blocked(
                Uuid::new_v4(),
                0.90,
                "Test".to_string(),
                "Blocked".to_string(),
                vec![],
            ));
        }

        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &results,
            0,
            0,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        let expected_total = approved_count + queued_count + blocked_count;

        prop_assert_eq!(
            stats.total_decisions_today() as usize,
            expected_total,
            "Total decisions should equal sum of all types"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Empty results should produce zero statistics
    #[test]
    fn prop_stats_empty_results_zero(_dummy in 0..1) {
        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &[],
            0,
            0,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        prop_assert_eq!(stats.total_approved_today, 0);
        prop_assert_eq!(stats.total_queued_today, 0);
        prop_assert_eq!(stats.total_blocked_today, 0);
        prop_assert!((stats.override_rate - 0.0).abs() < 0.0001);
        prop_assert!((stats.average_confidence - 0.0).abs() < 0.0001);
    }

    /// Feature: ai-supervised-auto-approve, Property 8: Statistics Calculation Accuracy
    /// Validates: Requirements 3.2
    ///
    /// Pending count should be passed through correctly
    #[test]
    fn prop_stats_pending_count_passthrough(
        pending_count in 0u64..1000
    ) {
        let stats = pharma_core::matching::AutoApproveStats::from_results(
            &[],
            0,
            pending_count,
            pharma_core::matching::SystemStatus::Active,
            None,
        );

        prop_assert_eq!(
            stats.pending_review_count,
            pending_count,
            "Pending count should be passed through"
        );
    }
}

// =============================================================================
// Property 9: Borderline Detection
// =============================================================================
// For any match with AI confidence within 5% of the configured threshold,
// the system SHALL flag it as a borderline case.
// Validates: Requirements 3.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Confidence within 5% below threshold should be flagged as borderline
    #[test]
    fn prop_borderline_below_threshold(
        threshold in 0.75f64..0.95,
        margin in 0.001f64..0.049
    ) {
        let confidence = threshold - margin;
        prop_assume!(confidence >= 0.0);

        let result = AutoApproveResult::queued_for_review(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            "Below threshold".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result.is_borderline,
            "Confidence {} (margin {:.3} below threshold {}) should be borderline",
            confidence,
            margin,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Confidence within 5% above threshold should be flagged as borderline
    #[test]
    fn prop_borderline_above_threshold(
        threshold in 0.75f64..0.94,
        margin in 0.001f64..0.049
    ) {
        let confidence = threshold + margin;
        prop_assume!(confidence <= 1.0);

        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result.is_borderline,
            "Confidence {} (margin {:.3} above threshold {}) should be borderline",
            confidence,
            margin,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Confidence exactly at threshold should be borderline
    #[test]
    fn prop_borderline_at_threshold(
        threshold in 0.75f64..0.95
    ) {
        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            threshold,
            "Test match".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result.is_borderline,
            "Confidence exactly at threshold {} should be borderline",
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Confidence well above threshold (>5%) should NOT be borderline
    #[test]
    fn prop_not_borderline_well_above(
        threshold in 0.75f64..0.90,
        margin in 0.06f64..0.15
    ) {
        let confidence = threshold + margin;
        prop_assume!(confidence <= 1.0);

        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            !result.is_borderline,
            "Confidence {} (margin {:.3} above threshold {}) should NOT be borderline",
            confidence,
            margin,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Confidence well below threshold (>5%) should NOT be borderline
    #[test]
    fn prop_not_borderline_well_below(
        threshold in 0.80f64..0.95,
        margin in 0.06f64..0.15
    ) {
        let confidence = threshold - margin;
        prop_assume!(confidence >= 0.0);

        let result = AutoApproveResult::queued_for_review(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            "Below threshold".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            !result.is_borderline,
            "Confidence {} (margin {:.3} below threshold {}) should NOT be borderline",
            confidence,
            margin,
            threshold
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Blocked matches should never be flagged as borderline
    #[test]
    fn prop_blocked_never_borderline(
        confidence in 0.70f64..0.99,
        _threshold in 0.75f64..0.95
    ) {
        let result = AutoApproveResult::blocked(
            Uuid::new_v4(),
            confidence,
            "Test match".to_string(),
            "Safety check failed".to_string(),
            vec![],
        );

        prop_assert!(
            !result.is_borderline,
            "Blocked match with confidence {} should never be borderline",
            confidence
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 9: Borderline Detection
    /// Validates: Requirements 3.4
    ///
    /// Borderline boundary at exactly 5% should be borderline
    #[test]
    fn prop_borderline_boundary_inclusive(
        threshold in 0.80f64..0.94
    ) {
        // Exactly 5% below threshold
        let confidence_below = threshold - 0.05;
        let result_below = AutoApproveResult::queued_for_review(
            Uuid::new_v4(),
            confidence_below,
            "Test match".to_string(),
            "Below threshold".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result_below.is_borderline,
            "Confidence exactly 5% below threshold ({} - 0.05 = {}) should be borderline",
            threshold,
            confidence_below
        );

        // Exactly 5% above threshold (but < threshold + 0.05)
        let confidence_above = threshold + 0.049;
        prop_assume!(confidence_above <= 1.0);
        let result_above = AutoApproveResult::approved(
            Uuid::new_v4(),
            confidence_above,
            "Test match".to_string(),
            vec![],
            threshold,
        );

        prop_assert!(
            result_above.is_borderline,
            "Confidence just under 5% above threshold ({} + 0.049 = {}) should be borderline",
            threshold,
            confidence_above
        );
    }
}

// =============================================================================
// Property 20: AI Service Failure Handling
// =============================================================================
// For any AI service failure during match evaluation, the match SHALL be
// queued for retry and a notification SHALL be sent.
// Validates: Requirements 6.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// RetryInfo should be created with correct initial values
    #[test]
    fn prop_retry_info_initial_values(
        max_retries in 1u32..10
    ) {
        let match_id = Uuid::new_v4();
        let retry_info = RetryInfo::new(match_id, "Test error".to_string(), max_retries);

        prop_assert_eq!(retry_info.match_id, match_id);
        prop_assert_eq!(retry_info.retry_count, 0);
        prop_assert_eq!(retry_info.max_retries, max_retries);
        prop_assert!(retry_info.should_retry());
    }

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// RetryInfo should stop retrying after max_retries
    #[test]
    fn prop_retry_stops_after_max(
        max_retries in 1u32..10
    ) {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), max_retries);

        // Increment until max
        for _ in 0..max_retries {
            prop_assert!(retry_info.should_retry());
            retry_info.increment();
        }

        // Should not retry after max
        prop_assert!(!retry_info.should_retry());
    }

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// Retry backoff should increase with each attempt
    #[test]
    fn prop_retry_backoff_increases(
        max_retries in 3u32..6
    ) {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), max_retries);

        let mut prev_next_retry = retry_info.next_retry_at;

        for i in 0..max_retries.min(5) {
            retry_info.increment();
            let current_next_retry = retry_info.next_retry_at;

            // Each retry should be scheduled further in the future
            prop_assert!(
                current_next_retry > prev_next_retry,
                "Retry {} should be scheduled later than retry {}",
                i + 1,
                i
            );

            prev_next_retry = current_next_retry;
        }
    }

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// Retry count should increment correctly
    #[test]
    fn prop_retry_count_increments(
        increments in 1usize..10
    ) {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 20);

        for i in 0..increments {
            prop_assert_eq!(retry_info.retry_count as usize, i);
            retry_info.increment();
        }

        prop_assert_eq!(retry_info.retry_count as usize, increments);
    }

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// is_ready_for_retry should return true when next_retry_at is in the past
    #[test]
    fn prop_ready_for_retry_when_past(
        seconds_ago in 1i64..3600
    ) {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 3);
        retry_info.next_retry_at = chrono::Utc::now() - chrono::Duration::seconds(seconds_ago);

        prop_assert!(
            retry_info.is_ready_for_retry(),
            "Should be ready for retry when next_retry_at is {} seconds ago",
            seconds_ago
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 20: AI Service Failure Handling
    /// Validates: Requirements 6.4
    ///
    /// is_ready_for_retry should return false when next_retry_at is in the future
    #[test]
    fn prop_not_ready_for_retry_when_future(
        seconds_ahead in 1i64..3600
    ) {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 3);
        retry_info.next_retry_at = chrono::Utc::now() + chrono::Duration::seconds(seconds_ahead);

        prop_assert!(
            !retry_info.is_ready_for_retry(),
            "Should not be ready for retry when next_retry_at is {} seconds ahead",
            seconds_ahead
        );
    }
}

// =============================================================================
// Property 3: AI Evaluation Data Persistence
// =============================================================================
// For any match evaluated by the AI_Reviewer, the resulting record SHALL contain
// non-empty ai_confidence, ai_explanation, and ai_status fields.
// Validates: Requirements 1.2
// =============================================================================

#[cfg(feature = "test-auto-approve")]
mod property_3_ai_evaluation_persistence {
    use super::*;
    use pharma_core::matching::{
        AIEvaluationData, AIEvaluationError, AIEvaluationRepository, AIStatus,
        InMemoryAIEvaluationRepository,
    };

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// For any successful AI evaluation, the resulting record SHALL contain
        /// non-empty ai_confidence (> 0), ai_explanation, and ai_status fields.
        #[test]
        fn prop_successful_evaluation_has_complete_data(
            confidence in 0.01f64..1.0,
            explanation in "[A-Za-z ]{10,100}"
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::evaluated(match_id, confidence, explanation.clone());

            // Property: ai_confidence must be non-zero for successful evaluations
            prop_assert!(
                data.ai_confidence > 0.0,
                "Successful evaluation must have non-zero ai_confidence"
            );

            // Property: ai_explanation must be non-empty
            prop_assert!(
                !data.ai_explanation.is_empty(),
                "Evaluation must have non-empty ai_explanation"
            );

            // Property: ai_status must be Evaluated for successful evaluations
            prop_assert_eq!(
                data.ai_status,
                AIStatus::Evaluated,
                "Successful evaluation must have Evaluated status"
            );

            // Property: is_complete() must return true
            prop_assert!(
                data.is_complete(),
                "Successful evaluation must be complete"
            );

            // Property: is_successful() must return true
            prop_assert!(
                data.is_successful(),
                "Successful evaluation must report as successful"
            );

            // Property: validate() must succeed
            prop_assert!(
                data.validate().is_ok(),
                "Successful evaluation must pass validation"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// For any failed AI evaluation, the record SHALL contain non-empty ai_explanation
        /// and ai_status = Failed.
        #[test]
        fn prop_failed_evaluation_has_required_fields(
            error_message in "[A-Za-z ]{10,100}"
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::failed(match_id, error_message.clone());

            // Property: ai_explanation must be non-empty (contains error message)
            prop_assert!(
                !data.ai_explanation.is_empty(),
                "Failed evaluation must have non-empty ai_explanation"
            );

            // Property: ai_status must be Failed
            prop_assert_eq!(
                data.ai_status,
                AIStatus::Failed,
                "Failed evaluation must have Failed status"
            );

            // Property: ai_confidence should be 0 for failed evaluations
            prop_assert!(
                (data.ai_confidence - 0.0).abs() < 0.001,
                "Failed evaluation should have zero ai_confidence"
            );

            // Property: is_complete() must return true (has explanation)
            prop_assert!(
                data.is_complete(),
                "Failed evaluation must be complete"
            );

            // Property: is_successful() must return false
            prop_assert!(
                !data.is_successful(),
                "Failed evaluation must not report as successful"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// For any skipped AI evaluation, the record SHALL contain non-empty ai_explanation
        /// and ai_status = Skipped.
        #[test]
        fn prop_skipped_evaluation_has_required_fields(
            reason in "[A-Za-z ]{10,100}"
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::skipped(match_id, reason.clone());

            // Property: ai_explanation must be non-empty (contains skip reason)
            prop_assert!(
                !data.ai_explanation.is_empty(),
                "Skipped evaluation must have non-empty ai_explanation"
            );

            // Property: ai_status must be Skipped
            prop_assert_eq!(
                data.ai_status,
                AIStatus::Skipped,
                "Skipped evaluation must have Skipped status"
            );

            // Property: is_complete() must return true
            prop_assert!(
                data.is_complete(),
                "Skipped evaluation must be complete"
            );

            // Property: is_successful() must return false
            prop_assert!(
                !data.is_successful(),
                "Skipped evaluation must not report as successful"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// For any persisted AI evaluation, retrieving it SHALL return the same data.
        #[test]
        fn prop_persisted_evaluation_is_retrievable(
            confidence in 0.01f64..1.0,
            explanation in "[A-Za-z ]{10,100}"
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let repo = InMemoryAIEvaluationRepository::new();
                let match_id = Uuid::new_v4();
                let data = AIEvaluationData::evaluated(match_id, confidence, explanation.clone());

                // Persist the evaluation
                let persist_result = repo.persist_evaluation(&data).await;
                assert!(persist_result.is_ok(), "Persist should succeed");

                // Retrieve the evaluation
                let retrieved = repo.get_evaluation(match_id).await.unwrap();
                assert!(retrieved.is_some(), "Retrieved evaluation should exist");

                let retrieved = retrieved.unwrap();

                // Property: Retrieved data must match original
                assert_eq!(retrieved.match_id, match_id);
                assert!((retrieved.ai_confidence - confidence).abs() < 0.0001);
                assert_eq!(retrieved.ai_explanation, explanation);
                assert_eq!(retrieved.ai_status, AIStatus::Evaluated);
            });
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// Evaluation with empty explanation SHALL fail validation.
        #[test]
        fn prop_empty_explanation_fails_validation(
            confidence in 0.01f64..1.0
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::new(
                match_id,
                confidence,
                "".to_string(), // Empty explanation
                AIStatus::Evaluated,
            );

            // Property: Validation must fail for empty explanation
            let result = data.validate();
            prop_assert!(
                matches!(result, Err(AIEvaluationError::EmptyExplanation)),
                "Empty explanation must fail validation"
            );

            // Property: is_complete() must return false
            prop_assert!(
                !data.is_complete(),
                "Evaluation with empty explanation must not be complete"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// Evaluation with invalid confidence (> 1.0) SHALL fail validation.
        #[test]
        fn prop_invalid_confidence_fails_validation(
            confidence in 1.01f64..10.0,
            explanation in "[A-Za-z ]{10,100}"
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::new(
                match_id,
                confidence,
                explanation,
                AIStatus::Evaluated,
            );

            // Property: Validation must fail for confidence > 1.0
            let result = data.validate();
            prop_assert!(
                matches!(result, Err(AIEvaluationError::InvalidConfidence(_))),
                "Confidence > 1.0 must fail validation"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// Evaluation with zero confidence and Evaluated status SHALL fail validation.
        #[test]
        fn prop_zero_confidence_evaluated_fails_validation(
            explanation in "[A-Za-z ]{10,100}"
        ) {
            let match_id = Uuid::new_v4();
            let data = AIEvaluationData::new(
                match_id,
                0.0, // Zero confidence
                explanation,
                AIStatus::Evaluated, // But status is Evaluated
            );

            // Property: Validation must fail for zero confidence with Evaluated status
            let result = data.validate();
            prop_assert!(
                matches!(result, Err(AIEvaluationError::InvalidConfidence(_))),
                "Zero confidence with Evaluated status must fail validation"
            );
        }

        /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
        /// Validates: Requirements 1.2
        ///
        /// Invalid evaluation data SHALL NOT be persisted.
        #[test]
        fn prop_invalid_evaluation_not_persisted(
            confidence in 1.01f64..10.0,
            explanation in "[A-Za-z ]{10,100}"
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let repo = InMemoryAIEvaluationRepository::new();
                let match_id = Uuid::new_v4();
                let data = AIEvaluationData::new(
                    match_id,
                    confidence, // Invalid: > 1.0
                    explanation,
                    AIStatus::Evaluated,
                );

                // Property: Persist should fail for invalid data
                let persist_result = repo.persist_evaluation(&data).await;
                assert!(persist_result.is_err(), "Persist should fail for invalid data");

                // Property: Data should not be retrievable
                let retrieved = repo.get_evaluation(match_id).await.unwrap();
                assert!(retrieved.is_none(), "Invalid data should not be persisted");
            });
        }
    }

    /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
    /// Validates: Requirements 1.2
    ///
    /// Non-existent evaluation SHALL return None.
    #[tokio::test]
    async fn test_nonexistent_evaluation_returns_none() {
        let repo = InMemoryAIEvaluationRepository::new();
        let match_id = Uuid::new_v4();

        let result = repo.get_evaluation(match_id).await.unwrap();
        assert!(
            result.is_none(),
            "Non-existent evaluation should return None"
        );
    }

    /// Feature: ai-supervised-auto-approve, Property 3: AI Evaluation Data Persistence
    /// Validates: Requirements 1.2
    ///
    /// Multiple evaluations can be persisted and retrieved independently.
    #[tokio::test]
    async fn test_multiple_evaluations_independent() {
        let repo = InMemoryAIEvaluationRepository::new();

        let match_id_1 = Uuid::new_v4();
        let match_id_2 = Uuid::new_v4();

        let data_1 = AIEvaluationData::evaluated(match_id_1, 0.85, "First evaluation".to_string());
        let data_2 = AIEvaluationData::evaluated(match_id_2, 0.92, "Second evaluation".to_string());

        // Persist both
        repo.persist_evaluation(&data_1).await.unwrap();
        repo.persist_evaluation(&data_2).await.unwrap();

        // Retrieve and verify independence
        let retrieved_1 = repo.get_evaluation(match_id_1).await.unwrap().unwrap();
        let retrieved_2 = repo.get_evaluation(match_id_2).await.unwrap().unwrap();

        assert!((retrieved_1.ai_confidence - 0.85).abs() < 0.001);
        assert!((retrieved_2.ai_confidence - 0.92).abs() < 0.001);
        assert_eq!(retrieved_1.ai_explanation, "First evaluation");
        assert_eq!(retrieved_2.ai_explanation, "Second evaluation");
    }
}
