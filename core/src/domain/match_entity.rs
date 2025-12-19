//! Match entity
//!
//! Ported from legacy/domain/entity/entity.go:131-149

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{MatchStatus, Offer, Request};

/// Represents a potential or confirmed match between offer and request
/// Ported from Go: Match struct (entity.go:131-142)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Match {
    pub id: String,
    pub offer_id: String,
    pub request_id: String,
    pub score: f64,
    pub reasoning: String,
    pub matched_by: Option<String>,
    pub status: MatchStatus,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

impl Default for Match {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            offer_id: String::new(),
            request_id: String::new(),
            score: 0.0,
            reasoning: String::new(),
            matched_by: None,
            status: MatchStatus::Pending,
            created_at: Utc::now(),
            confirmed_at: None,
            notes: None,
        }
    }
}

/// Match with full offer and request details for display
/// Ported from Go: MatchWithDetails struct (entity.go:145-149)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchWithDetails {
    #[serde(flatten)]
    pub match_info: Match,
    pub offer: Option<Offer>,
    pub request: Option<Request>,
}
