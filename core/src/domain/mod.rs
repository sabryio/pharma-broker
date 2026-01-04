//! Domain entities and types
//!
//! Uses pharma_db entity types directly via type aliases.

// Retained domain modules with additional utility types
mod match_entity;
mod stats;
mod types;

// Re-export pharma_db entity types as domain types
pub use pharma_db::entity::audit_log::Model as AuditLog;
pub use pharma_db::entity::feedback_record::Model as FeedbackRecord;
pub use pharma_db::entity::group::Model as Group;
pub use pharma_db::entity::match_::MatchStatus;
pub use pharma_db::entity::match_::Model as Match;
pub use pharma_db::entity::match_queue::Model as MatchQueueItem;
pub use pharma_db::entity::match_queue::QueueStatus as MatchQueueStatus;
pub use pharma_db::entity::medication_master::Model as MedicationMaster;
pub use pharma_db::entity::offer::Model as Offer;
pub use pharma_db::entity::offer::Status as ItemStatus;
pub use pharma_db::entity::offer::UrgencyLevel;
pub use pharma_db::entity::participant::Model as Participant;
pub use pharma_db::entity::raw_message::Model as RawMessage;
pub use pharma_db::entity::request::Model as Request;
pub use pharma_db::entity::review_queue::Model as ReviewQueueItem;
pub use pharma_db::entity::review_queue::ReviewStatus;
pub use pharma_db::entity::weight_history::Model as WeightHistory;
pub use pharma_db::traits::{FeedbackStats, ReviewQueueStats};

// Re-export additional domain types from retained modules
pub use match_entity::MatchWithDetails;
pub use pharma_db::audit_types::{AuditAction, EntityType};
pub use stats::Stats;
pub use types::{ConfidenceBand, FeedbackDecision, MessageType};
