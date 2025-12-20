//! Phase 11: Background Jobs Integration Tests
//!
//! Tests for janitor cleanup worker.
//! See: docs/phases/11-background-jobs.md

use std::time::Duration;

use pharma_core::worker::janitor::{CleanupStats, JanitorConfig};

/// Test JanitorConfig defaults
#[test]
fn test_janitor_config_defaults() {
    let config = JanitorConfig::default();

    assert_eq!(
        config.interval,
        Duration::from_secs(3600),
        "Default interval should be 1 hour"
    );
    assert_eq!(
        config.raw_message_retention_days, 30,
        "Default raw message retention should be 30 days"
    );
    assert_eq!(
        config.offer_retention_days, 90,
        "Default offer retention should be 90 days"
    );
    assert_eq!(
        config.request_retention_days, 90,
        "Default request retention should be 90 days"
    );
    assert_eq!(
        config.match_retention_days, 365,
        "Default match retention should be 365 days"
    );
    assert_eq!(
        config.audit_log_retention_days, 365,
        "Default audit log retention should be 365 days"
    );
    assert!(config.enabled, "Janitor should be enabled by default");
}

/// Test CleanupStats default
#[test]
fn test_cleanup_stats_default() {
    let stats = CleanupStats::default();

    assert_eq!(stats.raw_messages_deleted, 0);
    assert_eq!(stats.offers_deleted, 0);
    assert_eq!(stats.requests_deleted, 0);
    assert_eq!(stats.matches_deleted, 0);
    assert_eq!(stats.audit_logs_deleted, 0);
    assert!(stats.last_run.is_none());
    assert_eq!(stats.run_count, 0);
}

/// Test retention days validation
#[test]
fn test_retention_days_positive() {
    let config = JanitorConfig::default();

    assert!(config.raw_message_retention_days > 0);
    assert!(config.offer_retention_days > 0);
    assert!(config.request_retention_days > 0);
    assert!(config.match_retention_days > 0);
    assert!(config.audit_log_retention_days > 0);
}

/// Test interval is reasonable
#[test]
fn test_interval_reasonable() {
    let config = JanitorConfig::default();

    // Should be at least 1 minute
    assert!(config.interval >= Duration::from_secs(60));
    // Should be at most 24 hours
    assert!(config.interval <= Duration::from_secs(86400));
}
