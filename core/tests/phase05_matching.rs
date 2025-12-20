//! Phase 5: Matching Engine Integration Tests
//!
//! Tests for scorer, weights, and thresholds.
//! See: docs/phases/05-matching.md

use pharma_core::matching::{DecayType, MatchAction, Weights, cosine_similarity};

/// Test cosine similarity with identical vectors
#[test]
fn test_cosine_identical_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];

    let sim = cosine_similarity(&a, &b).unwrap();
    assert!(
        (sim - 1.0).abs() < 0.001,
        "Identical vectors should have similarity 1.0"
    );
}

/// Test cosine similarity with orthogonal vectors
#[test]
fn test_cosine_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];

    let sim = cosine_similarity(&a, &b).unwrap();
    assert!(
        sim.abs() < 0.001,
        "Orthogonal vectors should have similarity 0.0"
    );
}

/// Test cosine similarity with opposite vectors
#[test]
fn test_cosine_opposite_vectors() {
    let a = vec![1.0, 0.0];
    let b = vec![-1.0, 0.0];

    let sim = cosine_similarity(&a, &b).unwrap();
    assert!(
        (sim + 1.0).abs() < 0.001,
        "Opposite vectors should have similarity -1.0"
    );
}

/// Test cosine similarity rejects mismatched lengths
#[test]
fn test_cosine_length_mismatch() {
    let a = vec![1.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];

    let result = cosine_similarity(&a, &b);
    assert!(
        result.is_err(),
        "Should reject vectors of different lengths"
    );
}

/// Test default weights sum to 1.0
#[test]
fn test_weights_default_sum() {
    let weights = Weights::default();
    let sum =
        weights.medication + weights.dosage + weights.quantity + weights.price + weights.recency;

    assert!(
        (sum - 1.0).abs() < 0.01,
        "Default weights should sum to 1.0, got {}",
        sum
    );
}

/// Test match action variants
#[test]
fn test_match_action_variants() {
    // Test that all variants exist and can be compared
    let auto = MatchAction::AutoConfirm;
    let suggest = MatchAction::SuggestToOperator;
    let queue = MatchAction::QueueForReview;
    let ignore = MatchAction::Ignore;

    assert!(matches!(auto, MatchAction::AutoConfirm));
    assert!(matches!(suggest, MatchAction::SuggestToOperator));
    assert!(matches!(queue, MatchAction::QueueForReview));
    assert!(matches!(ignore, MatchAction::Ignore));
}

/// Test match action requires_human
#[test]
fn test_match_action_requires_human() {
    assert!(!MatchAction::AutoConfirm.requires_human());
    assert!(MatchAction::SuggestToOperator.requires_human());
    assert!(MatchAction::QueueForReview.requires_human());
    assert!(!MatchAction::Ignore.requires_human());
}

/// Test match action is_automatic
#[test]
fn test_match_action_is_automatic() {
    assert!(MatchAction::AutoConfirm.is_automatic());
    assert!(!MatchAction::SuggestToOperator.is_automatic());
    assert!(!MatchAction::QueueForReview.is_automatic());
    assert!(MatchAction::Ignore.is_automatic());
}

/// Test decay type variants
#[test]
fn test_decay_type_default() {
    let decay = DecayType::default();
    assert!(matches!(decay, DecayType::Exponential));
}
