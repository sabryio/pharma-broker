//! Repository traits
//!
//! Type-safe repository interfaces for database operations.
//! These traits are designed to be compatible with the core domain types.
//!
//! ## ID Newtypes
//! Repository methods accept `&str` for ID parameters to maintain dyn-compatibility.
//! Call sites can use ID newtypes (OfferId, RequestId, etc.) by calling `.as_ref()`
//! or `.as_str()` when passing them to repository methods.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::params::{
    AuditByEntityParams, FindDuplicateParams, SemanticDuplicateParams, UpdateMatchStatusParams,
    UpdateReviewStatusParams,
};

// Re-export ID newtypes for consumers
pub use crate::entity::{GroupJid, MatchId, MedicationMappingId, OfferId, RawMessageId, RequestId};

// Re-export entity types for consumers
pub use crate::entity::audit_log::Model as AuditLogModel;
pub use crate::entity::feedback_record::Model as FeedbackModel;
pub use crate::entity::group::Model as GroupModel;
pub use crate::entity::match_::Model as MatchModel;
pub use crate::entity::match_queue::Model as MatchQueueModel;
pub use crate::entity::match_queue::QueueStatus;
pub use crate::entity::medication_mapping::Model as MedicationMappingModel;
pub use crate::entity::offer::Model as OfferModel;
pub use crate::entity::raw_message::Model as RawMessageModel;
pub use crate::entity::request::Model as RequestModel;
pub use crate::entity::review_queue::Model as ReviewQueueModel;
pub use crate::entity::review_queue::ReviewStatus;
pub use crate::entity::weight_history::Model as WeightHistoryModel;

// Re-export common types from centralized location
pub use crate::entity::common::{ItemStatus, MatchStatus, UrgencyLevel};

/// Offer repository trait
#[async_trait]
pub trait OfferRepository: Send + Sync {
    /// Get an offer by its ID
    async fn get_by_id(&self, id: &str) -> Result<Option<OfferModel>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<OfferModel>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<OfferModel>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<OfferModel>>;
    async fn save(&self, offer: &OfferModel) -> Result<OfferModel>;
    /// Update the status of an offer by its ID
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<OfferModel>;
    async fn find_semantic_duplicates(
        &self,
        params: SemanticDuplicateParams<'_>,
    ) -> Result<Vec<OfferModel>>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Request repository trait
#[async_trait]
pub trait RequestRepository: Send + Sync {
    /// Get a request by its ID
    async fn get_by_id(&self, id: &str) -> Result<Option<RequestModel>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<RequestModel>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<RequestModel>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<RequestModel>>;
    async fn save(&self, request: &RequestModel) -> Result<RequestModel>;
    /// Update the status of a request by its ID
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<RequestModel>;
    async fn find_semantic_duplicates(
        &self,
        params: SemanticDuplicateParams<'_>,
    ) -> Result<Vec<RequestModel>>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Match repository trait
#[async_trait]
pub trait MatchRepository: Send + Sync {
    /// Get a match by its ID
    async fn get_by_id(&self, id: &str) -> Result<Option<MatchModel>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<MatchModel>>;
    async fn count_pending(&self) -> Result<i64>;
    /// Check if a match exists between an offer and request
    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool>;
    async fn save(&self, m: &MatchModel) -> Result<MatchModel>;
    async fn update_status(&self, params: UpdateMatchStatusParams<'_>) -> Result<MatchModel>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Raw message repository trait
#[async_trait]
pub trait RawMessageRepository: Send + Sync {
    async fn save(&self, message: &RawMessageModel) -> Result<RawMessageModel>;
    /// Get a raw message by its ID
    async fn get_by_id(&self, id: &str) -> Result<Option<RawMessageModel>>;
    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessageModel>>;
    /// Mark a message as processed
    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<RawMessageModel>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Group repository trait
#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn get_all(&self) -> Result<Vec<GroupModel>>;
    /// Get a group by its JID
    async fn get_by_jid(&self, jid: &str) -> Result<Option<GroupModel>>;
    /// Check if a group is monitored
    async fn is_monitored(&self, jid: &str) -> Result<bool>;
    async fn get_monitored(&self) -> Result<Vec<GroupModel>>;
    async fn save(&self, group: &GroupModel) -> Result<GroupModel>;
    /// Update the monitored status of a group
    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()>;
    /// Delete a group by its JID
    async fn delete(&self, jid: &str) -> Result<bool>;
    /// Update the last message timestamp for a group
    async fn update_last_message(&self, jid: &str) -> Result<()>;
    /// Increment the message count for a group
    async fn increment_message_count(&self, jid: &str) -> Result<()>;
}

/// Feedback record repository trait
#[async_trait]
pub trait FeedbackRepository: Send + Sync {
    async fn save(&self, record: &FeedbackModel) -> Result<FeedbackModel>;
    /// Get feedback records for a match
    async fn get_by_match(&self, match_id: &str) -> Result<Vec<FeedbackModel>>;
    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FeedbackModel>>;
    async fn get_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<FeedbackStats>;
    async fn count(&self) -> Result<i64>;
    /// Get a single feedback record by match ID
    async fn get_by_match_id(&self, match_id: &str) -> Result<Option<FeedbackModel>>;
}

/// Feedback statistics for learning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedbackStats {
    pub total_feedback: i64,
    pub confirmed_count: i64,
    pub rejected_count: i64,
    pub avg_confirmed_score: f64,
    pub avg_rejected_score: f64,
    // Extended fields for weight learning
    pub confirmation_rate: f64,
    pub confirmed_avg_medication: f64,
    pub rejected_avg_medication: f64,
    pub medication_diff: f64,
    pub confirmed_avg_dosage: f64,
    pub rejected_avg_dosage: f64,
    pub dosage_diff: f64,
    pub confirmed_avg_quantity: f64,
    pub rejected_avg_quantity: f64,
    pub quantity_diff: f64,
    pub confirmed_avg_price: f64,
    pub rejected_avg_price: f64,
    pub price_diff: f64,
    pub confirmed_avg_recency: f64,
    pub rejected_avg_recency: f64,
    pub recency_diff: f64,
    pub confirmed_avg_total: f64,
    pub rejected_avg_total: f64,
}

/// Weight history repository trait
#[async_trait]
pub trait WeightHistoryRepository: Send + Sync {
    async fn save(&self, history: &WeightHistoryModel) -> Result<WeightHistoryModel>;
    async fn get_current(&self) -> Result<Option<WeightHistoryModel>>;
    async fn get_history(&self, limit: i64) -> Result<Vec<WeightHistoryModel>>;
    /// Get weight history by ID
    async fn get_by_id(&self, id: &str) -> Result<Option<WeightHistoryModel>>;
    async fn count(&self) -> Result<i64>;
}

/// Review queue repository trait
#[async_trait]
pub trait ReviewQueueRepository: Send + Sync {
    async fn save(&self, item: &ReviewQueueModel) -> Result<ReviewQueueModel>;
    /// Get a review queue item by ID
    async fn get_by_id(&self, id: &str) -> Result<Option<ReviewQueueModel>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<ReviewQueueModel>>;
    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewQueueModel>>;
    async fn update_status(&self, params: UpdateReviewStatusParams<'_>)
    -> Result<ReviewQueueModel>;
    async fn get_stats(&self) -> Result<ReviewQueueStats>;
    async fn count_pending(&self) -> Result<i64>;
    /// Check if a review queue item exists for a message
    async fn exists_for_message(&self, raw_message_id: &str) -> Result<bool>;
}

/// Review queue statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewQueueStats {
    pub pending: i64,
    pub approved: i64,
    pub rejected: i64,
    pub skipped: i64,
}

/// Medication mapping repository trait
#[async_trait]
pub trait MedicationMappingRepository: Send + Sync {
    async fn save(&self, mapping: &MedicationMappingModel) -> Result<MedicationMappingModel>;
    async fn find_relevant(&self, query: &str, limit: i64) -> Result<Vec<MedicationMappingModel>>;
    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<MedicationMappingModel>>;
    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<MedicationMappingModel>>;
    async fn count(&self) -> Result<i64>;
    /// Get mappings that need embeddings (embedding is NULL)
    async fn get_needing_embeddings(&self, limit: i64) -> Result<Vec<MedicationMappingModel>>;
    /// Count mappings that need embeddings
    async fn count_needing_embeddings(&self) -> Result<i64>;
}

/// Audit log repository trait
#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn save(&self, log: &AuditLogModel) -> Result<AuditLogModel>;
    async fn get_by_entity(&self, params: AuditByEntityParams<'_>) -> Result<Vec<AuditLogModel>>;
    async fn get_by_actor(&self, actor: &str, limit: i64) -> Result<Vec<AuditLogModel>>;
    async fn get_by_action(&self, action: &str, limit: i64) -> Result<Vec<AuditLogModel>>;
    async fn get_recent(&self, limit: i64, offset: i64) -> Result<Vec<AuditLogModel>>;
    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AuditLogModel>>;
    async fn count(&self) -> Result<i64>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Match queue repository trait
#[async_trait]
pub trait MatchQueueRepository: Send + Sync {
    /// Enqueue a request for matching
    async fn enqueue(&self, request_id: &str, priority: i32) -> Result<MatchQueueModel>;
    async fn fetch_batch(&self, limit: i64) -> Result<Vec<MatchQueueModel>>;
    async fn complete(&self, id: &uuid::Uuid) -> Result<()>;
    async fn fail(&self, id: &uuid::Uuid, error: &str) -> Result<()>;
    async fn count_pending(&self) -> Result<i64>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}
