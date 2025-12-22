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
pub use pharma_db;

// Convenience re-exports from pharma_db
pub use pharma_db::{Database, DatabaseConnection};

// Re-export SeaORM repos
pub use pharma_db::repo::{
    SeaOrmAuditLogRepo, SeaOrmFeedbackRepo, SeaOrmGroupRepo, SeaOrmMatchQueueRepo, SeaOrmMatchRepo,
    SeaOrmMedicationMappingRepo, SeaOrmOfferRepo, SeaOrmRawMessageRepo, SeaOrmRequestRepo,
    SeaOrmReviewQueueRepo, SeaOrmWeightHistoryRepo,
};

// Re-export pharma_db traits
pub use pharma_db::traits::{
    AuditLogRepository, FeedbackRepository, FeedbackStats, GroupRepository, MatchQueueRepository,
    MatchRepository, MedicationMappingRepository, OfferRepository, RawMessageRepository,
    RequestRepository, ReviewQueueRepository, ReviewQueueStats, WeightHistoryRepository,
};

// Re-export entity types as type aliases for domain compatibility
pub use pharma_db::traits::{
    AuditLogModel, FeedbackModel, GroupModel, MatchModel, MatchQueueModel, MedicationMappingModel,
    OfferModel, RawMessageModel, RequestModel, ReviewQueueModel, WeightHistoryModel,
};

// Re-export enums
pub use pharma_db::traits::{ItemStatus, MatchStatus, QueueStatus, ReviewStatus, UrgencyLevel};

// Re-export ID newtypes for type-safe entity references
pub use pharma_db::traits::{
    GroupJid, MatchId, MedicationMappingId, OfferId, RawMessageId, RequestId,
};

pub use pharma_db::params::{
    AuditByEntityParams, FindDuplicateParams, SemanticDuplicateParams, UpdateMatchStatusParams,
    UpdateReviewStatusParams,
};

/// Create a SeaORM database connection
pub async fn create_connection(database_url: &str) -> Result<DatabaseConnection, pharma_db::DbErr> {
    pharma_db::Database::connect(database_url).await
}
