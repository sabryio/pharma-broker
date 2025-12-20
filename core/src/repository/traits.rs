//! Repository traits
//!
//! Ported from legacy/domain/repository/repository.go

use async_trait::async_trait;
use chrono::Duration;

use crate::Result;
use crate::domain::{
    FeedbackRecord, FeedbackStats, Group, ItemStatus, Match, MatchStatus, Offer, RawMessage,
    Request, ReviewQueueItem, ReviewQueueStats, ReviewStatus, Stats, WeightHistory,
};

/// Offer repository trait
/// Ported from Go: OfferReader + OfferWriter (repository.go:13-32)
#[async_trait]
pub trait OfferRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Offer>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<Offer>>;
    async fn save(&self, offer: &Offer) -> Result<()>;
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()>;
}

/// Request repository trait
/// Ported from Go: RequestReader + RequestWriter (repository.go:35-52)
#[async_trait]
pub trait RequestRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Request>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Request>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Request>>;
    async fn count_active(&self) -> Result<i64>;
    async fn save(&self, request: &Request) -> Result<()>;
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()>;
}

/// Match repository trait
/// Ported from Go: MatchReader + MatchWriter (repository.go:55-80)
#[async_trait]
pub trait MatchRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Match>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<Match>>;
    async fn count_pending(&self) -> Result<i64>;
    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool>;
    async fn save(&self, match_entity: &Match) -> Result<()>;
    async fn update_status(
        &self,
        id: &str,
        status: MatchStatus,
        matched_by: &str,
        notes: &str,
    ) -> Result<()>;
}

/// Raw message repository trait
/// Ported from Go: RawMessageRepository (repository.go:83-95)
#[async_trait]
pub trait RawMessageRepository: Send + Sync {
    async fn save(&self, message: &RawMessage) -> Result<()>;
    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessage>>;
    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<()>;
}

/// Group repository trait
/// Ported from Go: GroupRepository (repository.go:98-110)
#[async_trait]
pub trait GroupRepository: Send + Sync {
    /// Get all groups
    async fn get_all(&self) -> Result<Vec<Group>>;
    /// Get group by JID
    async fn get_by_jid(&self, jid: &str) -> Result<Option<Group>>;
    /// Check if a group is monitored
    async fn is_monitored(&self, jid: &str) -> Result<bool>;
    /// Get all monitored groups
    async fn get_monitored(&self) -> Result<Vec<Group>>;
    /// Save or update a group
    async fn save(&self, group: &Group) -> Result<()>;
    /// Update monitoring status
    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()>;
    /// Delete a group
    async fn delete(&self, jid: &str) -> Result<bool>;
    /// Update last message timestamp
    async fn update_last_message(&self, jid: &str) -> Result<()>;
    /// Increment message count
    async fn increment_message_count(&self, jid: &str) -> Result<()>;
}

/// Stats repository trait
#[async_trait]
pub trait StatsRepository: Send + Sync {
    async fn get_stats(&self) -> Result<Stats>;
}

/// Feedback record repository trait
/// Used by the weight learning system to collect and aggregate feedback
#[async_trait]
pub trait FeedbackRecordRepository: Send + Sync {
    /// Save a new feedback record
    async fn save(&self, record: &FeedbackRecord) -> Result<()>;

    /// Get feedback records within a date range
    async fn get_by_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<FeedbackRecord>>;

    /// Get aggregated feedback statistics for learning
    async fn get_stats(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<FeedbackStats>;

    /// Count total feedback records
    async fn count(&self) -> Result<i64>;

    /// Get feedback by match ID
    async fn get_by_match_id(&self, match_id: &str) -> Result<Option<FeedbackRecord>>;
}

/// Weight history repository trait
/// Used for auditing and rollback of weight configurations
#[async_trait]
pub trait WeightHistoryRepository: Send + Sync {
    /// Save a new weight history entry
    async fn save(&self, history: &WeightHistory) -> Result<()>;

    /// Get the most recent weights
    async fn get_current(&self) -> Result<Option<WeightHistory>>;

    /// Get weight history with optional limit
    async fn get_history(&self, limit: i64) -> Result<Vec<WeightHistory>>;

    /// Get weights by ID (for rollback)
    async fn get_by_id(&self, id: &str) -> Result<Option<WeightHistory>>;

    /// Count total history entries
    async fn count(&self) -> Result<i64>;
}

/// Review queue repository trait
/// Used for managing AI parse results that require human review
#[async_trait]
pub trait ReviewQueueRepository: Send + Sync {
    /// Save a new review queue item
    async fn save(&self, item: &ReviewQueueItem) -> Result<()>;

    /// Get a review item by ID
    async fn get_by_id(&self, id: &str) -> Result<Option<ReviewQueueItem>>;

    /// Get pending items (paginated)
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<ReviewQueueItem>>;

    /// Get items by status (paginated)
    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewQueueItem>>;

    /// Update item status (approve/reject/skip)
    async fn update_status(
        &self,
        id: &str,
        status: ReviewStatus,
        reviewed_by: &str,
        notes: Option<&str>,
    ) -> Result<()>;

    /// Get review queue statistics
    async fn get_stats(&self) -> Result<ReviewQueueStats>;

    /// Count pending items
    async fn count_pending(&self) -> Result<i64>;

    /// Check if a raw message is already queued
    async fn exists_for_message(&self, raw_message_id: &str) -> Result<bool>;
}
