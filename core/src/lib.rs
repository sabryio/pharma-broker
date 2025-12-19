//! PharmaBroker Core Engine
//!
//! A high-performance medication matching engine written in Rust.

pub mod ai;
pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod grpc;
pub mod matching;
pub mod metrics;
pub mod repository;

pub use error::{Error, Result};
