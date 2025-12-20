//! API module - REST endpoints using axum
//!
//! Ported from legacy/api/handlers/*.go

pub mod groups;
pub mod handlers;
pub mod rate_limit;
pub mod review_queue;
pub mod routes;
pub mod weights;

pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use routes::create_router;
