//! Match entity extensions
//!
//! Provides additional types for match display and aggregation.
//! The core Match type is defined in pharma_db::entity::match_.

use serde::{Deserialize, Serialize};

use super::{Match, Offer, Request};

/// Match with full offer and request details for display
///
/// Used when returning match data to the UI with related entities.
/// Ported from Go: MatchWithDetails struct (entity.go:145-149)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchWithDetails {
    /// The match entity
    #[serde(flatten)]
    pub match_info: Match,
    /// The associated offer (if loaded)
    pub offer: Option<Offer>,
    /// The associated request (if loaded)
    pub request: Option<Request>,
}

impl MatchWithDetails {
    /// Create a new MatchWithDetails from a match
    pub fn new(match_info: Match) -> Self {
        Self {
            match_info,
            offer: None,
            request: None,
        }
    }

    /// Create with offer and request
    pub fn with_details(match_info: Match, offer: Option<Offer>, request: Option<Request>) -> Self {
        Self {
            match_info,
            offer,
            request,
        }
    }

    /// Get the match score
    pub fn score(&self) -> f64 {
        self.match_info.score
    }

    /// Get the match ID
    pub fn id(&self) -> &str {
        &self.match_info.id
    }

    /// Check if both offer and request are loaded
    pub fn has_full_details(&self) -> bool {
        self.offer.is_some() && self.request.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_with_details_new() {
        let m = Match::default();
        let details = MatchWithDetails::new(m);
        assert!(details.offer.is_none());
        assert!(details.request.is_none());
        assert!(!details.has_full_details());
    }
}
