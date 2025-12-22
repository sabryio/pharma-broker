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
pub mod entity;
pub mod migration;
pub mod params;
pub mod repo;
pub mod service;
pub mod traits;

#[cfg(test)]
pub mod testing;

pub use migration::run_migrations;
pub use sea_orm::{Database, DatabaseConnection, DbErr};

/// Re-export common types
pub mod prelude {
    pub use super::entity::{
        AuditLog, FeedbackRecord, Group, Match, MatchQueue, MedicationMapping, Offer, RawMessage,
        Request, ReviewQueue, WeightHistory,
    };
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
