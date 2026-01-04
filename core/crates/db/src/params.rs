//! Repository parameter structs
//!
//! Provides descriptive structs for repository methods that have multiple arguments,
//! following best Rust practices for API design.

use chrono::Duration;
use uuid::Uuid;

use super::entity::common::MatchStatus;
use super::entity::review_queue::ReviewStatus;

/// Parameters for updating match status
#[derive(Debug, Clone)]
pub struct UpdateMatchStatusParams {
    /// Match ID to update
    pub id: Uuid,
    /// New status
    pub status: MatchStatus,
    /// Who performed the match (operator ID or "AUTO")
    pub matched_by: String,
    /// Optional notes about the status change
    pub notes: String,
}

impl UpdateMatchStatusParams {
    pub fn new(
        id: Uuid,
        status: MatchStatus,
        matched_by: impl Into<String>,
        notes: impl Into<String>,
    ) -> Self {
        Self {
            id,
            status,
            matched_by: matched_by.into(),
            notes: notes.into(),
        }
    }

    /// Create params for auto-confirmation
    pub fn auto_confirm(id: Uuid) -> Self {
        Self {
            id,
            status: MatchStatus::Confirmed,
            matched_by: "AUTO".to_string(),
            notes: "Auto-confirmed by matching engine".to_string(),
        }
    }
}

/// Parameters for updating review queue status
#[derive(Debug, Clone)]
pub struct UpdateReviewStatusParams {
    /// Review item ID
    pub id: Uuid,
    /// New status
    pub status: ReviewStatus,
    /// Who reviewed the item
    pub reviewed_by: String,
    /// Optional notes
    pub notes: Option<String>,
}

impl UpdateReviewStatusParams {
    pub fn new(
        id: Uuid,
        status: ReviewStatus,
        reviewed_by: impl Into<String>,
        notes: Option<String>,
    ) -> Self {
        Self {
            id,
            status,
            reviewed_by: reviewed_by.into(),
            notes,
        }
    }

    /// Create approve params
    pub fn approve(id: Uuid, reviewed_by: impl Into<String>) -> Self {
        Self {
            id,
            status: ReviewStatus::Approved,
            reviewed_by: reviewed_by.into(),
            notes: None,
        }
    }

    /// Create reject params with reason
    pub fn reject(id: Uuid, reviewed_by: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            id,
            status: ReviewStatus::Rejected,
            reviewed_by: reviewed_by.into(),
            notes: Some(reason.into()),
        }
    }
}

/// Parameters for finding recent duplicates
#[derive(Debug, Clone)]
pub struct FindDuplicateParams<'a> {
    /// Participant ID to match
    pub participant_id: Uuid,
    /// Medication name to match
    pub medication: &'a str,
    /// Time window to search within
    pub within: Duration,
}

impl<'a> FindDuplicateParams<'a> {
    pub fn new(participant_id: Uuid, medication: &'a str, within: Duration) -> Self {
        Self {
            participant_id,
            medication,
            within,
        }
    }

    /// Create with default 24-hour window
    pub fn within_day(participant_id: Uuid, medication: &'a str) -> Self {
        Self {
            participant_id,
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

// ============================================================================
// Raw Message Query Parameters
// ============================================================================

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Processing status filter for raw messages
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStatus {
    /// Return all messages regardless of processing status
    #[default]
    All,
    /// Return only successfully processed messages (processed_at is set, no error)
    Processed,
    /// Return only unprocessed messages (processed_at is null)
    Unprocessed,
    /// Return only messages with processing errors
    Error,
}

/// Sort field for raw messages
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RawMessageSortField {
    /// Sort by message timestamp (when the message was sent)
    #[default]
    Timestamp,
    /// Sort by when the message was processed
    ProcessedAt,
    /// Sort by when the record was created in the database
    CreatedAt,
}

/// Sort order
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Ascending order (oldest first for dates)
    Asc,
    /// Descending order (newest first for dates)
    #[default]
    Desc,
}

/// Query parameters for listing raw messages with filtering, sorting, and pagination
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawMessageQueryParams {
    /// Maximum number of results to return (default: 20, max: 100)
    pub limit: Option<i64>,
    /// Number of results to skip for pagination (default: 0)
    pub offset: Option<i64>,
    /// Search term to filter by message content (case-insensitive)
    pub search: Option<String>,
    /// Filter by processing status
    pub status: Option<ProcessingStatus>,
    /// Field to sort by
    pub sort_by: Option<RawMessageSortField>,
    /// Sort direction
    pub sort_order: Option<SortOrder>,
    /// Filter messages with timestamp >= start_date
    pub start_date: Option<DateTime<Utc>>,
    /// Filter messages with timestamp <= end_date
    pub end_date: Option<DateTime<Utc>>,
    /// Filter by group ID
    pub group_id: Option<Uuid>,
    /// Filter by participant ID
    pub participant_id: Option<Uuid>,
}

impl RawMessageQueryParams {
    /// Create new query params with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Get limit with default and max bounds
    pub fn get_limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }

    /// Get offset with default
    pub fn get_offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }

    /// Get sort field with default
    pub fn get_sort_field(&self) -> RawMessageSortField {
        self.sort_by.unwrap_or_default()
    }

    /// Get sort order with default
    pub fn get_sort_order(&self) -> SortOrder {
        self.sort_order.unwrap_or_default()
    }

    /// Get processing status filter with default
    pub fn get_status(&self) -> ProcessingStatus {
        self.status.unwrap_or_default()
    }

    /// Builder: set limit
    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Builder: set offset
    pub fn with_offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Builder: set search term
    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    /// Builder: set status filter
    pub fn with_status(mut self, status: ProcessingStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Builder: set sort field
    pub fn with_sort_by(mut self, sort_by: RawMessageSortField) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    /// Builder: set sort order
    pub fn with_sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }

    /// Builder: set date range
    pub fn with_date_range(
        mut self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Self {
        self.start_date = start;
        self.end_date = end;
        self
    }

    /// Builder: set group filter
    pub fn with_group(mut self, group_id: Uuid) -> Self {
        self.group_id = Some(group_id);
        self
    }

    /// Builder: set participant filter
    pub fn with_participant(mut self, participant_id: Uuid) -> Self {
        self.participant_id = Some(participant_id);
        self
    }
}
