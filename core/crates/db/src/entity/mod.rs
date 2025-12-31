//! SeaORM Entity Definitions
//!
//! Each entity maps to a database table with type-safe columns and relations.
//!
//! ## Module Organization
//! - `common` - Shared types (enums, status types) used across entities
//! - `ids` - Strongly-typed ID newtypes for type-safe entity references
//! - Entity modules - Individual entity definitions

pub mod audit_log;
pub mod common;
pub mod feedback_record;
pub mod group;
pub mod ids;
pub mod match_;
pub mod match_queue;
pub mod medication_alias;
pub mod medication_mapping;
pub mod medication_master;
pub mod offer;
pub mod participant;
pub mod participant_group;
pub mod raw_message;
pub mod request;
pub mod review_queue;
pub mod weight_history;

// Re-export entity types
pub use audit_log::Entity as AuditLog;
pub use feedback_record::Entity as FeedbackRecord;
pub use group::Entity as Group;
pub use match_::Entity as Match;
pub use match_queue::Entity as MatchQueue;
pub use medication_alias::Entity as MedicationAlias;
pub use medication_mapping::Entity as MedicationMapping;
pub use medication_master::Entity as MedicationMaster;
pub use offer::Entity as Offer;
pub use participant::Entity as Participant;
pub use participant_group::Entity as ParticipantGroup;
pub use raw_message::Entity as RawMessage;
pub use request::Entity as Request;
pub use review_queue::Entity as ReviewQueue;
pub use weight_history::Entity as WeightHistory;

// Re-export common types for convenience
pub use common::{ItemStatus, MatchStatus, UrgencyLevel};
pub use ids::{GroupJid, MatchId, MedicationMappingId, OfferId, RawMessageId, RequestId, UserJid};
pub use medication_alias::CurationStatus;
pub use medication_master::MedicationStatus;
