//! Phase 1: Observability Integration Tests
//!
//! Tests for metrics collection and health endpoints.
//! See: docs/phases/01-observability.md
//!
//! Run with: cargo test --features test-phase01 --test phase01_observability

#![cfg(feature = "test-phase01")]

use pharma_core::metrics;

/// Test that metrics can be initialized and recorded
#[test]
fn test_metrics_initialization() {
    // Initialize metrics (idempotent - may already be init from other tests)
    // In a real test, we'd use a fresh recorder per test

    // Record some metrics
    metrics::record_message_received("test-group", "success");
    metrics::record_offer_created();
    metrics::record_request_created();
    metrics::record_ai_parse("success");

    // If we get here without panic, metrics are working
}

/// Test counter metrics
#[test]
fn test_counter_metrics() {
    metrics::record_message_processed("offer", "success");
    metrics::record_db_operation("insert", "success");
    metrics::record_rate_limited("127.0.0.1");
    // No panic = success
}

/// Test histogram metrics
#[test]
fn test_histogram_metrics() {
    metrics::record_message_duration(0.5);
    metrics::record_ai_parse_duration(1.2);
    metrics::record_db_query_duration(0.05);
    // No panic = success
}

/// Test gauge metrics
#[test]
fn test_gauge_metrics() {
    metrics::set_active_connections(5);
    metrics::set_queue_size(100);
    metrics::set_monitored_groups(3);
    metrics::set_ai_batch_pending(10);
    // No panic = success
}

/// Test AI-specific metrics
#[test]
fn test_ai_metrics() {
    metrics::increment_ai_batch_pending();
    metrics::decrement_ai_batch_pending();
    metrics::record_ai_parse_detailed("success", 5);
    metrics::record_ai_tokens_used(1500);
    // No panic = success
}

/// Test match quality metrics
#[test]
fn test_match_quality_metrics() {
    metrics::record_match_confirmation(true);
    metrics::record_match_confirmation(false);
    metrics::record_match_score(0.85, "high");
    metrics::set_confirmation_rate(0.75);
    // No panic = success
}
