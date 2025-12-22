//! SeaORM Entity Definitions
//!
//! Each entity maps to a database table with type-safe columns and relations.

pub mod audit_log;
pub mod feedback_record;
pub mod group;
pub mod match_;
pub mod match_queue;
pub mod medication_mapping;
pub mod offer;
pub mod raw_message;
pub mod request;
pub mod review_queue;
pub mod weight_history;

pub use audit_log::Entity as AuditLog;
pub use feedback_record::Entity as FeedbackRecord;
pub use group::Entity as Group;
pub use match_::Entity as Match;
pub use match_queue::Entity as MatchQueue;
pub use medication_mapping::Entity as MedicationMapping;
pub use offer::Entity as Offer;
pub use raw_message::Entity as RawMessage;
pub use request::Entity as Request;
pub use review_queue::Entity as ReviewQueue;
pub use weight_history::Entity as WeightHistory;
