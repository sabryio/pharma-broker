//! Phase 8: Parsing Integration Tests
//!
//! Tests for batch processing configuration.
//! See: docs/phases/08-parsing.md

use std::time::Duration;

use pharma_core::parsing::{BatchConfig, MultiPassConfig};

/// Test default batch config values
#[test]
fn test_batch_config_defaults() {
    let config = BatchConfig::default();

    assert_eq!(config.batch_size, 10, "Default batch size should be 10");
    assert_eq!(
        config.batch_timeout,
        Duration::from_secs(5),
        "Default timeout should be 5s"
    );
    assert_eq!(config.worker_count, 2, "Default worker count should be 2");
    assert_eq!(
        config.channel_buffer, 100,
        "Default channel buffer should be 100"
    );
}

/// Test default multi-pass config values
#[test]
fn test_multipass_config_defaults() {
    let config = MultiPassConfig::default();

    assert!(
        (config.strict_min_confidence - 0.70).abs() < 0.01,
        "Default strict threshold should be 0.70"
    );
    assert!(
        (config.relaxed_min_confidence - 0.40).abs() < 0.01,
        "Default relaxed threshold should be 0.40"
    );
    assert!(config.enable_pass2, "Pass 2 should be enabled by default");
    assert!(
        config.enable_review_queue,
        "Review queue should be enabled by default"
    );
}

/// Test multi-pass config needs_pass2 logic
#[test]
fn test_multipass_needs_pass2() {
    let config = MultiPassConfig::default();

    // Above strict threshold -> no pass 2 needed
    assert!(
        !config.needs_pass2(0.80),
        "Should not need pass 2 above strict threshold"
    );

    // Below strict threshold -> needs pass 2
    assert!(
        config.needs_pass2(0.60),
        "Should need pass 2 below strict threshold"
    );

    // Below relaxed threshold -> still needs pass 2
    assert!(
        config.needs_pass2(0.30),
        "Should need pass 2 below relaxed threshold"
    );
}

/// Test multi-pass config needs_review logic
#[test]
fn test_multipass_needs_review() {
    let config = MultiPassConfig::default();

    // Above relaxed threshold -> no review needed
    assert!(
        !config.needs_review(0.50),
        "Should not need review above relaxed threshold"
    );

    // Below relaxed threshold -> needs review
    assert!(
        config.needs_review(0.35),
        "Should need review below relaxed threshold"
    );
}

/// Test multi-pass config with review disabled
#[test]
fn test_multipass_review_disabled() {
    let config = MultiPassConfig {
        enable_review_queue: false,
        ..Default::default()
    };

    // Even below threshold, should not need review when disabled
    assert!(
        !config.needs_review(0.20),
        "Should not need review when queue disabled"
    );
}
