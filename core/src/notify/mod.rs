//! Match Notification System
//!
//! Implements Task 5.2: MatchNotifier trait
//! Provides notification delivery for match events.

mod email;
mod telegram;

pub use email::{EmailConfig, EmailNotifier};
pub use telegram::{TelegramConfig, TelegramNotifier};

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::Result;
use crate::domain::Match;
use crate::matching::MatchAction;
use crate::ws::WsEvent;

// ============================================================================
// Notifier Trait
// ============================================================================

/// Trait for notifying about match events
#[async_trait]
pub trait MatchNotifier: Send + Sync {
    /// Notify about a new match requiring attention
    async fn notify_new_match(&self, match_entity: &Match, action: MatchAction) -> Result<()>;

    /// Notify that a match has been auto-confirmed
    async fn notify_auto_confirmed(&self, match_id: &str, score: f64) -> Result<()>;

    /// Notify that a match is suggested for operator approval
    async fn notify_suggested(&self, match_entity: &Match) -> Result<()>;

    /// Notify that a match has been queued for review
    async fn notify_queued_for_review(&self, match_id: &str, reason: &str) -> Result<()>;
}

// ============================================================================
// WebSocket Notifier
// ============================================================================

/// WebSocket-based notifier that broadcasts events to connected clients
pub struct WebSocketNotifier {
    tx: broadcast::Sender<WsEvent>,
}

impl WebSocketNotifier {
    /// Create a new WebSocket notifier
    pub fn new(tx: broadcast::Sender<WsEvent>) -> Self {
        Self { tx }
    }

    /// Broadcast an event (non-blocking)
    fn broadcast(&self, event: WsEvent) {
        // Ignore errors if no receivers
        let _ = self.tx.send(event);
    }
}

#[async_trait]
impl MatchNotifier for WebSocketNotifier {
    async fn notify_new_match(&self, match_entity: &Match, action: MatchAction) -> Result<()> {
        tracing::info!(
            match_id = %match_entity.id,
            score = match_entity.score,
            action = %action,
            "Notifying about new match"
        );

        // Broadcast the new match event
        self.broadcast(WsEvent::NewMatch(match_entity.clone()));

        Ok(())
    }

    async fn notify_auto_confirmed(&self, match_id: &str, score: f64) -> Result<()> {
        tracing::info!(
            match_id = %match_id,
            score = score,
            "Match auto-confirmed"
        );

        // Broadcast auto-confirmation event
        self.broadcast(WsEvent::MatchConfirmed(crate::ws::MatchStatusEvent {
            match_id: match_id.to_string(),
            user_id: "system".to_string(),
            notes: Some(format!("Auto-confirmed with score {:.2}", score)),
            reason: None,
        }));

        Ok(())
    }

    async fn notify_suggested(&self, match_entity: &Match) -> Result<()> {
        tracing::info!(
            match_id = %match_entity.id,
            score = match_entity.score,
            "Match suggested for operator"
        );

        // Broadcast suggestion event (uses NewMatch for now, operators will see it)
        self.broadcast(WsEvent::NewMatch(match_entity.clone()));

        Ok(())
    }

    async fn notify_queued_for_review(&self, match_id: &str, reason: &str) -> Result<()> {
        tracing::info!(
            match_id = %match_id,
            reason = %reason,
            "Match queued for review"
        );

        // TODO: Add a specific WsEvent::MatchQueuedForReview in the future
        Ok(())
    }
}

// ============================================================================
// Composite Notifier
// ============================================================================

/// A notifier that delegates to multiple notifiers
pub struct CompositeNotifier {
    notifiers: Vec<Box<dyn MatchNotifier>>,
}

impl CompositeNotifier {
    /// Create an empty composite notifier
    pub fn new() -> Self {
        Self {
            notifiers: Vec::new(),
        }
    }

    /// Add a notifier to the composite (builder pattern)
    pub fn with_notifier(mut self, notifier: impl MatchNotifier + 'static) -> Self {
        self.notifiers.push(Box::new(notifier));
        self
    }
}

impl Default for CompositeNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MatchNotifier for CompositeNotifier {
    async fn notify_new_match(&self, match_entity: &Match, action: MatchAction) -> Result<()> {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.notify_new_match(match_entity, action).await {
                tracing::warn!(error = %e, "Notifier failed for new_match");
            }
        }
        Ok(())
    }

    async fn notify_auto_confirmed(&self, match_id: &str, score: f64) -> Result<()> {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.notify_auto_confirmed(match_id, score).await {
                tracing::warn!(error = %e, "Notifier failed for auto_confirmed");
            }
        }
        Ok(())
    }

    async fn notify_suggested(&self, match_entity: &Match) -> Result<()> {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.notify_suggested(match_entity).await {
                tracing::warn!(error = %e, "Notifier failed for suggested");
            }
        }
        Ok(())
    }

    async fn notify_queued_for_review(&self, match_id: &str, reason: &str) -> Result<()> {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.notify_queued_for_review(match_id, reason).await {
                tracing::warn!(error = %e, "Notifier failed for queued_for_review");
            }
        }
        Ok(())
    }
}

// ============================================================================
// Null Notifier (for testing)
// ============================================================================

/// A notifier that does nothing (useful for testing)
pub struct NullNotifier;

#[async_trait]
impl MatchNotifier for NullNotifier {
    async fn notify_new_match(&self, _: &Match, _: MatchAction) -> Result<()> {
        Ok(())
    }

    async fn notify_auto_confirmed(&self, _: &str, _: f64) -> Result<()> {
        Ok(())
    }

    async fn notify_suggested(&self, _: &Match) -> Result<()> {
        Ok(())
    }

    async fn notify_queued_for_review(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_notifier() {
        let notifier = NullNotifier;

        // Should not fail
        assert!(
            notifier
                .notify_auto_confirmed("test-id", 0.95)
                .await
                .is_ok()
        );
        assert!(
            notifier
                .notify_queued_for_review("test-id", "low_confidence")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_composite_notifier_empty() {
        let notifier = CompositeNotifier::new();

        // Empty composite should not fail
        assert!(
            notifier
                .notify_auto_confirmed("test-id", 0.95)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_websocket_notifier() {
        let (tx, _rx) = broadcast::channel(16);
        let notifier = WebSocketNotifier::new(tx);

        // Should not fail even with no receivers
        assert!(
            notifier
                .notify_auto_confirmed("match-123", 0.92)
                .await
                .is_ok()
        );
        assert!(
            notifier
                .notify_queued_for_review("match-456", "low_score")
                .await
                .is_ok()
        );
    }
}
