//! Repository parameter structs
//!
//! Provides descriptive structs for repository methods that have multiple arguments,
//! following best Rust practices for API design.

use chrono::Duration;

use super::entity::common::MatchStatus;
use super::entity::review_queue::ReviewStatus;

/// Parameters for updating match status
#[derive(Debug, Clone)]
pub struct UpdateMatchStatusParams<'a> {
    /// Match ID to update
    pub id: &'a str,
    /// New status
    pub status: MatchStatus,
    /// Who performed the match (operator ID or "AUTO")
    pub matched_by: &'a str,
    /// Optional notes about the status change
    pub notes: &'a str,
}

impl<'a> UpdateMatchStatusParams<'a> {
    pub fn new(id: &'a str, status: MatchStatus, matched_by: &'a str, notes: &'a str) -> Self {
        Self {
            id,
            status,
            matched_by,
            notes,
        }
    }

    /// Create params for auto-confirmation
    pub fn auto_confirm(id: &'a str) -> Self {
        Self {
            id,
            status: MatchStatus::Confirmed,
            matched_by: "AUTO",
            notes: "Auto-confirmed by matching engine",
        }
    }
}

/// Parameters for updating review queue status
#[derive(Debug, Clone)]
pub struct UpdateReviewStatusParams<'a> {
    /// Review item ID
    pub id: &'a str,
    /// New status
    pub status: ReviewStatus,
    /// Who reviewed the item
    pub reviewed_by: &'a str,
    /// Optional notes
    pub notes: Option<&'a str>,
}

impl<'a> UpdateReviewStatusParams<'a> {
    pub fn new(
        id: &'a str,
        status: ReviewStatus,
        reviewed_by: &'a str,
        notes: Option<&'a str>,
    ) -> Self {
        Self {
            id,
            status,
            reviewed_by,
            notes,
        }
    }

    /// Create approve params
    pub fn approve(id: &'a str, reviewed_by: &'a str) -> Self {
        Self {
            id,
            status: ReviewStatus::Approved,
            reviewed_by,
            notes: None,
        }
    }

    /// Create reject params with reason
    pub fn reject(id: &'a str, reviewed_by: &'a str, reason: &'a str) -> Self {
        Self {
            id,
            status: ReviewStatus::Rejected,
            reviewed_by,
            notes: Some(reason),
        }
    }
}

/// Parameters for finding recent duplicates
#[derive(Debug, Clone)]
pub struct FindDuplicateParams<'a> {
    /// Sender phone to match
    pub sender_phone: &'a str,
    /// Medication name to match
    pub medication: &'a str,
    /// Time window to search within
    pub within: Duration,
}

impl<'a> FindDuplicateParams<'a> {
    pub fn new(sender_phone: &'a str, medication: &'a str, within: Duration) -> Self {
        Self {
            sender_phone,
            medication,
            within,
        }
    }

    /// Create with default 24-hour window
    pub fn within_day(sender_phone: &'a str, medication: &'a str) -> Self {
        Self {
            sender_phone,
            medication,
            within: Duration::hours(24),
        }
    }
}

/// Parameters for semantic duplicate search
#[derive(Debug, Clone)]
pub struct SemanticDuplicateParams<'a> {
    /// Embedding vector to compare
    pub embedding: &'a [f32],
    /// Minimum similarity threshold (0.0-1.0)
    pub similarity_threshold: f64,
    /// Time window to search within
    pub within: Duration,
}

impl<'a> SemanticDuplicateParams<'a> {
    pub fn new(embedding: &'a [f32], similarity_threshold: f64, within: Duration) -> Self {
        Self {
            embedding,
            similarity_threshold,
            within,
        }
    }

    /// Create with default threshold of 0.85
    pub fn with_defaults(embedding: &'a [f32], within: Duration) -> Self {
        Self {
            embedding,
            similarity_threshold: 0.85,
            within,
        }
    }
}

/// Parameters for getting audit logs by entity
#[derive(Debug, Clone)]
pub struct AuditByEntityParams<'a> {
    /// Entity type (e.g., "match", "offer", "request")
    pub entity_type: &'a str,
    /// Entity ID
    pub entity_id: &'a str,
    /// Maximum results to return
    pub limit: i64,
}

impl<'a> AuditByEntityParams<'a> {
    pub fn new(entity_type: &'a str, entity_id: &'a str, limit: i64) -> Self {
        Self {
            entity_type,
            entity_id,
            limit,
        }
    }

    pub fn for_match(match_id: &'a str, limit: i64) -> Self {
        Self {
            entity_type: "match",
            entity_id: match_id,
            limit,
        }
    }
}
