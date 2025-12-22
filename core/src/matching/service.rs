//! Matching service trait
//!
//! Ported from legacy/matching/interface.go

use async_trait::async_trait;

use super::MatchScore;
use crate::domain::{Match, Offer, Request};

/// Matching service interface
/// Ported from Go: Service interface (interface.go:12-24)
#[async_trait]
pub trait MatchingService: Send + Sync {
    /// Find matching offers for a request
    async fn find_matches(&self, request: &Request) -> Result<Vec<Match>, MatchingError>;

    /// Find matching requests for an offer
    async fn find_matches_for_offer(&self, offer: &Offer) -> Result<Vec<Match>, MatchingError>;

    /// Score a specific offer-request pair
    fn score_match(&self, offer: &Offer, request: &Request, medication_score: f64) -> MatchScore;

    /// Process pending items from match queue (batch processing)
    async fn process_queue(&self, batch_size: usize) -> Result<usize, MatchingError>;
}

/// Matching service errors
#[derive(Debug, thiserror::Error)]
pub enum MatchingError {
    #[error("Repository error: {0}")]
    Repository(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("No matches found")]
    NoMatches,
}
