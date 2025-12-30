//! Convex MatchRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::params::UpdateMatchStatusParams;
use crate::traits::{MatchModel, MatchRepository, MatchStatus};

/// Convex-backed match repository
pub struct ConvexMatchRepo {
    client: Arc<ConvexClient>,
}

impl ConvexMatchRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MatchRepository for ConvexMatchRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<MatchModel>> {
        self.client
            .query("matches:get", convex_args! { "id" => id })
            .await
    }

    async fn get_pending(&self, limit: i64, _offset: i64) -> Result<Vec<MatchModel>> {
        self.client
            .query(
                "matches:list",
                convex_args! {
                    "status" => "PENDING",
                    "limit" => limit
                },
            )
            .await
    }

    async fn count_pending(&self) -> Result<i64> {
        self.client
            .query("matches:countPending", convex_args!())
            .await
    }

    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool> {
        let existing: Option<MatchModel> = self
            .client
            .query(
                "matches:getByOfferAndRequest",
                convex_args! {
                    "offerId" => offer_id,
                    "requestId" => request_id
                },
            )
            .await?;
        Ok(existing.is_some())
    }

    async fn save(&self, m: &MatchModel) -> Result<MatchModel> {
        // MatchModel fields: id, offer_id, request_id, score, reasoning, matched_by, status, created_at, confirmed_at, notes
        let id: String = self
            .client
            .mutation(
                "matches:create",
                convex_args! {
                    "offerId" => &m.offer_id,
                    "requestId" => &m.request_id,
                    "score" => m.score,
                    "reasoning" => m.reasoning.as_ref(),
                    "matchedBy" => m.matched_by.as_ref(),
                    "status" => format!("{:?}", m.status),
                    "notes" => m.notes.as_ref()
                },
            )
            .await?;

        let mut saved = m.clone();
        saved.id = id;
        Ok(saved)
    }

    async fn update_status(&self, params: UpdateMatchStatusParams<'_>) -> Result<MatchModel> {
        // UpdateMatchStatusParams: id, status, matched_by, notes
        let fn_name = match params.status {
            MatchStatus::Confirmed => "matches:confirm",
            MatchStatus::Rejected => "matches:reject",
            _ => "matches:update",
        };

        self.client
            .mutation(
                fn_name,
                convex_args! {
                    "id" => params.id,
                    "matchedBy" => params.matched_by,
                    "notes" => params.notes
                },
            )
            .await
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        self.client
            .mutation(
                "matches:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
