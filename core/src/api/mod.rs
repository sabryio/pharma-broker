//! API module - REST endpoints using axum
//!
//! Ported from legacy/api/handlers/*.go

pub mod groups;
pub mod handlers;
pub mod routes;

pub use routes::create_router;
