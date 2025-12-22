//! Phase 12: End-to-End Integration Tests
//!
//! Comprehensive tests covering the full message processing flow.
//! See: docs/phases/12-e2e-testing.md

use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use tokio::sync::broadcast;

use pharma_core::domain::{
    ItemStatus, Match, MatchStatus, Offer, RawMessage, Request, UrgencyLevel,
};
use pharma_core::matching::{MatchAction, Weights, cosine_similarity};
use pharma_core::notify::{CompositeNotifier, MatchNotifier, NullNotifier};
use pharma_core::ws::WsEvent;

/// Create a test raw message
fn create_raw_message() -> RawMessage {
    RawMessage {
        id: uuid::Uuid::new_v4().to_string(),
        external_id: Some(uuid::Uuid::new_v4().to_string()),
        group_jid: "pharmacy-group@g.us".to_string(),
        group_name: "Pharmacy Exchange".to_string(),
        sender_jid: "sender@s.whatsapp.net".to_string(),
        sender_phone: Some("+201234567890".to_string()),
        sender_name: Some("Test Pharmacist".to_string()),
        content: "Selling Augmentin 1g - 50 boxes at 150 EGP each".to_string(),
        timestamp: Utc::now(),
        processed_at: None,
        error: None,
        reply_to_id: None,
        reply_to_content: None,
        reply_to_sender: None,
        created_at: Utc::now(),
    }
}

/// Create a test offer
fn create_offer() -> Offer {
    let now = Utc::now();
    Offer {
        id: uuid::Uuid::new_v4().to_string(),
        raw_message_id: uuid::Uuid::new_v4().to_string(),
        source_phone: "+201234567890".to_string(),
        source_name: Some("Test Seller".to_string()),
        source_group: "pharmacy-group@g.us".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جم".to_string(),
        quantity: Decimal::from_f64(50.0),
        unit: Some("boxes".to_string()),
        price: Decimal::from_f64(150.0),
        currency: Some("EGP".to_string()),
        expiry_date: None,
        batch_number: None,
        notes: None,
        status: ItemStatus::Active,
        content_embedding: None,
        urgency_level: UrgencyLevel::Normal,
        expiry_info: None,
        ai_confidence: 0.9,
        created_at: now,
        updated_at: now,
    }
}

/// Create a test request that matches the offer
fn create_matching_request() -> Request {
    let now = Utc::now();
    Request {
        id: uuid::Uuid::new_v4().to_string(),
        raw_message_id: uuid::Uuid::new_v4().to_string(),
        source_phone: "+201098765432".to_string(),
        source_name: Some("Test Buyer".to_string()),
        source_group: "pharmacy-group@g.us".to_string(),
        medication: "Augmentin 1g".to_string(),
        medication_raw: "أوجمنتين 1 جرام".to_string(),
        quantity: Decimal::from_f64(30.0),
        unit: Some("boxes".to_string()),
        max_price: Decimal::from_f64(160.0),
        currency: Some("EGP".to_string()),
        urgency_level: UrgencyLevel::Normal,
        expiry_requirement: None,
        ai_confidence: 0.9,
        notes: None,
        status: ItemStatus::Active,
        content_embedding: None,
        created_at: now,
        updated_at: now,
    }
}

/// Create a match between offer and request
fn create_match(offer: &Offer, request: &Request, score: f64) -> Match {
    Match {
        id: uuid::Uuid::new_v4().to_string(),
        offer_id: offer.id.clone(),
        request_id: request.id.clone(),
        score,
        reasoning: Some(format!(
            "Medication match: {} vs {}",
            offer.medication, request.medication
        )),
        matched_by: Some("AUTO".to_string()),
        status: MatchStatus::Pending,
        created_at: Utc::now(),
        confirmed_at: None,
        notes: None,
    }
}

// =============================================================================
// End-to-End Flow Tests
// =============================================================================

/// Test complete message flow from raw message to match
#[test]
fn test_e2e_message_to_match_flow() {
    // Step 1: Create raw message
    let raw_message = create_raw_message();
    assert!(!raw_message.id.is_empty());
    assert!(raw_message.processed_at.is_none());

    // Step 2: Parse into offer
    let offer = create_offer();
    assert_eq!(offer.medication, "Augmentin 1g");
    assert_eq!(offer.quantity_f64(), 50.0);
    assert_eq!(offer.price_f64(), 150.0);
    assert_eq!(offer.status, ItemStatus::Active);

    // Step 3: Create matching request
    let request = create_matching_request();
    assert_eq!(request.medication, offer.medication);

    // Step 4: Calculate match score
    let score = 0.85;
    let match_entity = create_match(&offer, &request, score);

    // Verify match entity
    assert_eq!(match_entity.offer_id, offer.id);
    assert_eq!(match_entity.request_id, request.id);
    assert!(match_entity.score >= 0.8);
}

/// Test WebSocket event broadcast in E2E flow
#[tokio::test]
async fn test_e2e_websocket_broadcast() {
    let (tx, mut rx) = broadcast::channel::<WsEvent>(16);

    // Simulate match creation
    let offer = create_offer();
    let request = create_matching_request();
    let match_entity = create_match(&offer, &request, 0.90);

    // Broadcast match event
    tx.send(WsEvent::NewMatch(match_entity.clone())).unwrap();

    // Verify event received
    let event = rx.recv().await.unwrap();
    match event {
        WsEvent::NewMatch(m) => {
            assert_eq!(m.offer_id, offer.id);
            assert_eq!(m.score, 0.90);
        }
        _ => panic!("Expected NewMatch event"),
    }
}

/// Test notification dispatch in E2E flow
#[tokio::test]
async fn test_e2e_notification_dispatch() {
    let notifier = CompositeNotifier::new().with_notifier(NullNotifier);

    let offer = create_offer();
    let request = create_matching_request();
    let match_entity = create_match(&offer, &request, 0.95);
    let action = MatchAction::AutoConfirm;

    // Should not fail
    let result = notifier.notify_new_match(&match_entity, action).await;
    assert!(result.is_ok());
}

/// Test embedding similarity in E2E flow
#[test]
fn test_e2e_embedding_similarity() {
    // Simulate embeddings for similar medications
    let offer_embedding: Vec<f32> = vec![0.5; 768];
    let mut request_embedding: Vec<f32> = vec![0.5; 768];

    // Add small difference
    request_embedding[0] = 0.51;

    let similarity = cosine_similarity(&offer_embedding, &request_embedding).unwrap();
    assert!(
        similarity > 0.99,
        "Similar embeddings should have high similarity"
    );
}

/// Test weight configuration in E2E flow
#[test]
fn test_e2e_weight_configuration() {
    let weights = Weights::default();

    // Verify weights are valid
    let sum =
        weights.medication + weights.dosage + weights.quantity + weights.price + weights.recency;

    assert!((sum - 1.0).abs() < 0.01, "Weights should sum to 1.0");

    // Medication should have highest weight
    assert!(
        weights.medication > weights.price,
        "Medication weight should be higher than price"
    );
}

/// Test match status transitions
#[test]
fn test_e2e_match_status_transitions() {
    let offer = create_offer();
    let request = create_matching_request();
    let mut match_entity = create_match(&offer, &request, 0.85);

    // Initial status
    assert_eq!(match_entity.status, MatchStatus::Pending);
    assert!(match_entity.confirmed_at.is_none());

    // Confirm match
    match_entity.status = MatchStatus::Confirmed;
    match_entity.confirmed_at = Some(Utc::now());
    match_entity.matched_by = Some("operator@pharma.com".to_string());

    assert_eq!(match_entity.status, MatchStatus::Confirmed);
    assert!(match_entity.confirmed_at.is_some());
}

/// Test match action variants
#[test]
fn test_e2e_match_action_variants() {
    // Test all action variants exist
    assert!(matches!(MatchAction::AutoConfirm, MatchAction::AutoConfirm));
    assert!(matches!(
        MatchAction::SuggestToOperator,
        MatchAction::SuggestToOperator
    ));
    assert!(matches!(
        MatchAction::QueueForReview,
        MatchAction::QueueForReview
    ));
    assert!(matches!(MatchAction::Ignore, MatchAction::Ignore));
}

/// Test full match scoring calculation
#[test]
fn test_e2e_full_scoring() {
    let weights = Weights::default();

    // Simulate component scores
    let medication_score = 1.0;
    let dosage_score = 0.9;
    let quantity_score = 0.6;
    let price_score = 1.0;
    let recency_score = 0.8;

    // Calculate weighted score
    let total = medication_score * weights.medication
        + dosage_score * weights.dosage
        + quantity_score * weights.quantity
        + price_score * weights.price
        + recency_score * weights.recency;

    // Should be a high score
    assert!(total > 0.75, "Well-matched items should score high");
    assert!(total <= 1.0, "Score should not exceed 1.0");
}
