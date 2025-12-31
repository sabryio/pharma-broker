//! Phase 3: Features Integration Tests
//!
//! Tests for WebSocket events.
//! See: docs/phases/03-features.md

use tokio::sync::broadcast;

use chrono::Utc;
use pharma_core::domain::{Match, MatchStatus};
use pharma_core::ws::WsEvent;
use uuid::Uuid;

/// Create a test match for WebSocket events
fn create_test_match() -> Match {
    Match {
        id: Uuid::new_v4(),
        offer_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        score: 0.85,
        reasoning: Some("High medication similarity".to_string()),
        matched_by: Some("AUTO".to_string()),
        status: MatchStatus::Pending,
        created_at: Utc::now(),
        confirmed_at: None,
        notes: None,
    }
}

/// Test WebSocket broadcast channel
#[tokio::test]
async fn test_websocket_broadcast() {
    let (tx, mut rx1) = broadcast::channel::<WsEvent>(16);
    let mut rx2 = tx.subscribe();

    // Broadcast a new match event
    let match_entity = create_test_match();
    tx.send(WsEvent::NewMatch(match_entity.clone())).unwrap();

    // Both receivers should get the event
    let event1 = rx1.recv().await.unwrap();
    let event2 = rx2.recv().await.unwrap();

    match event1 {
        WsEvent::NewMatch(m) => assert_eq!(m.id, match_entity.id),
        _ => panic!("Expected NewMatch event"),
    }

    match event2 {
        WsEvent::NewMatch(m) => assert_eq!(m.score, 0.85),
        _ => panic!("Expected NewMatch event"),
    }
}

/// Test multiple event types
#[tokio::test]
async fn test_multiple_event_types() {
    let (tx, mut rx) = broadcast::channel::<WsEvent>(16);

    // Send different event types
    tx.send(WsEvent::NewMatch(create_test_match())).unwrap();
    tx.send(WsEvent::Ping).unwrap();

    // Receive and verify
    let event1 = rx.recv().await.unwrap();
    assert!(matches!(event1, WsEvent::NewMatch(_)));

    let event2 = rx.recv().await.unwrap();
    assert!(matches!(event2, WsEvent::Ping));
}

/// Test broadcast with no receivers (should not block)
#[test]
fn test_broadcast_no_receivers() {
    let (tx, _) = broadcast::channel::<WsEvent>(16);

    // This should not block even with no active receivers
    let result = tx.send(WsEvent::Ping);
    // With no receivers, this returns an error, which is fine
    assert!(result.is_err());
}

/// Test receiver lag handling
#[tokio::test]
async fn test_receiver_lag() {
    let (tx, mut rx) = broadcast::channel::<WsEvent>(4);

    // Send more events than buffer size
    for _ in 0..10 {
        let _ = tx.send(WsEvent::Ping);
    }

    // Receiver should get lagged error then can continue
    let result = rx.recv().await;
    // Either succeeds with recent event or gets Lagged error
    match result {
        Ok(_) => {} // Got an event
        Err(broadcast::error::RecvError::Lagged(n)) => {
            assert!(n > 0); // Missed some events
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
