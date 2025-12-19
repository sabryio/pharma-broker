//! Repository module - Data access layer
//!
//! Ported from legacy/domain/repository/repository.go

pub mod postgres;
mod traits;

pub use postgres::{PostgresMatchRepo, PostgresOfferRepo, PostgresRequestRepo, create_pool};
pub use traits::*;
