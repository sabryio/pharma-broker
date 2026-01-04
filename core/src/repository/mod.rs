//! Repository module - Data access layer
//!
//! Uses pharma_db (SeaORM) for database operations.
//!
//! ## Usage
//! ```rust,ignore
//! use pharma_core::repository::{Database, GroupRepository, SeaOrmGroupRepo};
//!
//! let db = Database::connect("postgres://...").await?;
//! let repo = SeaOrmGroupRepo::new(db);
//! let groups = repo.get_monitored().await?;
//! ```

// Re-export pharma_db
pub use pharma_db::maintenance::{MaintenanceRepositories, MaintenanceService, PruneReport};
pub use pharma_db::migration;

// Convenience re-exports from pharma_db
pub use pharma_db::{Database, DatabaseConnection};

// Re-export SeaORM repos
pub use pharma_db::repo::{
    SeaOrmAuditLogRepo, SeaOrmFeedbackRepo, SeaOrmGroupRepo, SeaOrmMatchAuditRecordRepo,
    SeaOrmMatchQueueRepo, SeaOrmMatchRepo, SeaOrmMedicationAliasRepo, SeaOrmMedicationMasterRepo,
    SeaOrmOfferRepo, SeaOrmParticipantRepo, SeaOrmRawMessageRepo, SeaOrmRequestRepo,
    SeaOrmReviewQueueRepo, SeaOrmWeightHistoryRepo,
};

// Re-export pharma_db traits
pub use pharma_db::traits::{
    AuditLogRepository, EnrichedReviewItem, FeedbackRepository, FeedbackStats, GroupRepository,
    MatchAuditRecordRepository, MatchQueueRepository, MatchRepository, MedicationAliasRepository,
    MedicationMasterRepository, OfferRepository, ParticipantRepository, RawMessageRepository,
    RequestRepository, ReviewQueueRepository, ReviewQueueStats, WeightHistoryRepository,
};

// Re-export entity types as type aliases for domain compatibility
pub use pharma_db::traits::{
    AuditLogModel, CurationStats, FeedbackModel, GroupModel, MatchAuditRecordModel, MatchModel,
    MatchQueueModel, MedicationAliasModel, MedicationMasterModel, OfferModel, ParticipantModel,
    RawMessageModel, RequestModel, ReviewQueueModel, WeightHistoryModel,
};

// Re-export enums
pub use pharma_db::traits::{
    CurationStatus, ItemStatus, MatchStatus, MedicationStatus, QueueStatus, ReviewStatus,
    UrgencyLevel,
};

// Re-export ID newtypes for type-safe entity references
pub use pharma_db::traits::{MatchId, OfferId, RawMessageId, RequestId};

pub use pharma_db::params::{
    AuditByEntityParams, FindDuplicateParams, SemanticDuplicateParams, UpdateMatchStatusParams,
    UpdateReviewStatusParams,
};

// Re-export db create_connection function
pub use pharma_db::create_connection;
