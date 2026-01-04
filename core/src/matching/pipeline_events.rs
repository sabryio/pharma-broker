//! WebSocket Pipeline Events
//!
//! This module defines the event types and emitter for real-time pipeline updates.
//! Events are emitted during match operations to allow frontend clients to display
//! live progress via WebSocket connections.
//!
//! Feature: debug-recording-enhancement
//! Implements: Requirements 2.1, 2.2, 2.3, 2.4, 2.5

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::PipelineStageType;

// =============================================================================
// Pipeline Event Types (Task 4.1)
// =============================================================================

/// Events emitted during pipeline execution for real-time WebSocket updates.
///
/// These events allow frontend clients to track match operation progress in real-time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineEvent {
    /// Emitted when a match operation starts
    MatchStarted {
        match_id: Uuid,
        offer_id: Uuid,
        request_id: Uuid,
        timestamp: DateTime<Utc>,
        /// Optional session ID for frontend correlation
        session_id: Option<String>,
    },

    /// Emitted when a pipeline stage completes successfully
    StageCompleted {
        match_id: Uuid,
        stage: PipelineStageType,
        stage_name: String,
        duration_ms: u64,
        candidates_in: usize,
        candidates_out: usize,
        /// Human-readable summary of what happened in this stage
        summary: String,
        timestamp: DateTime<Utc>,
    },

    /// Emitted when AI processing begins (parsing or review)
    AiProcessingStarted {
        match_id: Uuid,
        model: String,
        operation: AiOperation,
        /// Estimated duration based on historical data
        estimated_duration_ms: Option<u64>,
        timestamp: DateTime<Utc>,
    },

    /// Emitted when AI processing completes
    AiProcessingCompleted {
        match_id: Uuid,
        model: String,
        operation: AiOperation,
        duration_ms: u64,
        success: bool,
        timestamp: DateTime<Utc>,
    },

    /// Emitted when a match operation completes successfully
    MatchCompleted {
        match_id: Uuid,
        audit_record_id: Uuid,
        final_score: f64,
        outcome: MatchOutcome,
        total_duration_ms: u64,
        stages_completed: usize,
        timestamp: DateTime<Utc>,
    },

    /// Emitted when a pipeline stage encounters an error
    StageError {
        match_id: Uuid,
        stage: PipelineStageType,
        stage_name: String,
        error: String,
        /// Any partial results that were computed before the error
        partial_results: Option<serde_json::Value>,
        /// Whether the pipeline can continue after this error
        recoverable: bool,
        timestamp: DateTime<Utc>,
    },

    /// Emitted when the entire match operation fails
    MatchFailed {
        match_id: Uuid,
        error: String,
        last_completed_stage: Option<PipelineStageType>,
        partial_audit_record_id: Option<Uuid>,
        timestamp: DateTime<Utc>,
    },

    /// Emitted for progress updates during long-running stages
    StageProgress {
        match_id: Uuid,
        stage: PipelineStageType,
        stage_name: String,
        progress_percent: u8,
        message: String,
        timestamp: DateTime<Utc>,
    },
}

/// Type of AI operation being performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiOperation {
    /// Parsing a message to extract medication data
    Parsing,
    /// Reviewing a match for approval
    Review,
    /// Consensus auditing with multiple models
    ConsensusAudit,
    /// Contrastive validation
    ContrastiveValidation,
}

impl std::fmt::Display for AiOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiOperation::Parsing => write!(f, "parsing"),
            AiOperation::Review => write!(f, "review"),
            AiOperation::ConsensusAudit => write!(f, "consensus_audit"),
            AiOperation::ContrastiveValidation => write!(f, "contrastive_validation"),
        }
    }
}

/// Outcome of a completed match operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOutcome {
    /// Match was approved (manually or automatically)
    Approved,
    /// Match was rejected
    Rejected,
    /// Match is pending review
    PendingReview,
    /// Match was auto-approved based on confidence
    AutoApproved,
    /// Match was flagged for manual review
    Flagged,
    /// No suitable match was found
    NoMatch,
}

impl std::fmt::Display for MatchOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchOutcome::Approved => write!(f, "approved"),
            MatchOutcome::Rejected => write!(f, "rejected"),
            MatchOutcome::PendingReview => write!(f, "pending_review"),
            MatchOutcome::AutoApproved => write!(f, "auto_approved"),
            MatchOutcome::Flagged => write!(f, "flagged"),
            MatchOutcome::NoMatch => write!(f, "no_match"),
        }
    }
}

impl PipelineEvent {
    /// Get the match_id associated with this event
    pub fn match_id(&self) -> Uuid {
        match self {
            PipelineEvent::MatchStarted { match_id, .. } => *match_id,
            PipelineEvent::StageCompleted { match_id, .. } => *match_id,
            PipelineEvent::AiProcessingStarted { match_id, .. } => *match_id,
            PipelineEvent::AiProcessingCompleted { match_id, .. } => *match_id,
            PipelineEvent::MatchCompleted { match_id, .. } => *match_id,
            PipelineEvent::StageError { match_id, .. } => *match_id,
            PipelineEvent::MatchFailed { match_id, .. } => *match_id,
            PipelineEvent::StageProgress { match_id, .. } => *match_id,
        }
    }

    /// Get the timestamp of this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            PipelineEvent::MatchStarted { timestamp, .. } => *timestamp,
            PipelineEvent::StageCompleted { timestamp, .. } => *timestamp,
            PipelineEvent::AiProcessingStarted { timestamp, .. } => *timestamp,
            PipelineEvent::AiProcessingCompleted { timestamp, .. } => *timestamp,
            PipelineEvent::MatchCompleted { timestamp, .. } => *timestamp,
            PipelineEvent::StageError { timestamp, .. } => *timestamp,
            PipelineEvent::MatchFailed { timestamp, .. } => *timestamp,
            PipelineEvent::StageProgress { timestamp, .. } => *timestamp,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            PipelineEvent::StageError { .. } | PipelineEvent::MatchFailed { .. }
        )
    }

    /// Check if this is a terminal event (match completed or failed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PipelineEvent::MatchCompleted { .. } | PipelineEvent::MatchFailed { .. }
        )
    }

    /// Get the event type name for logging/metrics
    pub fn event_type(&self) -> &'static str {
        match self {
            PipelineEvent::MatchStarted { .. } => "match_started",
            PipelineEvent::StageCompleted { .. } => "stage_completed",
            PipelineEvent::AiProcessingStarted { .. } => "ai_processing_started",
            PipelineEvent::AiProcessingCompleted { .. } => "ai_processing_completed",
            PipelineEvent::MatchCompleted { .. } => "match_completed",
            PipelineEvent::StageError { .. } => "stage_error",
            PipelineEvent::MatchFailed { .. } => "match_failed",
            PipelineEvent::StageProgress { .. } => "stage_progress",
        }
    }
}

// =============================================================================
// Pipeline Event Emitter (Task 4.2)
// =============================================================================

/// Default channel capacity for event broadcasting
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Trait for emitting pipeline events
///
/// Implementations of this trait can broadcast events to subscribers
/// and allow filtering by match_id.
#[async_trait::async_trait]
pub trait PipelineEventEmitter: Send + Sync {
    /// Emit an event to all subscribers
    fn emit(&self, event: PipelineEvent);

    /// Subscribe to events for a specific match_id
    /// Returns a receiver that will only receive events for that match
    fn subscribe(&self, match_id: Uuid) -> broadcast::Receiver<PipelineEvent>;

    /// Subscribe to all events (no filtering)
    fn subscribe_all(&self) -> broadcast::Receiver<PipelineEvent>;

    /// Get the number of active subscribers
    fn subscriber_count(&self) -> usize;
}

/// Broadcast channel-based implementation of PipelineEventEmitter
///
/// This implementation uses tokio's broadcast channel to efficiently
/// distribute events to multiple subscribers.
pub struct BroadcastEventEmitter {
    /// Main broadcast channel for all events
    sender: broadcast::Sender<PipelineEvent>,
    /// Channel capacity (stored since broadcast::Sender doesn't expose it)
    capacity: usize,
    /// Track active subscriptions per match_id for metrics
    subscriptions: std::sync::RwLock<HashMap<Uuid, usize>>,
}

impl BroadcastEventEmitter {
    /// Create a new broadcast event emitter with default capacity
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new broadcast event emitter with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            capacity,
            subscriptions: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get the channel capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of pending messages in the channel
    pub fn pending_count(&self) -> usize {
        self.sender.len()
    }

    /// Track a subscription for a specific match_id
    fn track_subscription(&self, match_id: Uuid) {
        if let Ok(mut subs) = self.subscriptions.write() {
            *subs.entry(match_id).or_insert(0) += 1;
        }
    }

    /// Untrack a subscription for a specific match_id
    #[allow(dead_code)]
    fn untrack_subscription(&self, match_id: Uuid) {
        if let Ok(mut subs) = self.subscriptions.write()
            && let Some(count) = subs.get_mut(&match_id)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                subs.remove(&match_id);
            }
        }
    }
}

impl Default for BroadcastEventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PipelineEventEmitter for BroadcastEventEmitter {
    fn emit(&self, event: PipelineEvent) {
        // Log emission for debugging
        tracing::debug!(
            event_type = event.event_type(),
            match_id = %event.match_id(),
            "Emitting pipeline event"
        );

        // Send to all subscribers - ignore errors if no subscribers
        let _ = self.sender.send(event);
    }

    fn subscribe(&self, match_id: Uuid) -> broadcast::Receiver<PipelineEvent> {
        self.track_subscription(match_id);
        self.sender.subscribe()
    }

    fn subscribe_all(&self) -> broadcast::Receiver<PipelineEvent> {
        self.sender.subscribe()
    }

    fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

/// Shared reference to a pipeline event emitter
pub type SharedEventEmitter = Arc<dyn PipelineEventEmitter>;

/// Create a new shared broadcast event emitter
pub fn new_shared_emitter() -> SharedEventEmitter {
    Arc::new(BroadcastEventEmitter::new())
}

/// Create a new shared broadcast event emitter with specified capacity
pub fn new_shared_emitter_with_capacity(capacity: usize) -> SharedEventEmitter {
    Arc::new(BroadcastEventEmitter::with_capacity(capacity))
}

// =============================================================================
// Filtered Event Receiver (Task 4.2)
// =============================================================================

/// A receiver that filters events by match_id
///
/// This wraps a broadcast receiver and only yields events for a specific match.
pub struct FilteredEventReceiver {
    match_id: Uuid,
    receiver: broadcast::Receiver<PipelineEvent>,
}

impl FilteredEventReceiver {
    /// Create a new filtered receiver
    pub fn new(match_id: Uuid, receiver: broadcast::Receiver<PipelineEvent>) -> Self {
        Self { match_id, receiver }
    }

    /// Receive the next event for this match_id
    ///
    /// This will skip events for other matches and only return events
    /// that match the configured match_id.
    pub async fn recv(&mut self) -> Result<PipelineEvent, broadcast::error::RecvError> {
        loop {
            let event = self.receiver.recv().await?;
            if event.match_id() == self.match_id {
                return Ok(event);
            }
            // Skip events for other matches
        }
    }

    /// Try to receive the next event without blocking
    pub fn try_recv(&mut self) -> Result<PipelineEvent, broadcast::error::TryRecvError> {
        loop {
            let event = self.receiver.try_recv()?;
            if event.match_id() == self.match_id {
                return Ok(event);
            }
            // Skip events for other matches
        }
    }

    /// Get the match_id this receiver is filtering for
    pub fn match_id(&self) -> Uuid {
        self.match_id
    }
}

// =============================================================================
// Event Builder Helpers (Task 4.3 preparation)
// =============================================================================

impl PipelineEvent {
    /// Create a MatchStarted event
    pub fn match_started(
        match_id: Uuid,
        offer_id: Uuid,
        request_id: Uuid,
        session_id: Option<String>,
    ) -> Self {
        PipelineEvent::MatchStarted {
            match_id,
            offer_id,
            request_id,
            timestamp: Utc::now(),
            session_id,
        }
    }

    /// Create a StageCompleted event
    pub fn stage_completed(
        match_id: Uuid,
        stage: PipelineStageType,
        duration_ms: u64,
        candidates_in: usize,
        candidates_out: usize,
        summary: impl Into<String>,
    ) -> Self {
        PipelineEvent::StageCompleted {
            match_id,
            stage,
            stage_name: stage.to_string(),
            duration_ms,
            candidates_in,
            candidates_out,
            summary: summary.into(),
            timestamp: Utc::now(),
        }
    }

    /// Create an AiProcessingStarted event
    pub fn ai_processing_started(
        match_id: Uuid,
        model: impl Into<String>,
        operation: AiOperation,
        estimated_duration_ms: Option<u64>,
    ) -> Self {
        PipelineEvent::AiProcessingStarted {
            match_id,
            model: model.into(),
            operation,
            estimated_duration_ms,
            timestamp: Utc::now(),
        }
    }

    /// Create an AiProcessingCompleted event
    pub fn ai_processing_completed(
        match_id: Uuid,
        model: impl Into<String>,
        operation: AiOperation,
        duration_ms: u64,
        success: bool,
    ) -> Self {
        PipelineEvent::AiProcessingCompleted {
            match_id,
            model: model.into(),
            operation,
            duration_ms,
            success,
            timestamp: Utc::now(),
        }
    }

    /// Create a MatchCompleted event
    pub fn match_completed(
        match_id: Uuid,
        audit_record_id: Uuid,
        final_score: f64,
        outcome: MatchOutcome,
        total_duration_ms: u64,
        stages_completed: usize,
    ) -> Self {
        PipelineEvent::MatchCompleted {
            match_id,
            audit_record_id,
            final_score,
            outcome,
            total_duration_ms,
            stages_completed,
            timestamp: Utc::now(),
        }
    }

    /// Create a StageError event
    pub fn stage_error(
        match_id: Uuid,
        stage: PipelineStageType,
        error: impl Into<String>,
        partial_results: Option<serde_json::Value>,
        recoverable: bool,
    ) -> Self {
        PipelineEvent::StageError {
            match_id,
            stage,
            stage_name: stage.to_string(),
            error: error.into(),
            partial_results,
            recoverable,
            timestamp: Utc::now(),
        }
    }

    /// Create a MatchFailed event
    pub fn match_failed(
        match_id: Uuid,
        error: impl Into<String>,
        last_completed_stage: Option<PipelineStageType>,
        partial_audit_record_id: Option<Uuid>,
    ) -> Self {
        PipelineEvent::MatchFailed {
            match_id,
            error: error.into(),
            last_completed_stage,
            partial_audit_record_id,
            timestamp: Utc::now(),
        }
    }

    /// Create a StageProgress event
    pub fn stage_progress(
        match_id: Uuid,
        stage: PipelineStageType,
        progress_percent: u8,
        message: impl Into<String>,
    ) -> Self {
        PipelineEvent::StageProgress {
            match_id,
            stage,
            stage_name: stage.to_string(),
            progress_percent: progress_percent.min(100),
            message: message.into(),
            timestamp: Utc::now(),
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_event_serialization() {
        let event = PipelineEvent::match_started(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("session-123".to_string()),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("match_started"));
        assert!(json.contains("session-123"));

        let deserialized: PipelineEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PipelineEvent::MatchStarted { .. }));
    }

    #[test]
    fn test_stage_completed_serialization() {
        let event = PipelineEvent::stage_completed(
            Uuid::new_v4(),
            PipelineStageType::HierarchicalStage { stage_number: 2 },
            150,
            100,
            50,
            "Filtered 50 candidates",
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("stage_completed"));
        assert!(json.contains("Filtered 50 candidates"));
    }

    #[test]
    fn test_ai_operation_display() {
        assert_eq!(AiOperation::Parsing.to_string(), "parsing");
        assert_eq!(AiOperation::Review.to_string(), "review");
        assert_eq!(AiOperation::ConsensusAudit.to_string(), "consensus_audit");
    }

    #[test]
    fn test_match_outcome_display() {
        assert_eq!(MatchOutcome::Approved.to_string(), "approved");
        assert_eq!(MatchOutcome::AutoApproved.to_string(), "auto_approved");
        assert_eq!(MatchOutcome::PendingReview.to_string(), "pending_review");
    }

    #[test]
    fn test_event_match_id_extraction() {
        let match_id = Uuid::new_v4();
        let event = PipelineEvent::match_started(match_id, Uuid::new_v4(), Uuid::new_v4(), None);
        assert_eq!(event.match_id(), match_id);
    }

    #[test]
    fn test_event_is_error() {
        let match_id = Uuid::new_v4();

        let error_event = PipelineEvent::stage_error(
            match_id,
            PipelineStageType::AiParsing,
            "Test error",
            None,
            false,
        );
        assert!(error_event.is_error());

        let success_event =
            PipelineEvent::match_started(match_id, Uuid::new_v4(), Uuid::new_v4(), None);
        assert!(!success_event.is_error());
    }

    #[test]
    fn test_event_is_terminal() {
        let match_id = Uuid::new_v4();

        let completed = PipelineEvent::match_completed(
            match_id,
            Uuid::new_v4(),
            0.95,
            MatchOutcome::Approved,
            1000,
            5,
        );
        assert!(completed.is_terminal());

        let failed = PipelineEvent::match_failed(match_id, "Error", None, None);
        assert!(failed.is_terminal());

        let started = PipelineEvent::match_started(match_id, Uuid::new_v4(), Uuid::new_v4(), None);
        assert!(!started.is_terminal());
    }

    #[tokio::test]
    async fn test_broadcast_emitter_basic() {
        let emitter = BroadcastEventEmitter::new();
        let mut receiver = emitter.subscribe_all();

        let match_id = Uuid::new_v4();
        let event = PipelineEvent::match_started(match_id, Uuid::new_v4(), Uuid::new_v4(), None);

        emitter.emit(event.clone());

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.match_id(), match_id);
    }

    #[tokio::test]
    async fn test_broadcast_emitter_multiple_subscribers() {
        let emitter = BroadcastEventEmitter::new();
        let mut receiver1 = emitter.subscribe_all();
        let mut receiver2 = emitter.subscribe_all();

        assert_eq!(emitter.subscriber_count(), 2);

        let match_id = Uuid::new_v4();
        let event = PipelineEvent::match_started(match_id, Uuid::new_v4(), Uuid::new_v4(), None);

        emitter.emit(event);

        let received1 = receiver1.recv().await.unwrap();
        let received2 = receiver2.recv().await.unwrap();

        assert_eq!(received1.match_id(), match_id);
        assert_eq!(received2.match_id(), match_id);
    }

    #[tokio::test]
    async fn test_filtered_receiver() {
        let emitter = BroadcastEventEmitter::new();
        let match_id_1 = Uuid::new_v4();
        let match_id_2 = Uuid::new_v4();

        let receiver = emitter.subscribe(match_id_1);
        let mut filtered = FilteredEventReceiver::new(match_id_1, receiver);

        // Emit events for both matches
        emitter.emit(PipelineEvent::match_started(
            match_id_2,
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
        ));
        emitter.emit(PipelineEvent::match_started(
            match_id_1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
        ));

        // Should only receive the event for match_id_1
        let received = filtered.recv().await.unwrap();
        assert_eq!(received.match_id(), match_id_1);
    }

    #[test]
    fn test_stage_progress_clamps_percent() {
        let event = PipelineEvent::stage_progress(
            Uuid::new_v4(),
            PipelineStageType::AiParsing,
            150, // Over 100
            "Processing",
        );

        if let PipelineEvent::StageProgress {
            progress_percent, ..
        } = event
        {
            assert_eq!(progress_percent, 100);
        } else {
            panic!("Expected StageProgress event");
        }
    }
}
