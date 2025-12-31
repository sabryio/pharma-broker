//! # PharmaBroker Database Layer
//!
//! Type-safe database operations using SeaORM.
//!
//! ## Structure
//! - `entity/` - SeaORM entity definitions
//! - `migration/` - Database migrations
//! - `service/` - Business logic services with tests
//! - `traits/` - Repository trait definitions
//! - `repo/` - Repository implementations
//!
//! ## Usage
//! ```rust,ignore
//! use pharma_db::{Database, repo::SeaOrmGroupRepo, traits::GroupRepository};
//!
//! let db = Database::connect("postgres://...").await?;
//! let repo = SeaOrmGroupRepo::new(db);
//! let groups = repo.get_monitored().await?;
//! ```

pub mod audit_types;
pub mod diagnostics;
pub mod entity;
pub mod feedback_params;
pub mod maintenance;
pub mod migration;
pub mod params;
pub mod repo;
pub mod service;
pub mod traits;

#[cfg(all(test, feature = "integration-tests"))]
pub mod testing;

use std::sync::Arc;

pub use migration::run_migrations;
pub use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
pub use pgvector::Vector;

// Export diagnostics types
pub use diagnostics::{DatabaseHealth, DbDiagnostics, IndexStats, QueryPlanAnalysis, TableStats};

/// Re-export common types
pub mod prelude {
    pub use super::entity::{
        AuditLog, FeedbackRecord, Group, Match, MatchQueue, MedicationMapping, Offer, RawMessage,
        Request, ReviewQueue, WeightHistory,
    };
    pub use super::entity::{
        GroupJid, MatchId, MedicationMappingId, OfferId, RawMessageId, RequestId, UserJid,
    };
    pub use super::entity::{ItemStatus, MatchStatus, UrgencyLevel};
    pub use super::feedback_params::{CreateFeedbackParams, FeedbackScores, RecordFeedbackParams};
    pub use super::service::{
        AuditLogService, FeedbackService, GroupService, MatchQueueService, MatchService,
        MedicationMappingService, OfferService, RawMessageService, RequestService,
        ReviewQueueService, WeightHistoryService,
    };
    pub use sea_orm::{
        ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
        QueryOrder, QuerySelect,
    };
}

/// Custom error type for database operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Duplicate entity: {0}")]
    Duplicate(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Create a SeaORM database connection
pub async fn create_connection(database_url: &str) -> Result<Arc<DatabaseConnection>> {
    use sea_orm::ConnectOptions;
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.sqlx_logging(false);
    Database::connect(opt)
        .await
        .map_err(Error::Database)
        .map(Arc::new)
}
