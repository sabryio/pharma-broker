//! Property-based tests for ExpiryScorer functionality
//!
//! Feature: matching-system-improvements
//! Tests Properties 13, 14, 15, and 16 from the design document
//!
//! These tests validate:
//! - Property 13: Expired Offer Exclusion
//! - Property 14: Near-Expiry Decay
//! - Property 15: Shelf Life Filtering
//! - Property 16: Missing Expiry Handling
//!
//! Run with: cargo test --features test-expiry-props --test expiry_properties

#![cfg(feature = "test-expiry-props")]

use chrono::{DateTime, Duration, TimeZone, Utc};
use pharma_core::matching::{ExpiryConfig, ExpiryScorer, ExpiryWarning};
use proptest::prelude::*;

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate a fixed "now" time for deterministic tests
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap()
}

/// Generate a DateTime offset from a base time by a number of days
fn datetime_offset(base: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    base + Duration::days(days)
}

/// Strategy for generating days in the past (expired)
fn days_in_past() -> impl Strategy<Value = i64> {
    1i64..365 // 1 to 365 days in the past
}

/// Strategy for generating days in the future (not expired)
fn days_in_future() -> impl Strategy<Value = i64> {
    1i64..365 // 1 to 365 days in the future
}

/// Strategy for generating days within decay period (1-30 days)
fn days_within_decay() -> impl Strategy<Value = i64> {
    1i64..=30 // 1 to 30 days (within default decay period)
}

/// Strategy for generating days beyond decay period (31+ days)
fn days_beyond_decay() -> impl Strategy<Value = i64> {
    31i64..365 // 31 to 365 days (beyond default decay period)
}

/// Strategy for generating minimum shelf life requirements
fn shelf_life_days() -> impl Strategy<Value = u32> {
    1u32..180 // 1 to 180 days shelf life requirement
}

/// Strategy for generating decay_start_days config values
fn decay_start_days_config() -> impl Strategy<Value = u32> {
    7u32..90 // 7 to 90 days decay start
}

/// Strategy for generating missing_score config values
fn missing_score_config() -> impl Strategy<Value = f64> {
    0.0f64..=1.0 // 0.0 to 1.0 score
}

// =============================================================================
// Property 13: Expired Offer Exclusion
// =============================================================================
// For any offer with an expiry date in the past, the offer SHALL be excluded
// from matching results (not returned as a candidate).
// Validates: Requirements 5.1

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 13: Expired Offer Exclusion
    /// Validates: Requirements 5.1
    ///
    /// For any offer with an expiry date in the past, the score SHALL be 0.0
    /// and is_expired SHALL be true
    #[test]
    fn prop_expired_offer_score_zero(
        days_past in days_in_past()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, -days_past);

        let result = scorer.score(Some(expiry), now);

        prop_assert!(
            (result.score - 0.0).abs() < 0.001,
            "Expired offer should have score 0.0, got {}",
            result.score
        );
        prop_assert!(
            result.is_expired,
            "Expired offer should have is_expired = true"
        );
        prop_assert_eq!(
            result.warning,
            Some(ExpiryWarning::Expired),
            "Expired offer should have Expired warning"
        );
    }

    /// Feature: matching-system-improvements, Property 13: Expired Offer Exclusion
    /// Validates: Requirements 5.1
    ///
    /// is_expired() should return true for any past expiry date
    #[test]
    fn prop_is_expired_true_for_past_dates(
        days_past in days_in_past()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, -days_past);

        prop_assert!(
            scorer.is_expired(Some(expiry), now),
            "is_expired should return true for expiry {} days in the past",
            days_past
        );
    }

    /// Feature: matching-system-improvements, Property 13: Expired Offer Exclusion
    /// Validates: Requirements 5.1
    ///
    /// is_expired() should return false for any future expiry date
    #[test]
    fn prop_is_expired_false_for_future_dates(
        days_future in days_in_future()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, days_future);

        prop_assert!(
            !scorer.is_expired(Some(expiry), now),
            "is_expired should return false for expiry {} days in the future",
            days_future
        );
    }
}

// =============================================================================
// Property 14: Near-Expiry Decay
// =============================================================================
// For any offer expiring within 30 days, the expiry_score SHALL be less than 1.0
// and decrease as expiry approaches.
// Validates: Requirements 5.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 14: Near-Expiry Decay
    /// Validates: Requirements 5.2
    ///
    /// For any offer expiring within decay period, score SHALL be less than 1.0
    #[test]
    fn prop_near_expiry_score_less_than_one(
        days_remaining in days_within_decay()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, days_remaining);

        let result = scorer.score(Some(expiry), now);

        // Score should be days_remaining / 30 (linear decay)
        let expected_score = days_remaining as f64 / 30.0;

        prop_assert!(
            result.score <= 1.0,
            "Near-expiry score should be <= 1.0, got {}",
            result.score
        );
        prop_assert!(
            (result.score - expected_score).abs() < 0.001,
            "Near-expiry score should be {}, got {}",
            expected_score,
            result.score
        );
        prop_assert!(
            matches!(result.warning, Some(ExpiryWarning::NearExpiry { .. })),
            "Near-expiry offer should have NearExpiry warning"
        );
    }

    /// Feature: matching-system-improvements, Property 14: Near-Expiry Decay
    /// Validates: Requirements 5.2
    ///
    /// Score should decrease monotonically as expiry approaches
    #[test]
    fn prop_near_expiry_score_decreases_monotonically(
        days1 in days_within_decay(),
        days2 in days_within_decay()
    ) {
        prop_assume!(days1 != days2);

        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        let expiry1 = datetime_offset(now, days1);
        let expiry2 = datetime_offset(now, days2);

        let result1 = scorer.score(Some(expiry1), now);
        let result2 = scorer.score(Some(expiry2), now);

        // If days1 > days2, then score1 should be > score2
        if days1 > days2 {
            prop_assert!(
                result1.score > result2.score,
                "Score for {} days ({}) should be > score for {} days ({})",
                days1, result1.score, days2, result2.score
            );
        } else {
            prop_assert!(
                result1.score < result2.score,
                "Score for {} days ({}) should be < score for {} days ({})",
                days1, result1.score, days2, result2.score
            );
        }
    }

    /// Feature: matching-system-improvements, Property 14: Near-Expiry Decay
    /// Validates: Requirements 5.2
    ///
    /// For any offer expiring beyond decay period, score SHALL be 1.0
    #[test]
    fn prop_beyond_decay_score_is_one(
        days_remaining in days_beyond_decay()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, days_remaining);

        let result = scorer.score(Some(expiry), now);

        prop_assert!(
            (result.score - 1.0).abs() < 0.001,
            "Beyond-decay score should be 1.0, got {}",
            result.score
        );
        prop_assert!(
            result.warning.is_none(),
            "Beyond-decay offer should have no warning"
        );
    }

    /// Feature: matching-system-improvements, Property 14: Near-Expiry Decay
    /// Validates: Requirements 5.2
    ///
    /// Decay should respect custom decay_start_days configuration
    #[test]
    fn prop_custom_decay_start_days(
        decay_days in decay_start_days_config(),
        days_remaining in 1i64..100
    ) {
        let config = ExpiryConfig {
            decay_start_days: decay_days,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();
        let expiry = datetime_offset(now, days_remaining);

        let result = scorer.score(Some(expiry), now);

        if days_remaining <= decay_days as i64 {
            // Within decay period - score should be proportional
            let expected_score = days_remaining as f64 / decay_days as f64;
            prop_assert!(
                (result.score - expected_score).abs() < 0.001,
                "Within custom decay period, score should be {}, got {}",
                expected_score,
                result.score
            );
        } else {
            // Beyond decay period - score should be 1.0
            prop_assert!(
                (result.score - 1.0).abs() < 0.001,
                "Beyond custom decay period, score should be 1.0, got {}",
                result.score
            );
        }
    }
}

// =============================================================================
// Property 15: Shelf Life Filtering
// =============================================================================
// For any request specifying a minimum shelf life of N days, all matched offers
// SHALL have expiry dates at least N days in the future.
// Validates: Requirements 5.3

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 15: Shelf Life Filtering
    /// Validates: Requirements 5.3
    ///
    /// meets_shelf_life should return true when days_remaining >= min_days
    #[test]
    fn prop_meets_shelf_life_sufficient(
        min_days in shelf_life_days(),
        extra_days in 0u32..180
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let days_remaining = min_days as i64 + extra_days as i64;
        let expiry = datetime_offset(now, days_remaining);

        prop_assert!(
            scorer.meets_shelf_life_at(Some(expiry), min_days, now),
            "Should meet shelf life when {} days remaining >= {} required",
            days_remaining,
            min_days
        );
    }

    /// Feature: matching-system-improvements, Property 15: Shelf Life Filtering
    /// Validates: Requirements 5.3
    ///
    /// meets_shelf_life should return false when days_remaining < min_days
    #[test]
    fn prop_meets_shelf_life_insufficient(
        min_days in 2u32..180,
        days_short in 1u32..180
    ) {
        prop_assume!(days_short < min_days);

        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let days_remaining = (min_days - days_short) as i64;

        // Ensure we have a positive number of days remaining
        prop_assume!(days_remaining > 0);

        let expiry = datetime_offset(now, days_remaining);

        prop_assert!(
            !scorer.meets_shelf_life_at(Some(expiry), min_days, now),
            "Should NOT meet shelf life when {} days remaining < {} required",
            days_remaining,
            min_days
        );
    }

    /// Feature: matching-system-improvements, Property 15: Shelf Life Filtering
    /// Validates: Requirements 5.3
    ///
    /// meets_shelf_life should return false for expired offers
    #[test]
    fn prop_meets_shelf_life_expired(
        min_days in shelf_life_days(),
        days_past in days_in_past()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, -days_past);

        prop_assert!(
            !scorer.meets_shelf_life_at(Some(expiry), min_days, now),
            "Expired offer should NOT meet any shelf life requirement"
        );
    }

    /// Feature: matching-system-improvements, Property 15: Shelf Life Filtering
    /// Validates: Requirements 5.3
    ///
    /// meets_shelf_life should return false for missing expiry dates
    #[test]
    fn prop_meets_shelf_life_missing(
        min_days in shelf_life_days()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        prop_assert!(
            !scorer.meets_shelf_life_at(None, min_days, now),
            "Missing expiry should NOT meet shelf life requirement"
        );
    }

    /// Feature: matching-system-improvements, Property 15: Shelf Life Filtering
    /// Validates: Requirements 5.3
    ///
    /// meets_shelf_life should return true at exact boundary
    #[test]
    fn prop_meets_shelf_life_exact_boundary(
        min_days in shelf_life_days()
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, min_days as i64);

        prop_assert!(
            scorer.meets_shelf_life_at(Some(expiry), min_days, now),
            "Should meet shelf life at exact boundary ({} days)",
            min_days
        );
    }
}

// =============================================================================
// Property 16: Missing Expiry Handling
// =============================================================================
// For any offer without an expiry date, the expiry_score SHALL be 0.5 and
// the result SHALL contain an ExpiryWarning flag.
// Validates: Requirements 5.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 16: Missing Expiry Handling
    /// Validates: Requirements 5.5
    ///
    /// Missing expiry should return configured missing_score (default 0.5)
    #[test]
    fn prop_missing_expiry_default_score(_dummy in 0..1) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        let result = scorer.score(None, now);

        prop_assert!(
            (result.score - 0.5).abs() < 0.001,
            "Missing expiry should have score 0.5, got {}",
            result.score
        );
        prop_assert!(
            !result.is_expired,
            "Missing expiry should not be marked as expired"
        );
        prop_assert!(
            result.days_until_expiry.is_none(),
            "Missing expiry should have None for days_until_expiry"
        );
        prop_assert_eq!(
            result.warning,
            Some(ExpiryWarning::MissingExpiry),
            "Missing expiry should have MissingExpiry warning"
        );
    }

    /// Feature: matching-system-improvements, Property 16: Missing Expiry Handling
    /// Validates: Requirements 5.5
    ///
    /// Missing expiry should respect custom missing_score configuration
    #[test]
    fn prop_missing_expiry_custom_score(
        missing_score in missing_score_config()
    ) {
        let config = ExpiryConfig {
            missing_score,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();

        let result = scorer.score(None, now);

        prop_assert!(
            (result.score - missing_score).abs() < 0.001,
            "Missing expiry should have configured score {}, got {}",
            missing_score,
            result.score
        );
    }

    /// Feature: matching-system-improvements, Property 16: Missing Expiry Handling
    /// Validates: Requirements 5.5
    ///
    /// is_expired should return false for missing expiry dates
    #[test]
    fn prop_missing_expiry_not_expired(_dummy in 0..1) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();

        prop_assert!(
            !scorer.is_expired(None, now),
            "Missing expiry should not be considered expired"
        );
    }
}

// =============================================================================
// Additional Property Tests for Consistency
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements
    /// Validates: Requirements 5.1, 5.2
    ///
    /// Score should always be in range [0.0, 1.0]
    #[test]
    fn prop_score_always_in_valid_range(
        days_offset in -365i64..365
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, days_offset);

        let result = scorer.score(Some(expiry), now);

        prop_assert!(
            result.score >= 0.0 && result.score <= 1.0,
            "Score should be in [0.0, 1.0], got {}",
            result.score
        );
    }

    /// Feature: matching-system-improvements
    /// Validates: Requirements 5.1, 5.2
    ///
    /// days_until_expiry should be consistent with is_expired
    #[test]
    fn prop_days_until_expiry_consistent_with_is_expired(
        days_offset in -365i64..365
    ) {
        let scorer = ExpiryScorer::default();
        let now = fixed_now();
        let expiry = datetime_offset(now, days_offset);

        let result = scorer.score(Some(expiry), now);

        if let Some(days) = result.days_until_expiry {
            if days < 0 {
                prop_assert!(
                    result.is_expired,
                    "Negative days_until_expiry ({}) should mean is_expired = true",
                    days
                );
            } else {
                prop_assert!(
                    !result.is_expired,
                    "Non-negative days_until_expiry ({}) should mean is_expired = false",
                    days
                );
            }
        }
    }

    /// Feature: matching-system-improvements
    /// Validates: Requirements 5.1, 5.2, 5.5
    ///
    /// Disabled scorer should always return score 1.0
    #[test]
    fn prop_disabled_scorer_always_returns_one(
        days_offset in -365i64..365
    ) {
        let config = ExpiryConfig {
            enabled: false,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();
        let expiry = datetime_offset(now, days_offset);

        let result = scorer.score(Some(expiry), now);

        prop_assert!(
            (result.score - 1.0).abs() < 0.001,
            "Disabled scorer should return 1.0, got {}",
            result.score
        );
    }

    /// Feature: matching-system-improvements
    /// Validates: Requirements 5.1, 5.2, 5.5
    ///
    /// Disabled scorer should return 1.0 even for missing expiry
    #[test]
    fn prop_disabled_scorer_missing_expiry(_dummy in 0..1) {
        let config = ExpiryConfig {
            enabled: false,
            ..Default::default()
        };
        let scorer = ExpiryScorer::new(config);
        let now = fixed_now();

        let result = scorer.score(None, now);

        prop_assert!(
            (result.score - 1.0).abs() < 0.001,
            "Disabled scorer should return 1.0 for missing expiry, got {}",
            result.score
        );
    }
}
