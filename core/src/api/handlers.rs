//! API Handlers
//!
//! Ported from legacy/api/handlers/*.go

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::domain::{ItemStatus, MatchStatus};
use crate::repository::{MatchRepository, OfferRepository, RequestRepository};

/// Pagination query parameters
#[derive(Debug, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
        }
    }

    pub fn with_meta(mut self, total: i64, limit: i64, offset: i64) -> Self {
        self.meta = Some(Meta {
            total,
            limit,
            offset,
        });
        self
    }
}

/// Health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "pharma-core",
        "version": "0.1.0"
    }))
}

/// Get active offers with pagination
/// Ported from: legacy/api/handlers/offer_handler.go:GetOffersGin
pub async fn get_offers<O, R, M>(
    State(state): State<AppState<O, R, M>>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<ApiResponse<Vec<crate::domain::Offer>>>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    let offers = state
        .offer_repo
        .get_active(pagination.limit, pagination.offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state.offer_repo.count_active().await.unwrap_or(0);

    Ok(Json(ApiResponse::success(offers).with_meta(
        total,
        pagination.limit,
        pagination.offset,
    )))
}

/// Get active requests with pagination
/// Ported from: legacy/api/handlers/request_handler.go:GetRequestsGin
pub async fn get_requests<O, R, M>(
    State(state): State<AppState<O, R, M>>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<ApiResponse<Vec<crate::domain::Request>>>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    let requests = state
        .request_repo
        .get_active(pagination.limit, pagination.offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state.request_repo.count_active().await.unwrap_or(0);

    Ok(Json(ApiResponse::success(requests).with_meta(
        total,
        pagination.limit,
        pagination.offset,
    )))
}

/// Get pending matches with pagination
/// Ported from: legacy/api/handlers/match_handler.go:GetMatchesGin
pub async fn get_matches<O, R, M>(
    State(state): State<AppState<O, R, M>>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<ApiResponse<Vec<crate::domain::Match>>>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    let matches = state
        .match_repo
        .get_pending(pagination.limit, pagination.offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state.match_repo.count_pending().await.unwrap_or(0);

    Ok(Json(ApiResponse::success(matches).with_meta(
        total,
        pagination.limit,
        pagination.offset,
    )))
}

/// Confirm match request body
#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub matched_by: String,
    #[serde(default)]
    pub notes: String,
}

/// Confirm a pending match
/// Ported from: legacy/api/handlers/match_handler.go:ConfirmMatchGin
pub async fn confirm_match<O, R, M>(
    State(state): State<AppState<O, R, M>>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    // Get the match first
    let match_entity = state
        .match_repo
        .get_by_id(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Match not found".to_string()))?;

    // Update match status
    state
        .match_repo
        .update_status(&id, MatchStatus::Confirmed, &req.matched_by, &req.notes)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update offer and request status to MATCHED
    let _ = state
        .offer_repo
        .update_status(&match_entity.offer_id, ItemStatus::Matched)
        .await;
    let _ = state
        .request_repo
        .update_status(&match_entity.request_id, ItemStatus::Matched)
        .await;

    Ok(Json(serde_json::json!({
        "status": "confirmed",
        "match_id": id
    })))
}

/// Reject match request body
#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    #[serde(default)]
    pub matched_by: String,
    #[serde(default)]
    pub reason: String,
}

/// Reject a pending match
/// Ported from: legacy/api/handlers/match_handler.go:RejectMatchGin
pub async fn reject_match<O, R, M>(
    State(state): State<AppState<O, R, M>>,
    Path(id): Path<String>,
    Json(req): Json<RejectRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    state
        .match_repo
        .update_status(&id, MatchStatus::Rejected, &req.matched_by, &req.reason)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "rejected",
        "match_id": id
    })))
}

/// Get dashboard stats
/// Ported from: legacy/api/handlers/stats_handler.go:GetStatsGin
pub async fn get_stats<O, R, M>(
    State(state): State<AppState<O, R, M>>,
) -> Result<Json<crate::domain::Stats>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    let active_offers = state.offer_repo.count_active().await.unwrap_or(0);
    let active_requests = state.request_repo.count_active().await.unwrap_or(0);
    let pending_matches = state.match_repo.count_pending().await.unwrap_or(0);

    Ok(Json(crate::domain::Stats {
        active_offers,
        active_requests,
        pending_matches,
        confirmed_today: 0,   // TODO: Implement
        processed_today: 0,   // TODO: Implement
        avg_match_score: 0.0, // TODO: Implement
        monitored_groups: 0,
        connected_clients: 0,
    }))
}
