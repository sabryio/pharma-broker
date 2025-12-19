//! Stats entity
//!
//! Ported from legacy/domain/entity/entity.go:162-171

use serde::{Deserialize, Serialize};

/// Dashboard statistics
/// Ported from Go: Stats struct (entity.go:162-171)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub active_offers: i64,
    pub active_requests: i64,
    pub pending_matches: i64,
    pub confirmed_today: i64,
    pub processed_today: i64,
    pub avg_match_score: f64,
    pub monitored_groups: i32,
    pub connected_clients: i32,
}
