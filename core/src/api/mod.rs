//! API module - REST endpoints using axum
//!
//! Ported from legacy/api/handlers/*.go

pub mod audit_trail;
pub mod calibration;
pub mod confidence;
pub mod curation;
pub mod diagnostics;
pub mod embedding_cache;
pub mod groups;
pub mod handlers;
pub mod match_filter;
pub mod match_reviews;
pub mod matching;
pub mod middleware;
pub mod rate_limit;
pub mod reclassify;
pub mod reparse;
pub mod review_queue;
pub mod routes;
pub mod weights;

pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use routes::create_router;
