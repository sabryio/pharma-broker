//! Review Queue entity
//!
//! Stores AI parse results that require human review due to low confidence.
//! Messages with avg_confidence < 0.5 are automatically queued for review.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================================
// Review Queue Status
// ============================================================================

/// Status of a review queue item
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "review_status", rename_all = "lowercase")]
pub enum ReviewStatus {
    /// Pending human review
    #[default]
    Pending,
    /// Approved by reviewer (parse result is correct)
    Approved,
    /// Rejected by reviewer (parse result is incorrect)
    Rejected,
    /// Skipped/deferred for later review
    Skipped,
}

impl std::fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

// ============================================================================
// Review Queue Item
// ============================================================================

/// A queued AI parse result requiring human review
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReviewQueueItem {
    /// Unique identifier
    pub id: Uuid,
    /// Reference to the original raw message
    pub raw_message_id: String,
    /// The AI parse result as JSON
    pub ai_result: serde_json::Value,
    /// Average confidence score from AI (0.0 - 1.0)
    pub confidence: f64,
    /// Reason for queuing (e.g., "low_confidence", "ambiguous_medication")
    pub reason: String,
    /// Current review status
    #[sqlx(rename = "status")]
    pub status: ReviewStatus,
    /// Who reviewed this item (if reviewed)
    pub reviewed_by: Option<String>,
    /// Notes from the reviewer
    pub review_notes: Option<String>,
    /// When the item was queued
    pub created_at: DateTime<Utc>,
    /// When the item was reviewed
    pub reviewed_at: Option<DateTime<Utc>>,
}

impl ReviewQueueItem {
    /// Create a new review queue item for a low-confidence parse result
    pub fn new(
        raw_message_id: String,
        ai_result: serde_json::Value,
        confidence: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            raw_message_id,
            ai_result,
            confidence,
            reason: reason.into(),
            status: ReviewStatus::Pending,
            reviewed_by: None,
            review_notes: None,
            created_at: Utc::now(),
            reviewed_at: None,
        }
    }

    /// Create a review item for low confidence
    pub fn for_low_confidence(
        raw_message_id: String,
        ai_result: serde_json::Value,
        confidence: f64,
    ) -> Self {
        Self::new(
            raw_message_id,
            ai_result,
            confidence,
            format!("low_confidence: {:.2}", confidence),
        )
    }

    /// Mark as approved
    pub fn approve(&mut self, reviewer: String, notes: Option<String>) {
        self.status = ReviewStatus::Approved;
        self.reviewed_by = Some(reviewer);
        self.review_notes = notes;
        self.reviewed_at = Some(Utc::now());
    }

    /// Mark as rejected
    pub fn reject(&mut self, reviewer: String, notes: Option<String>) {
        self.status = ReviewStatus::Rejected;
        self.reviewed_by = Some(reviewer);
        self.review_notes = notes;
        self.reviewed_at = Some(Utc::now());
    }

    /// Skip for later review
    pub fn skip(&mut self, reviewer: String, notes: Option<String>) {
        self.status = ReviewStatus::Skipped;
        self.reviewed_by = Some(reviewer);
        self.review_notes = notes;
        self.reviewed_at = Some(Utc::now());
    }

    /// Check if this item is still pending review
    pub fn is_pending(&self) -> bool {
        self.status == ReviewStatus::Pending
    }
}

// ============================================================================
// Review Queue Statistics
// ============================================================================

/// Statistics for the review queue
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReviewQueueStats {
    /// Total items in queue
    pub total: i64,
    /// Items pending review
    pub pending: i64,
    /// Items approved
    pub approved: i64,
    /// Items rejected
    pub rejected: i64,
    /// Items skipped
    pub skipped: i64,
    /// Average confidence score of pending items
    pub avg_pending_confidence: f64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_status_display() {
        assert_eq!(ReviewStatus::Pending.to_string(), "pending");
        assert_eq!(ReviewStatus::Approved.to_string(), "approved");
        assert_eq!(ReviewStatus::Rejected.to_string(), "rejected");
        assert_eq!(ReviewStatus::Skipped.to_string(), "skipped");
    }

    #[test]
    fn test_review_queue_item_new() {
        let item = ReviewQueueItem::new(
            "msg-123".to_string(),
            serde_json::json!({"items": []}),
            0.45,
            "low_confidence",
        );

        assert_eq!(item.raw_message_id, "msg-123");
        assert_eq!(item.confidence, 0.45);
        assert_eq!(item.status, ReviewStatus::Pending);
        assert!(item.reviewed_by.is_none());
        assert!(item.is_pending());
    }

    #[test]
    fn test_review_queue_item_for_low_confidence() {
        let item = ReviewQueueItem::for_low_confidence(
            "msg-456".to_string(),
            serde_json::json!({"items": [{"type": "OFFER"}]}),
            0.35,
        );

        assert!(item.reason.contains("low_confidence"));
        assert!(item.reason.contains("0.35"));
    }

    #[test]
    fn test_review_queue_item_approve() {
        let mut item =
            ReviewQueueItem::new("msg-789".to_string(), serde_json::json!({}), 0.45, "test");

        item.approve(
            "reviewer@example.com".to_string(),
            Some("Looks correct".to_string()),
        );

        assert_eq!(item.status, ReviewStatus::Approved);
        assert_eq!(item.reviewed_by, Some("reviewer@example.com".to_string()));
        assert!(item.reviewed_at.is_some());
        assert!(!item.is_pending());
    }

    #[test]
    fn test_review_queue_item_reject() {
        let mut item =
            ReviewQueueItem::new("msg-abc".to_string(), serde_json::json!({}), 0.3, "test");

        item.reject("admin".to_string(), Some("Wrong medication".to_string()));

        assert_eq!(item.status, ReviewStatus::Rejected);
        assert_eq!(item.review_notes, Some("Wrong medication".to_string()));
    }

    #[test]
    fn test_review_queue_stats_default() {
        let stats = ReviewQueueStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
    }
}
