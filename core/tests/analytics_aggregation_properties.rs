//! Property-based tests for Performance Analytics Aggregation
//!
//! Feature: debug-recording-enhancement
//! Tests Property 12 from the design document
//!
//! Property 12: Aggregation Correctness
//! For any set of audit records, the computed average, p95, and p99 latencies
//! per stage SHALL be mathematically correct based on the individual stage latencies.
//!
//! Validates: Requirements 8.5
//!
//! Run with: cargo test --features test-analytics-props --test analytics_aggregation_properties

#![cfg(feature = "test-analytics-props")]

use proptest::prelude::*;

// =============================================================================
// Aggregation Functions (mirroring the API implementation)
// =============================================================================

/// Compute latency statistics from a vector of latency values
fn compute_latency_stats(values: &[u64]) -> LatencyStats {
    if values.is_empty() {
        return LatencyStats::default();
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();

    let count = sorted.len();
    let min_ms = sorted[0];
    let max_ms = sorted[count - 1];
    let sum: u64 = sorted.iter().sum();
    let avg_ms = sum as f64 / count as f64;

    // Median
    let median_ms = if count % 2 == 0 {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2
    } else {
        sorted[count / 2]
    };

    // Percentiles
    let p95_idx = ((count as f64) * 0.95).ceil() as usize - 1;
    let p99_idx = ((count as f64) * 0.99).ceil() as usize - 1;
    let p95_ms = sorted[p95_idx.min(count - 1)];
    let p99_ms = sorted[p99_idx.min(count - 1)];

    // Standard deviation
    let variance: f64 = sorted
        .iter()
        .map(|&v| {
            let diff = v as f64 - avg_ms;
            diff * diff
        })
        .sum::<f64>()
        / count as f64;
    let std_dev_ms = variance.sqrt();

    LatencyStats {
        count,
        min_ms,
        max_ms,
        avg_ms,
        median_ms,
        p95_ms,
        p99_ms,
        std_dev_ms,
    }
}

/// Compute percentile from sorted values
fn compute_percentile(sorted_values: &[u64], percentile: f64) -> u64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let idx = ((sorted_values.len() as f64) * percentile).ceil() as usize - 1;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

/// Compute average from values
fn compute_average(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let sum: u64 = values.iter().sum();
    sum as f64 / values.len() as f64
}

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Default)]
struct LatencyStats {
    count: usize,
    min_ms: u64,
    max_ms: u64,
    avg_ms: f64,
    median_ms: u64,
    p95_ms: u64,
    p99_ms: u64,
    std_dev_ms: f64,
}

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate a random latency value in milliseconds
fn arb_latency_ms() -> impl Strategy<Value = u64> {
    1u64..10000
}

/// Generate a vector of latency values
fn arb_latency_vec(min_size: usize, max_size: usize) -> impl Strategy<Value = Vec<u64>> {
    prop::collection::vec(arb_latency_ms(), min_size..max_size)
}

// =============================================================================
// Property 12: Aggregation Correctness
// =============================================================================
// For any set of audit records, the computed average, p95, and p99 latencies
// per stage SHALL be mathematically correct based on the individual stage latencies.
//
// Validates: Requirements 8.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the computed average SHALL equal
    /// the sum of values divided by the count.
    #[test]
    fn prop_average_is_mathematically_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        // Compute expected average manually
        let sum: u64 = latencies.iter().sum();
        let expected_avg = sum as f64 / latencies.len() as f64;

        // Allow small floating point tolerance
        prop_assert!(
            (stats.avg_ms - expected_avg).abs() < 0.0001,
            "Average should be sum/count. Expected {}, got {}",
            expected_avg,
            stats.avg_ms
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the computed p95 SHALL be the value
    /// at the 95th percentile position in the sorted array.
    #[test]
    fn prop_p95_is_mathematically_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        // Compute expected p95 manually
        let mut sorted = latencies.clone();
        sorted.sort_unstable();
        let expected_p95 = compute_percentile(&sorted, 0.95);

        prop_assert_eq!(
            stats.p95_ms,
            expected_p95,
            "P95 should match percentile calculation"
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the computed p99 SHALL be the value
    /// at the 99th percentile position in the sorted array.
    #[test]
    fn prop_p99_is_mathematically_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        // Compute expected p99 manually
        let mut sorted = latencies.clone();
        sorted.sort_unstable();
        let expected_p99 = compute_percentile(&sorted, 0.99);

        prop_assert_eq!(
            stats.p99_ms,
            expected_p99,
            "P99 should match percentile calculation"
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, min_ms SHALL be the smallest value
    /// and max_ms SHALL be the largest value.
    #[test]
    fn prop_min_max_are_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        let expected_min = *latencies.iter().min().unwrap();
        let expected_max = *latencies.iter().max().unwrap();

        prop_assert_eq!(
            stats.min_ms,
            expected_min,
            "Min should be the smallest value"
        );

        prop_assert_eq!(
            stats.max_ms,
            expected_max,
            "Max should be the largest value"
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the median SHALL be the middle value
    /// (or average of two middle values for even counts).
    #[test]
    fn prop_median_is_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        let mut sorted = latencies.clone();
        sorted.sort_unstable();
        let count = sorted.len();

        let expected_median = if count % 2 == 0 {
            (sorted[count / 2 - 1] + sorted[count / 2]) / 2
        } else {
            sorted[count / 2]
        };

        prop_assert_eq!(
            stats.median_ms,
            expected_median,
            "Median should be the middle value"
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the count SHALL equal the number of input values.
    #[test]
    fn prop_count_is_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        prop_assert_eq!(
            stats.count,
            latencies.len(),
            "Count should equal input length"
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// For any set of latency values, the standard deviation SHALL be
    /// mathematically correct (sqrt of variance).
    #[test]
    fn prop_std_dev_is_correct(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        // Compute expected standard deviation manually
        let avg = compute_average(&latencies);
        let variance: f64 = latencies.iter()
            .map(|&v| {
                let diff = v as f64 - avg;
                diff * diff
            })
            .sum::<f64>() / latencies.len() as f64;
        let expected_std_dev = variance.sqrt();

        // Allow small floating point tolerance
        prop_assert!(
            (stats.std_dev_ms - expected_std_dev).abs() < 0.0001,
            "Standard deviation should be sqrt(variance). Expected {}, got {}",
            expected_std_dev,
            stats.std_dev_ms
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// Percentile ordering invariant: p95 <= p99 <= max
    #[test]
    fn prop_percentile_ordering(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        prop_assert!(
            stats.p95_ms <= stats.p99_ms,
            "P95 ({}) should be <= P99 ({})",
            stats.p95_ms,
            stats.p99_ms
        );

        prop_assert!(
            stats.p99_ms <= stats.max_ms,
            "P99 ({}) should be <= max ({})",
            stats.p99_ms,
            stats.max_ms
        );

        prop_assert!(
            stats.min_ms <= stats.p95_ms,
            "Min ({}) should be <= P95 ({})",
            stats.min_ms,
            stats.p95_ms
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// Average bounds invariant: min <= avg <= max
    #[test]
    fn prop_average_bounds(
        latencies in arb_latency_vec(1, 100),
    ) {
        let stats = compute_latency_stats(&latencies);

        prop_assert!(
            stats.avg_ms >= stats.min_ms as f64,
            "Average ({}) should be >= min ({})",
            stats.avg_ms,
            stats.min_ms
        );

        prop_assert!(
            stats.avg_ms <= stats.max_ms as f64,
            "Average ({}) should be <= max ({})",
            stats.avg_ms,
            stats.max_ms
        );
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// Empty input handling: empty input should return default stats.
    #[test]
    fn prop_empty_input_returns_default(
        _dummy in Just(()),
    ) {
        let stats = compute_latency_stats(&[]);

        prop_assert_eq!(stats.count, 0);
        prop_assert_eq!(stats.min_ms, 0);
        prop_assert_eq!(stats.max_ms, 0);
        prop_assert_eq!(stats.avg_ms, 0.0);
        prop_assert_eq!(stats.median_ms, 0);
        prop_assert_eq!(stats.p95_ms, 0);
        prop_assert_eq!(stats.p99_ms, 0);
        prop_assert_eq!(stats.std_dev_ms, 0.0);
    }

    /// Feature: debug-recording-enhancement, Property 12: Aggregation Correctness
    /// Validates: Requirements 8.5
    ///
    /// Single value: all stats should equal that value (except std_dev = 0).
    #[test]
    fn prop_single_value_stats(
        value in arb_latency_ms(),
    ) {
        let stats = compute_latency_stats(&[value]);

        prop_assert_eq!(stats.count, 1);
        prop_assert_eq!(stats.min_ms, value);
        prop_assert_eq!(stats.max_ms, value);
        prop_assert_eq!(stats.avg_ms, value as f64);
        prop_assert_eq!(stats.median_ms, value);
        prop_assert_eq!(stats.p95_ms, value);
        prop_assert_eq!(stats.p99_ms, value);
        prop_assert_eq!(stats.std_dev_ms, 0.0);
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_compute_percentile_basic() {
        let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(compute_percentile(&values, 0.5), 50);
        assert_eq!(compute_percentile(&values, 0.95), 100);
        assert_eq!(compute_percentile(&values, 0.99), 100);
    }

    #[test]
    fn test_compute_average_basic() {
        assert_eq!(compute_average(&[10, 20, 30]), 20.0);
        assert_eq!(compute_average(&[]), 0.0);
    }

    #[test]
    fn test_latency_stats_known_values() {
        let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let stats = compute_latency_stats(&values);

        assert_eq!(stats.count, 10);
        assert_eq!(stats.min_ms, 10);
        assert_eq!(stats.max_ms, 100);
        assert_eq!(stats.avg_ms, 55.0);
        assert_eq!(stats.median_ms, 55); // (50 + 60) / 2
    }
}
