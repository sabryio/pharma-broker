//! Repository implementations using SeaORM
//!
//! These implement the traits defined in `crate::traits`.

mod audit_log;
mod auto_approve_config;
mod feedback;
mod group;
mod match_audit_record;
mod match_queue;
mod match_repo;
mod medication_alias;
mod medication_master;
mod offer;
mod participant;
mod raw_message;
mod request;
mod review_queue;
mod weight_history;

pub use audit_log::SeaOrmAuditLogRepo;
pub use auto_approve_config::SeaOrmAutoApproveConfigRepo;
pub use feedback::SeaOrmFeedbackRepo;
pub use group::SeaOrmGroupRepo;
pub use match_audit_record::SeaOrmMatchAuditRecordRepo;
pub use match_queue::SeaOrmMatchQueueRepo;
pub use match_repo::SeaOrmMatchRepo;
pub use medication_alias::{SeaOrmMedicationAliasRepo, normalize_arabic_text};
pub use medication_master::SeaOrmMedicationMasterRepo;
pub use offer::SeaOrmOfferRepo;
pub use participant::SeaOrmParticipantRepo;
pub use raw_message::SeaOrmRawMessageRepo;
pub use request::SeaOrmRequestRepo;
pub use review_queue::SeaOrmReviewQueueRepo;
pub use weight_history::SeaOrmWeightHistoryRepo;
