//! Domain entities and types
//!
//! Ported from legacy/domain/entity/entity.go

mod match_entity;
mod message;
mod offer;
mod request;
mod stats;
mod types;

pub use match_entity::{Match, MatchWithDetails};
pub use message::RawMessage;
pub use offer::Offer;
pub use request::Request;
pub use stats::Stats;
pub use types::*;
