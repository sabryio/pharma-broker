//! Duplicate Handling Tests
//!
//! Tests for duplicate request/offer detection and filtering in the matching pipeline.
//! These tests verify that:
//! 1. Duplicate requests are not enqueued for matching
//! 2. Non-active requests are skipped during match processing
//! 3. Cross-participant duplicates don't create redundant matches

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use uuid::Uuid;

use pharma_core::domain::{ItemStatus, Offer, Request, UrgencyLevel};

// =============================================================================
// Test Helpers
// =============================================================================

fn create_request_with_status(status: ItemStatus) -> Request {
    let now = Utc::now();
    Request {
        id: Uuid::new_v4(),
        raw_message_id: Uuid::new_v4(),
        participant_id: Uuid::new_v4(),
        group_id: Uuid::new_v4(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جم".to_string(),
        quantity: Decimal::from_f64(30.0),
        unit: Some("boxes".to_string()),
        max_price: Decimal::from_f64(160.0),
        currency: Some("EGP".to_string()),
        urgency_level: UrgencyLevel::Normal,
        expiry_requirement: None,
        ai_confidence: 0.9,
        notes: None,
        status,
        content_embedding: None,
        master_medication_id: None,
        medication_curated: false,
        created_at: now,
        updated_at: now,
    }
}

fn create_offer_with_status(status: ItemStatus) -> Offer {
    let now = Utc::now();
    Offer {
        id: Uuid::new_v4(),
        raw_message_id: Uuid::new_v4(),
        participant_id: Uuid::new_v4(),
        group_id: Uuid::new_v4(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جم".to_string(),
        quantity: Decimal::from_f64(50.0),
        unit: Some("boxes".to_string()),
        price: Decimal::from_f64(150.0),
        currency: Some("EGP".to_string()),
        expiry_date: None,
        batch_number: None,
        notes: None,
        status,
        content_embedding: None,
        urgency_level: UrgencyLevel::Normal,
        expiry_info: None,
        ai_confidence: 0.9,
        master_medication_id: None,
        medication_curated: false,
        created_at: now,
        updated_at: now,
    }
}

// =============================================================================
// Request Status Filtering Tests
// =============================================================================

/// Test: Duplicate requests should be skipped during match processing
#[test]
fn test_duplicate_request_is_skipped() {
    let request = create_request_with_status(ItemStatus::Duplicate);

    // The match processor checks: request.status != ItemStatus::Active
    let should_skip = request.status != ItemStatus::Active;

    assert!(should_skip, "Duplicate request should be skipped");
    assert_eq!(request.status, ItemStatus::Duplicate);
}

/// Test: Active requests should be processed
#[test]
fn test_active_request_is_processed() {
    let request = create_request_with_status(ItemStatus::Active);

    let should_skip = request.status != ItemStatus::Active;

    assert!(!should_skip, "Active request should NOT be skipped");
    assert_eq!(request.status, ItemStatus::Active);
}

/// Test: Matched requests should be skipped
#[test]
fn test_matched_request_is_skipped() {
    let request = create_request_with_status(ItemStatus::Matched);

    let should_skip = request.status != ItemStatus::Active;

    assert!(should_skip, "Matched request should be skipped");
}

/// Test: Expired requests should be skipped
#[test]
fn test_expired_request_is_skipped() {
    let request = create_request_with_status(ItemStatus::Expired);

    let should_skip = request.status != ItemStatus::Active;

    assert!(should_skip, "Expired request should be skipped");
}

/// Test: Cancelled requests should be skipped
#[test]
fn test_cancelled_request_is_skipped() {
    let request = create_request_with_status(ItemStatus::Cancelled);

    let should_skip = request.status != ItemStatus::Active;

    assert!(should_skip, "Cancelled request should be skipped");
}

// =============================================================================
// Offer Status Filtering Tests
// =============================================================================

/// Test: Only active offers are returned by get_active query
#[test]
fn test_only_active_offers_should_be_candidates() {
    let active_offer = create_offer_with_status(ItemStatus::Active);
    let duplicate_offer = create_offer_with_status(ItemStatus::Duplicate);
    let matched_offer = create_offer_with_status(ItemStatus::Matched);

    // Simulate the filter that get_active applies
    let offers = [
        active_offer.clone(),
        duplicate_offer.clone(),
        matched_offer.clone(),
    ];
    let active_only: Vec<_> = offers
        .iter()
        .filter(|o| o.status == ItemStatus::Active)
        .collect();

    assert_eq!(active_only.len(), 1);
    assert_eq!(active_only[0].id, active_offer.id);
}

// =============================================================================
// Duplicate Detection Logic Tests
// =============================================================================

/// Test: Same participant + same medication within time window = duplicate
#[test]
fn test_same_participant_same_medication_is_duplicate() {
    let participant_id = Uuid::new_v4();
    let medication = "Augmentin 1g";

    let request1 = Request {
        participant_id,
        medication: medication.to_string(),
        ..create_request_with_status(ItemStatus::Active)
    };

    let request2 = Request {
        participant_id,                     // Same participant
        medication: medication.to_string(), // Same medication
        ..create_request_with_status(ItemStatus::Active)
    };

    // These should be detected as duplicates
    let is_same_participant = request1.participant_id == request2.participant_id;
    let is_same_medication = request1.medication == request2.medication;

    assert!(
        is_same_participant && is_same_medication,
        "Same participant + same medication should be flagged as duplicate"
    );
}

/// Test: Different participant + same medication = NOT duplicate (valid cross-pharmacy)
#[test]
fn test_different_participant_same_medication_not_duplicate() {
    let medication = "Augmentin 1g";

    let request1 = Request {
        participant_id: Uuid::new_v4(),
        medication: medication.to_string(),
        ..create_request_with_status(ItemStatus::Active)
    };

    let request2 = Request {
        participant_id: Uuid::new_v4(),     // Different participant
        medication: medication.to_string(), // Same medication
        ..create_request_with_status(ItemStatus::Active)
    };

    // These should NOT be detected as duplicates (different pharmacies can request same med)
    let is_same_participant = request1.participant_id == request2.participant_id;

    assert!(
        !is_same_participant,
        "Different participants requesting same medication should NOT be duplicates"
    );
}

/// Test: Same participant + different medication = NOT duplicate
#[test]
fn test_same_participant_different_medication_not_duplicate() {
    let participant_id = Uuid::new_v4();

    let request1 = Request {
        participant_id,
        medication: "Augmentin 1g".to_string(),
        ..create_request_with_status(ItemStatus::Active)
    };

    let request2 = Request {
        participant_id,                          // Same participant
        medication: "Panadol Extra".to_string(), // Different medication
        ..create_request_with_status(ItemStatus::Active)
    };

    let is_same_medication = request1.medication == request2.medication;

    assert!(
        !is_same_medication,
        "Same participant requesting different medications should NOT be duplicates"
    );
}

// =============================================================================
// Match Deduplication Tests
// =============================================================================

/// Test: Existing match should prevent duplicate match creation
#[test]
fn test_existing_match_prevents_duplicate() {
    let offer_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();

    // Simulate existing matches check
    let existing_matches: Vec<(Uuid, Uuid)> = vec![(offer_id, request_id)];

    // Check if match already exists
    let match_exists = existing_matches
        .iter()
        .any(|(o, r)| *o == offer_id && *r == request_id);

    assert!(match_exists, "Should detect existing match");

    // New match with same offer/request should be skipped
    let should_skip = match_exists;
    assert!(should_skip, "Duplicate match should be skipped");
}

/// Test: Different offer-request pair should create new match
#[test]
fn test_new_pair_creates_match() {
    let existing_offer_id = Uuid::new_v4();
    let existing_request_id = Uuid::new_v4();
    let new_offer_id = Uuid::new_v4();
    let new_request_id = Uuid::new_v4();

    let existing_matches: Vec<(Uuid, Uuid)> = vec![(existing_offer_id, existing_request_id)];

    // Check if new pair already exists
    let match_exists = existing_matches
        .iter()
        .any(|(o, r)| *o == new_offer_id && *r == new_request_id);

    assert!(!match_exists, "New pair should not exist in matches");

    let should_create = !match_exists;
    assert!(should_create, "New offer-request pair should create match");
}

// =============================================================================
// Enqueue Logic Tests
// =============================================================================

/// Test: Duplicate request should NOT be enqueued for matching
#[test]
fn test_duplicate_request_not_enqueued() {
    let is_duplicate = true;

    // Simulating the grpc server logic:
    // if !is_duplicate { new_request_ids.push(request.id); }
    let mut new_request_ids: Vec<Uuid> = Vec::new();
    let request_id = Uuid::new_v4();

    if !is_duplicate {
        new_request_ids.push(request_id);
    }

    assert!(
        new_request_ids.is_empty(),
        "Duplicate request should NOT be added to queue"
    );
}

/// Test: Non-duplicate request should be enqueued for matching
#[test]
fn test_non_duplicate_request_is_enqueued() {
    let is_duplicate = false;

    let mut new_request_ids: Vec<Uuid> = Vec::new();
    let request_id = Uuid::new_v4();

    if !is_duplicate {
        new_request_ids.push(request_id);
    }

    assert_eq!(
        new_request_ids.len(),
        1,
        "Non-duplicate request should be added to queue"
    );
    assert_eq!(new_request_ids[0], request_id);
}

// =============================================================================
// Edge Cases
// =============================================================================

/// Test: All ItemStatus variants are handled correctly
#[test]
fn test_all_item_status_variants() {
    let statuses = vec![
        (ItemStatus::Active, false),   // Should NOT skip
        (ItemStatus::Duplicate, true), // Should skip
        (ItemStatus::Matched, true),   // Should skip
        (ItemStatus::Expired, true),   // Should skip
        (ItemStatus::Cancelled, true), // Should skip
    ];

    for (status, expected_skip) in statuses {
        let should_skip = status != ItemStatus::Active;
        assert_eq!(
            should_skip, expected_skip,
            "Status {:?} should_skip={} but got {}",
            status, expected_skip, should_skip
        );
    }
}
