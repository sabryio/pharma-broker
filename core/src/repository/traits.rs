//! Repository traits
//!
//! Ported from legacy/domain/repository/repository.go

use async_trait::async_trait;
use chrono::Duration;

use crate::Result;
use crate::domain::{ItemStatus, Match, MatchStatus, Offer, RawMessage, Request, Stats};

/// Offer repository trait
/// Ported from Go: OfferReader + OfferWriter (repository.go:13-32)
#[async_trait]
pub trait OfferRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Offer>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<Offer>>;
    async fn save(&self, offer: &Offer) -> Result<()>;
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()>;
}

/// Request repository trait
/// Ported from Go: RequestReader + RequestWriter (repository.go:35-52)
#[async_trait]
pub trait RequestRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Request>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Request>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Request>>;
    async fn count_active(&self) -> Result<i64>;
    async fn save(&self, request: &Request) -> Result<()>;
    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()>;
}

/// Match repository trait
/// Ported from Go: MatchReader + MatchWriter (repository.go:55-80)
#[async_trait]
pub trait MatchRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Match>>;
    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<Match>>;
    async fn count_pending(&self) -> Result<i64>;
    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool>;
    async fn save(&self, match_entity: &Match) -> Result<()>;
    async fn update_status(
        &self,
        id: &str,
        status: MatchStatus,
        matched_by: &str,
        notes: &str,
    ) -> Result<()>;
}

/// Raw message repository trait
/// Ported from Go: RawMessageRepository (repository.go:83-95)
#[async_trait]
pub trait RawMessageRepository: Send + Sync {
    async fn save(&self, message: &RawMessage) -> Result<()>;
    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessage>>;
    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<()>;
}

/// Stats repository trait
#[async_trait]
pub trait StatsRepository: Send + Sync {
    async fn get_stats(&self) -> Result<Stats>;
}
