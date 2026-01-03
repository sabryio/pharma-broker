//! Repository traits
//!
//! Type-safe repository interfaces for database operations.
//! These traits are designed to be compatible with the core domain types.
//!
//! ## ID Types
//! Repository methods accept `Uuid` for ID parameters where entities use UUID primary keys.
//! For entities with string IDs (like Group with JID), methods accept `&str`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub use crate::entity::medication_alias::CurationStatus;
pub use crate::entity::medication_alias::Model as MedicationAliasModel;
pub use crate::entity::medication_mapping::Model as MedicationMappingModel;
pub use crate::entity::medication_master::MedicationStatus;
pub use crate::entity::medication_master::Model as MedicationMasterModel;
pub use crate::entity::offer::Model as OfferModel;
pub use crate::entity::participant::Model as ParticipantModel;
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
    async fn get_by_id(&self, id: Uuid) -> Result<Option<OfferModel>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<OfferModel>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<OfferModel>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<OfferModel>>;
    async fn save(&self, offer: &OfferModel) -> Result<OfferModel>;
    /// Update the status of an offer by its ID
    async fn update_status(&self, id: Uuid, status: ItemStatus) -> Result<OfferModel>;
    /// Increment the confirmed match count for an offer
    async fn increment_match_count(&self, id: Uuid) -> Result<OfferModel>;
    /// Decrement the confirmed match count for an offer (for undo)
    async fn decrement_match_count(&self, id: Uuid) -> Result<OfferModel>;
    /// Update medication info after AI re-parse
    async fn update_medication(
        &self,
        id: Uuid,
        medication: &str,
        medication_raw: &str,
        ai_confidence: Option<f64>,
    ) -> Result<OfferModel>;
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
    async fn get_by_id(&self, id: Uuid) -> Result<Option<RequestModel>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<RequestModel>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<RequestModel>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<RequestModel>>;
    async fn save(&self, request: &RequestModel) -> Result<RequestModel>;
    /// Update the status of a request by its ID
    async fn update_status(&self, id: Uuid, status: ItemStatus) -> Result<RequestModel>;
    /// Increment the confirmed match count for a request
    async fn increment_match_count(&self, id: Uuid) -> Result<RequestModel>;
    /// Decrement the confirmed match count for a request (for undo)
    async fn decrement_match_count(&self, id: Uuid) -> Result<RequestModel>;
    /// Update medication info after AI re-parse
    async fn update_medication(
        &self,
        id: Uuid,
        medication: &str,
        medication_raw: &str,
        ai_confidence: Option<f64>,
    ) -> Result<RequestModel>;
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
    async fn get_by_id(&self, id: Uuid) -> Result<Option<MatchModel>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<MatchModel>>;
    /// Get all matches with optional status filter
    async fn get_all(
        &self,
        limit: i64,
        offset: i64,
        status: Option<MatchStatus>,
    ) -> Result<Vec<MatchModel>>;
    /// Count all matches with optional status filter
    async fn count_all(&self, status: Option<MatchStatus>) -> Result<i64>;
    async fn count_pending(&self) -> Result<i64>;
    /// Check if a match exists between an offer and request
    async fn exists(&self, offer_id: Uuid, request_id: Uuid) -> Result<bool>;
    async fn save(&self, m: &MatchModel) -> Result<MatchModel>;
    async fn update_status(&self, params: UpdateMatchStatusParams) -> Result<MatchModel>;
    /// Update AI review results for a match
    async fn update_ai_review(
        &self,
        id: Uuid,
        ai_status: &str,
        ai_confidence: f64,
        ai_explanation: &str,
    ) -> Result<MatchModel>;
    /// Update match score and reasoning
    async fn update_score(&self, id: Uuid, score: f64, reasoning: &str) -> Result<MatchModel>;
    /// Update match notes
    async fn update_notes(&self, id: Uuid, notes: &str) -> Result<MatchModel>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
    /// Cancel all pending matches that reference a specific offer (used when reclassifying)
    async fn cancel_matches_for_offer(&self, offer_id: Uuid) -> Result<u64>;
    /// Cancel all pending matches that reference a specific request (used when reclassifying)
    async fn cancel_matches_for_request(&self, request_id: Uuid) -> Result<u64>;
    /// Delete all pending matches for an offer
    async fn delete_pending_matches_for_offer(&self, offer_id: Uuid) -> Result<u64>;
    /// Delete all pending matches for a request
    async fn delete_pending_matches_for_request(&self, request_id: Uuid) -> Result<u64>;
    /// Count matches confirmed today
    async fn count_confirmed_today(&self) -> Result<i64>;
    /// Count matches rejected today
    async fn count_rejected_today(&self) -> Result<i64>;
    /// Get average score of pending matches
    async fn avg_pending_score(&self) -> Result<f64>;
}

/// Raw message repository trait
#[async_trait]
pub trait RawMessageRepository: Send + Sync {
    async fn save(&self, message: &RawMessageModel) -> Result<RawMessageModel>;
    /// Get a raw message by its ID
    async fn get_by_id(&self, id: Uuid) -> Result<Option<RawMessageModel>>;
    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessageModel>>;
    /// Mark a message as processed
    async fn mark_processed(&self, id: Uuid, error: Option<&str>) -> Result<RawMessageModel>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Group repository trait
#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn get_all(&self) -> Result<Vec<GroupModel>>;
    /// Get a group by its UUID
    async fn get_by_id(&self, id: Uuid) -> Result<Option<GroupModel>>;
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

/// Participant repository trait
#[async_trait]
pub trait ParticipantRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<crate::entity::participant::Model>>;
    async fn get_by_jid(&self, jid: &str) -> Result<Option<crate::entity::participant::Model>>;
    async fn get_by_phone(&self, phone: &str) -> Result<Option<crate::entity::participant::Model>>;
    async fn save(
        &self,
        participant: &crate::entity::participant::Model,
    ) -> Result<crate::entity::participant::Model>;
    async fn get_groups(&self, participant_id: Uuid) -> Result<Vec<GroupModel>>;
    async fn add_to_group(&self, participant_id: Uuid, group_id: Uuid) -> Result<()>;
    /// Get participant statistics (offers, requests, match rates)
    async fn get_stats(&self, participant_id: Uuid) -> Result<ParticipantStats>;
}

/// Participant statistics for sender profiles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantStats {
    pub participant_id: Uuid,
    pub total_offers: i64,
    pub total_requests: i64,
    pub confirmed_matches: i64,
    pub rejected_matches: i64,
    pub approval_rate: f64,
    pub avg_confidence: f64,
    pub last_activity: Option<DateTime<Utc>>,
    pub reputation: String, // "new", "regular", "trusted"
}

/// Feedback record repository trait
#[async_trait]
pub trait FeedbackRepository: Send + Sync {
    async fn save(&self, record: &FeedbackModel) -> Result<FeedbackModel>;
    /// Get feedback records for a match
    async fn get_by_match(&self, match_id: Uuid) -> Result<Vec<FeedbackModel>>;
    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FeedbackModel>>;
    async fn get_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<FeedbackStats>;
    async fn count(&self) -> Result<i64>;
    /// Get a single feedback record by match ID
    async fn get_by_match_id(&self, match_id: Uuid) -> Result<Option<FeedbackModel>>;
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
    pub confirmed_avg_ai_logic: f64,
    pub rejected_avg_ai_logic: f64,
    pub ai_logic_diff: f64,
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
    async fn get_by_id(&self, id: Uuid) -> Result<Option<WeightHistoryModel>>;
    async fn count(&self) -> Result<i64>;
}

/// Enriched review queue item with joined message data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedReviewItem {
    pub id: Uuid,
    pub raw_message_id: Uuid,
    pub ai_result: serde_json::Value,
    pub confidence: f64,
    pub reason: String,
    pub status: ReviewStatus,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    // Joined from raw_messages
    pub original_text: String,
    pub message_timestamp: DateTime<Utc>,
    // Joined from participants
    pub sender_name: Option<String>,
    pub sender_phone: Option<String>,
    // Joined from groups
    pub group_name: Option<String>,
}

/// Review queue repository trait
#[async_trait]
pub trait ReviewQueueRepository: Send + Sync {
    async fn save(&self, item: &ReviewQueueModel) -> Result<ReviewQueueModel>;
    /// Get a review queue item by ID
    async fn get_by_id(&self, id: Uuid) -> Result<Option<ReviewQueueModel>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<ReviewQueueModel>>;
    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewQueueModel>>;
    async fn update_status(&self, params: UpdateReviewStatusParams) -> Result<ReviewQueueModel>;
    async fn get_stats(&self) -> Result<ReviewQueueStats>;
    async fn count_pending(&self) -> Result<i64>;
    /// Check if a review queue item exists for a message
    async fn exists_for_message(&self, raw_message_id: Uuid) -> Result<bool>;
    /// Get pending items with joined message, participant, and group data
    async fn get_pending_enriched(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EnrichedReviewItem>>;
    /// Get a single enriched review item by ID
    async fn get_by_id_enriched(&self, id: Uuid) -> Result<Option<EnrichedReviewItem>>;
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
    async fn enqueue(&self, request_id: Uuid, priority: i32) -> Result<MatchQueueModel>;
    async fn fetch_batch(&self, limit: i64) -> Result<Vec<MatchQueueModel>>;
    async fn complete(&self, id: Uuid) -> Result<()>;
    async fn fail(&self, id: Uuid, error: &str) -> Result<()>;
    async fn count_pending(&self) -> Result<i64>;
    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64>;
}

/// Medication master repository trait
#[async_trait]
pub trait MedicationMasterRepository: Send + Sync {
    async fn save(&self, master: &MedicationMasterModel) -> Result<MedicationMasterModel>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<MedicationMasterModel>>;
    async fn find_by_name(&self, name: &str) -> Result<Option<MedicationMasterModel>>;
    async fn search(&self, name: &str, limit: i64) -> Result<Vec<MedicationMasterModel>>;
    /// Fuzzy search using trigram similarity (pg_trgm)
    async fn search_fuzzy(
        &self,
        name: &str,
        limit: i64,
        min_similarity: f32,
    ) -> Result<Vec<(MedicationMasterModel, f32)>>;
    async fn search_semantic(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(MedicationMasterModel, f32)>>;
    async fn count(&self) -> Result<i64>;
}

/// Medication alias repository trait
#[async_trait]
pub trait MedicationAliasRepository: Send + Sync {
    async fn save(&self, alias: &MedicationAliasModel) -> Result<MedicationAliasModel>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<MedicationAliasModel>>;
    async fn get_by_name(&self, name: &str) -> Result<Option<MedicationAliasModel>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<MedicationAliasModel>>;
    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<MedicationAliasModel>>;
    async fn count_pending(&self) -> Result<i64>;
    async fn count_rejected(&self) -> Result<i64>;
    async fn count_all(&self) -> Result<i64>;
    async fn delete(&self, id: Uuid) -> Result<bool>;
    async fn get_stats(&self) -> Result<CurationStats>;
}

/// Curation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationStats {
    pub total_offers: i64,
    pub curated_offers: i64,
    pub master_medications: i64,
    pub total_aliases: i64,
    pub pending_aliases: i64,
    pub rejected_aliases: i64,
}

// ============================================================================
// Match Review Types
// ============================================================================

/// Summary of an offer for match review display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferSummary {
    pub id: Uuid,
    pub product: String,
    /// Original medication text for highlighting
    pub medication_raw: Option<String>,
    pub source: String,
    /// WhatsApp group name where the offer came from
    pub source_group: Option<String>,
    /// Sender's display name
    pub sender_name: Option<String>,
    /// Sender's WhatsApp JID
    pub sender_jid: Option<String>,
    /// Original raw message content
    pub raw_message: Option<String>,
    pub quantity: Option<String>,
    pub price: Option<String>,
    pub expiry: Option<String>,
    pub master_id: Option<Uuid>,
    pub medication_alias_id: Option<Uuid>,
    pub curation_status: Option<String>,
}

/// Summary of a request for match review display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    pub id: Uuid,
    pub product: String,
    /// Original medication text for highlighting
    pub medication_raw: Option<String>,
    pub source: String,
    /// WhatsApp group name where the request came from
    pub source_group: Option<String>,
    /// Sender's display name
    pub sender_name: Option<String>,
    /// Sender's WhatsApp JID
    pub sender_jid: Option<String>,
    /// Original raw message content
    pub raw_message: Option<String>,
    pub quantity: Option<String>,
    pub max_price: Option<String>,
    pub urgency: String,
    pub master_id: Option<Uuid>,
    pub medication_alias_id: Option<Uuid>,
    pub curation_status: Option<String>,
}

/// Enriched match review item with joined offer and request data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReviewItem {
    pub id: Uuid,
    pub confidence: f64,
    pub status: MatchStatus,
    pub reasoning: Option<String>,
    pub issues: Vec<String>,
    pub offer: OfferSummary,
    pub request: RequestSummary,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub ai_status: Option<String>,
    pub ai_confidence: Option<f64>,
    pub ai_explanation: Option<String>,
}

/// Match review statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReviewStats {
    pub pending: i64,
    pub confirmed_today: i64,
    pub rejected_today: i64,
    pub total_pending: i64,
    pub avg_confidence: f64,
}
