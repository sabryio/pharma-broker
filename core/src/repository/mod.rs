//! Repository module - Data access layer
//!
//! Ported from legacy/domain/repository/repository.go

pub mod postgres;
mod traits;

pub use postgres::{
    PostgresGroupRepo, PostgresMatchRepo, PostgresOfferRepo, PostgresRawMessageRepo,
    PostgresRequestRepo, create_pool,
};
pub use traits::*;
