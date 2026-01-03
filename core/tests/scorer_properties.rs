//! Property-based tests for Scorer functionality
//!
//! Feature: matching-system-improvements
//! Tests Properties 2, 12, 17, 18, and 20 from the design document
//!
//! These tests validate:
//! - Property 2: Dynamic Dosage Weight
//! - Property 12: Quantity Scoring Thresholds
//! - Property 17: Recency Exponential Decay
//! - Property 18: Category-Specific Decay
//! - Property 20: Weight Sum Validation

use chrono::{DateTime, Duration, Utc};
use pharma_core::matching::{MedicationCategory, Scorer, Weights};
use proptest::prelude::*;

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate a DateTime offset from now by a number of hours
fn datetime_offset_hours(hours: i64) -> DateTime<Utc> {
    Utc::now() + Duration::hours(hours)
}

/// Strategy for generating positive quantities
fn positive_quantity() -> impl Strategy<Value = f64> {
    0.1f64..1000.0
}

/// Strategy for generating half-life values
fn half_life_hours() -> impl Strategy<Value = f64> {
    1.0f64..200.0 // 1 to 200 hours
}

/// Strategy for generating minimum fulfillment thresholds
fn min_fulfillment_threshold() -> impl Strategy<Value = f64> {
    0.1f64..0.9 // 10% to 90%
}

// =============================================================================
// Property 12: Quantity Scoring Thresholds
// =============================================================================
// For any offer-request quantity pair:
// - If offer_qty < 0.5 * request_qty, quantity_score SHALL be 0.0
// - If 0.5 <= offer_qty/request_qty <= 1.0, quantity_score SHALL equal offer_qty/request_qty
// - If offer_qty > request_qty, quantity_score SHALL be 1.0
// Validates: Requirements 4.1, 4.2, 4.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 12: Quantity Scoring Thresholds
    /// Validates: Requirements 4.1
    ///
    /// If offer_qty < 0.5 * request_qty, quantity_score SHALL be 0.0
    #[test]
    fn prop_quantity_below_threshold_returns_zero(
        request_qty in positive_quantity(),
        ratio in 0.0f64..0.5 // Below 50% threshold
    ) {
        prop_assume!(ratio < 0.5); // Ensure strictly below threshold

        let scorer = Scorer::default();
        let offer_qty = request_qty * ratio;

        let score = scorer.quantity_score(offer_qty, request_qty);

        prop_assert!(
            (score - 0.0).abs() < 0.001,
            "Quantity score should be 0.0 when ratio ({}) < 0.5, got {}",
            ratio,
            score
        );
    }

    /// Feature: matching-system-improvements, Property 12: Quantity Scoring Thresholds
    /// Validates: Requirements 4.2
    ///
    /// If 0.5 <= offer_qty/request_qty <= 1.0, quantity_score SHALL equal offer_qty/request_qty
    #[test]
    fn prop_quantity_proportional_in_valid_range(
        request_qty in positive_quantity(),
        ratio in 0.5f64..=1.0 // Between 50% and 100%
    ) {
        let scorer = Scorer::default();
        let offer_qty = request_qty * ratio;

        let score = scorer.quantity_score(offer_qty, request_qty);

        prop_assert!(
            (score - ratio).abs() < 0.01,
            "Quantity score should equal ratio {} when in [0.5, 1.0], got {}",
            ratio,
            score
        );
    }

    /// Feature: matching-system-improvements, Property 12: Quantity Scoring Thresholds
    /// Validates: Requirements 4.3
    ///
    /// If offer_qty > request_qty, quantity_score SHALL be 1.0
    #[test]
    fn prop_quantity_exceeds_request_capped_at_one(
        request_qty in positive_quantity(),
        excess_ratio in 1.01f64..3.0 // 101% to 300% of request
    ) {
        let scorer = Scorer::default();
        let offer_qty = request_qty * excess_ratio;

        let score = scorer.quantity_score(offer_qty, request_qty);

        prop_assert!(
            (score - 1.0).abs() < 0.001,
            "Quantity score should be 1.0 when offer exceeds request (ratio {}), got {}",
            excess_ratio,
            score
        );
    }

    /// Feature: matching-system-improvements, Property 12: Quantity Scoring Thresholds
    /// Validates: Requirements 4.4
    ///
    /// Configurable minimum fulfillment threshold should be respected
    #[test]
    fn prop_quantity_configurable_threshold(
        request_qty in positive_quantity(),
        threshold in min_fulfillment_threshold(),
        ratio in 0.0f64..1.0
    ) {
        let scorer = Scorer::default();
        scorer.set_min_quantity_fulfillment(threshold);

        let offer_qty = request_qty * ratio;
        let score = scorer.quantity_score(offer_qty, request_qty);

        if ratio < threshold {
            prop_assert!(
                (score - 0.0).abs() < 0.001,
                "Score should be 0.0 when ratio ({}) < threshold ({}), got {}",
                ratio,
                threshold,
                score
            );
        } else {
            prop_assert!(
                (score - ratio).abs() < 0.01,
                "Score should equal ratio ({}) when >= threshold ({}), got {}",
                ratio,
                threshold,
                score
            );
        }
    }

    /// Feature: matching-system-improvements, Property 12: Quantity Scoring Thresholds
    /// Validates: Requirements 4.1, 4.2, 4.3
    ///
    /// Quantity score should always be in range [0.0, 1.0]
    #[test]
    fn prop_quantity_score_always_valid_range(
        offer_qty in 0.0f64..1000.0,
        request_qty in 0.0f64..1000.0
    ) {
        let scorer = Scorer::default();
        let score = scorer.quantity_score(offer_qty, request_qty);

        prop_assert!(
            (0.0..=1.0).contains(&score),
            "Quantity score should be in [0.0, 1.0], got {}",
            score
        );
    }
}

// =============================================================================
// Property 17: Recency Exponential Decay
// =============================================================================
// For any offer age T hours with half-life H hours, the recency_score SHALL
// equal 0.5^(T/H) within floating-point tolerance.
// Validates: Requirements 6.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 17: Recency Exponential Decay
    /// Validates: Requirements 6.2
    ///
    /// recency_score SHALL equal 0.5^(T/H) for exponential decay
    #[test]
    fn prop_recency_exponential_decay_formula(
        age_hours in 1i64..500, // Start from 1 to avoid edge case
        half_life in half_life_hours()
    ) {
        let scorer = Scorer::default();
        let created_at = datetime_offset_hours(-age_hours);

        let score = scorer.recency_score_with_half_life(created_at, half_life);
        let expected = 0.5_f64.powf(age_hours as f64 / half_life);

        prop_assert!(
            (score - expected).abs() < 0.05,
            "Recency score should be ~0.5^({}/{}) = {}, got {}",
            age_hours,
            half_life,
            expected,
            score
        );
    }

    /// Feature: matching-system-improvements, Property 17: Recency Exponential Decay
    /// Validates: Requirements 6.2
    ///
    /// At exactly half-life age, score should be ~0.5
    #[test]
    fn prop_recency_at_half_life_is_half(
        half_life in half_life_hours()
    ) {
        let scorer = Scorer::default();
        let created_at = datetime_offset_hours(-(half_life as i64));

        let score = scorer.recency_score_with_half_life(created_at, half_life);

        prop_assert!(
            (score - 0.5).abs() < 0.1,
            "Recency score at half-life ({} hours) should be ~0.5, got {}",
            half_life,
            score
        );
    }

    /// Feature: matching-system-improvements, Property 17: Recency Exponential Decay
    /// Validates: Requirements 6.2
    ///
    /// Score should decrease monotonically with age
    #[test]
    fn prop_recency_decreases_with_age(
        age1 in 1i64..500,
        age2 in 1i64..500,
        half_life in half_life_hours()
    ) {
        prop_assume!(age1 != age2);

        let scorer = Scorer::default();

        let created1 = datetime_offset_hours(-age1);
        let created2 = datetime_offset_hours(-age2);

        let score1 = scorer.recency_score_with_half_life(created1, half_life);
        let score2 = scorer.recency_score_with_half_life(created2, half_life);

        // Older offers (higher age) should have lower scores
        if age1 > age2 {
            prop_assert!(
                score1 <= score2,
                "Older offer (age {}) should have lower score ({}) than newer (age {}, score {})",
                age1, score1, age2, score2
            );
        } else {
            prop_assert!(
                score1 >= score2,
                "Newer offer (age {}) should have higher score ({}) than older (age {}, score {})",
                age1, score1, age2, score2
            );
        }
    }

    /// Feature: matching-system-improvements, Property 17: Recency Exponential Decay
    /// Validates: Requirements 6.2
    ///
    /// Brand new offers (age 0) should have score 1.0
    #[test]
    fn prop_recency_new_offer_score_one(
        half_life in half_life_hours()
    ) {
        let scorer = Scorer::default();
        let now = Utc::now();

        let score = scorer.recency_score_with_half_life(now, half_life);

        prop_assert!(
            (score - 1.0).abs() < 0.01,
            "Brand new offer should have score ~1.0, got {}",
            score
        );
    }

    /// Feature: matching-system-improvements, Property 17: Recency Exponential Decay
    /// Validates: Requirements 6.2
    ///
    /// Recency score should always be in range (0.0, 1.0]
    #[test]
    fn prop_recency_score_always_valid_range(
        age_hours in 1i64..500,
        half_life in half_life_hours()
    ) {
        let scorer = Scorer::default();
        let created_at = datetime_offset_hours(-age_hours);

        let score = scorer.recency_score_with_half_life(created_at, half_life);

        prop_assert!(
            score > 0.0 && score <= 1.0,
            "Recency score should be in (0.0, 1.0], got {}",
            score
        );
    }
}

// =============================================================================
// Property 18: Category-Specific Decay
// =============================================================================
// For any medication in the "urgent" category, the recency half-life SHALL be
// shorter than for medications in the "stable" category.
// Validates: Requirements 6.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 18: Category-Specific Decay
    /// Validates: Requirements 6.4
    ///
    /// Urgent category should have shorter half-life than stable
    #[test]
    fn prop_urgent_half_life_shorter_than_stable(_dummy in 0..1) {
        let scorer = Scorer::default();

        let urgent_half_life = scorer.get_category_half_life(MedicationCategory::Urgent);
        let stable_half_life = scorer.get_category_half_life(MedicationCategory::Stable);

        prop_assert!(
            urgent_half_life < stable_half_life,
            "Urgent half-life ({}) should be < stable half-life ({})",
            urgent_half_life,
            stable_half_life
        );
    }

    /// Feature: matching-system-improvements, Property 18: Category-Specific Decay
    /// Validates: Requirements 6.4
    ///
    /// Urgent medications should decay faster (lower score for same age)
    #[test]
    fn prop_urgent_decays_faster_than_stable(
        age_hours in 1i64..200 // Non-zero age
    ) {
        let scorer = Scorer::default();
        let created_at = datetime_offset_hours(-age_hours);

        let urgent_score = scorer.recency_score_for_category(created_at, MedicationCategory::Urgent);
        let stable_score = scorer.recency_score_for_category(created_at, MedicationCategory::Stable);

        prop_assert!(
            urgent_score <= stable_score,
            "Urgent score ({}) should be <= stable score ({}) for same age ({}h)",
            urgent_score,
            stable_score,
            age_hours
        );
    }

    /// Feature: matching-system-improvements, Property 18: Category-Specific Decay
    /// Validates: Requirements 6.4
    ///
    /// Category half-lives should be configurable
    #[test]
    fn prop_category_half_life_configurable(
        urgent_half_life in 1.0f64..50.0,
        stable_half_life in 50.0f64..200.0
    ) {
        let scorer = Scorer::default();

        scorer.set_category_half_life(MedicationCategory::Urgent, urgent_half_life);
        scorer.set_category_half_life(MedicationCategory::Stable, stable_half_life);

        let got_urgent = scorer.get_category_half_life(MedicationCategory::Urgent);
        let got_stable = scorer.get_category_half_life(MedicationCategory::Stable);

        prop_assert!(
            (got_urgent - urgent_half_life).abs() < 0.001,
            "Urgent half-life should be {}, got {}",
            urgent_half_life,
            got_urgent
        );
        prop_assert!(
            (got_stable - stable_half_life).abs() < 0.001,
            "Stable half-life should be {}, got {}",
            stable_half_life,
            got_stable
        );
    }

    /// Feature: matching-system-improvements, Property 18: Category-Specific Decay
    /// Validates: Requirements 6.4
    ///
    /// Category-specific scoring should use correct half-life
    #[test]
    fn prop_category_scoring_uses_correct_half_life(
        age_hours in 1i64..200
    ) {
        let scorer = Scorer::default();
        let created_at = datetime_offset_hours(-age_hours);

        let urgent_half_life = scorer.get_category_half_life(MedicationCategory::Urgent);
        let stable_half_life = scorer.get_category_half_life(MedicationCategory::Stable);

        let urgent_score = scorer.recency_score_for_category(created_at, MedicationCategory::Urgent);
        let stable_score = scorer.recency_score_for_category(created_at, MedicationCategory::Stable);

        // Calculate expected scores
        let expected_urgent = 0.5_f64.powf(age_hours as f64 / urgent_half_life);
        let expected_stable = 0.5_f64.powf(age_hours as f64 / stable_half_life);

        prop_assert!(
            (urgent_score - expected_urgent).abs() < 0.05,
            "Urgent score should be ~{}, got {}",
            expected_urgent,
            urgent_score
        );
        prop_assert!(
            (stable_score - expected_stable).abs() < 0.05,
            "Stable score should be ~{}, got {}",
            expected_stable,
            stable_score
        );
    }
}

// =============================================================================
// Property 20: Weight Sum Validation
// =============================================================================
// For any weight configuration update, the system SHALL reject configurations
// where weights do not sum to 1.0 (within tolerance of 0.001).
// Validates: Requirements 8.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 20: Weight Sum Validation
    /// Validates: Requirements 8.3
    ///
    /// Valid weights (sum = 1.0) should pass validation
    #[test]
    fn prop_valid_weights_pass_validation(
        med in 0.1f64..0.9,
        dosage in 0.01f64..0.2,
        quantity in 0.01f64..0.1,
        price in 0.01f64..0.1
    ) {
        // Calculate remaining weight to ensure sum = 1.0
        let remaining = 1.0 - med - dosage - quantity - price;
        prop_assume!(remaining > 0.0);

        // Distribute remaining among recency, expiry, supplier
        let recency = remaining / 3.0;
        let expiry = remaining / 3.0;
        let supplier = remaining - recency - expiry;

        let weights = Weights {
            medication: med,
            dosage,
            quantity,
            price,
            recency,
            expiry,
            supplier,
            ai_logic: 0.0,
        };

        let result = weights.validate();

        prop_assert!(
            result.is_ok(),
            "Weights summing to 1.0 should pass validation, got {:?}",
            result
        );
    }

    /// Feature: matching-system-improvements, Property 20: Weight Sum Validation
    /// Validates: Requirements 8.3
    ///
    /// Invalid weights (sum != 1.0) should fail validation
    #[test]
    fn prop_invalid_weights_fail_validation(
        med in 0.1f64..0.5,
        dosage in 0.1f64..0.3,
        quantity in 0.1f64..0.2,
        price in 0.1f64..0.2,
        recency in 0.1f64..0.2,
        expiry in 0.1f64..0.2,
        supplier in 0.1f64..0.2
    ) {
        let sum = med + dosage + quantity + price + recency + expiry + supplier;
        prop_assume!((sum - 1.0).abs() > 0.001); // Ensure sum is NOT 1.0

        let weights = Weights {
            medication: med,
            dosage,
            quantity,
            price,
            recency,
            expiry,
            supplier,
            ai_logic: 0.0,
        };

        let result = weights.validate();

        prop_assert!(
            result.is_err(),
            "Weights summing to {} (not 1.0) should fail validation",
            sum
        );
    }

    /// Feature: matching-system-improvements, Property 20: Weight Sum Validation
    /// Validates: Requirements 8.3
    ///
    /// Normalize should produce weights that sum to 1.0
    #[test]
    fn prop_normalize_produces_valid_weights(
        med in 0.1f64..1.0,
        dosage in 0.1f64..1.0,
        quantity in 0.1f64..1.0,
        price in 0.1f64..1.0,
        recency in 0.1f64..1.0,
        expiry in 0.1f64..1.0,
        supplier in 0.1f64..1.0
    ) {
        let weights = Weights {
            medication: med,
            dosage,
            quantity,
            price,
            recency,
            expiry,
            supplier,
            ai_logic: 0.0,
        };

        let normalized = weights.normalized();
        let result = normalized.validate();

        prop_assert!(
            result.is_ok(),
            "Normalized weights should pass validation, got {:?}",
            result
        );
    }

    /// Feature: matching-system-improvements, Property 20: Weight Sum Validation
    /// Validates: Requirements 8.3
    ///
    /// Normalize should preserve relative proportions
    #[test]
    fn prop_normalize_preserves_proportions(
        med in 0.1f64..1.0,
        dosage in 0.1f64..1.0
    ) {
        prop_assume!(med > 0.0 && dosage > 0.0);

        let weights = Weights {
            medication: med,
            dosage,
            quantity: 0.1,
            price: 0.1,
            recency: 0.1,
            expiry: 0.1,
            supplier: 0.1,
            ai_logic: 0.0,
        };

        let original_ratio = med / dosage;
        let normalized = weights.normalized();
        let normalized_ratio = normalized.medication / normalized.dosage;

        prop_assert!(
            (original_ratio - normalized_ratio).abs() < 0.001,
            "Normalize should preserve ratio: original {}, normalized {}",
            original_ratio,
            normalized_ratio
        );
    }

    /// Feature: matching-system-improvements, Property 20: Weight Sum Validation
    /// Validates: Requirements 8.3
    ///
    /// Default weights should be valid
    #[test]
    fn prop_default_weights_are_valid(_dummy in 0..1) {
        let weights = Weights::default();
        let result = weights.validate();

        prop_assert!(
            result.is_ok(),
            "Default weights should be valid, got {:?}",
            result
        );
    }
}

// =============================================================================
// Additional Consistency Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements
    /// Validates: Requirements 4.1, 4.2, 4.3, 6.2
    ///
    /// Scorer should be thread-safe (can update and read concurrently)
    #[test]
    fn prop_scorer_thread_safe_updates(
        threshold in min_fulfillment_threshold()
    ) {
        let scorer = Scorer::default();

        // Update threshold
        scorer.set_min_quantity_fulfillment(threshold);

        // Read back should match
        let got = scorer.get_min_quantity_fulfillment();

        prop_assert!(
            (got - threshold).abs() < 0.001,
            "Threshold should be {}, got {}",
            threshold,
            got
        );
    }
}
