//! Convex client module
//!
//! Provides repository implementations backed by Convex instead of PostgreSQL.

mod client;
mod error;

// Repository implementations
mod audit_logs;
mod feedback;
mod groups;
mod match_queue;
mod match_repo;
mod medication_mappings;
mod offers;
mod raw_messages;
mod requests;
mod review_queue;
mod weight_history;

// Re-exports
pub use client::ConvexClient;
pub use error::ConvexError;

// Repository re-exports
pub use audit_logs::ConvexAuditLogRepo;
pub use feedback::ConvexFeedbackRepo;
pub use groups::ConvexGroupRepo;
pub use match_queue::ConvexMatchQueueRepo;
pub use match_repo::ConvexMatchRepo;
pub use medication_mappings::ConvexMedicationMappingRepo;
pub use offers::ConvexOfferRepo;
pub use raw_messages::ConvexRawMessageRepo;
pub use requests::ConvexRequestRepo;
pub use review_queue::ConvexReviewQueueRepo;
pub use weight_history::ConvexWeightHistoryRepo;
