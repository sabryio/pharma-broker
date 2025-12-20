//! Repository module - Data access layer
//!
//! Ported from legacy/domain/repository/repository.go

pub mod postgres;
mod traits;

pub use postgres::{
    PostgresFeedbackRepo, PostgresGroupRepo, PostgresMatchRepo, PostgresOfferRepo,
    PostgresRawMessageRepo, PostgresRequestRepo, PostgresWeightHistoryRepo, create_pool,
};
pub use traits::*;
