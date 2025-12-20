//! PostgreSQL repository implementations
//!
//! Uses sqlx for async database operations

mod audit_log;
mod feedback;
mod group;
mod match_queue;
mod match_repo;
mod medication_mapping;
mod offer;
mod raw_message;
mod request;
mod review_queue;
mod stats;
mod weight_history;

#[cfg(test)]
pub mod testing;

// Integration tests - mirrors legacy/storage/gorm/*_test.go
#[cfg(test)]
mod audit_log_test;
#[cfg(test)]
mod feedback_test;
#[cfg(test)]
mod group_test;
#[cfg(test)]
mod match_repo_test;
#[cfg(test)]
mod medication_mapping_test;
#[cfg(test)]
mod offer_test;
#[cfg(test)]
mod raw_message_test;
#[cfg(test)]
mod request_test;
#[cfg(test)]
mod review_queue_test;
#[cfg(test)]
mod stats_test;
#[cfg(test)]
mod weight_history_test;

pub use audit_log::PostgresAuditLogRepo;
pub use feedback::PostgresFeedbackRepo;
pub use group::PostgresGroupRepo;
pub use match_queue::PostgresMatchQueueRepo;
pub use match_repo::PostgresMatchRepo;
pub use medication_mapping::PostgresMedicationMappingRepo;
pub use offer::PostgresOfferRepo;
pub use raw_message::PostgresRawMessageRepo;
pub use request::PostgresRequestRepo;
pub use review_queue::PostgresReviewQueueRepo;
pub use stats::PostgresStatsRepo;
pub use weight_history::PostgresWeightHistoryRepo;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Create a PostgreSQL connection pool
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
