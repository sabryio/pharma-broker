//! Match Filter API Endpoints
//!
//! REST API for managing match filtering (stale offers, same-sender exclusion)

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::matching::{MatchFilterConfig, MatchFilterStatsSnapshot};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// =============================================================================
// Response Types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct MatchFilterResponse {
    pub config: MatchFilterConfig,
    pub stats: MatchFilterStatsSnapshot,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMatchFilterRequest {
    pub enable_stale_filter: Option<bool>,
    pub max_offer_age_days: Option<i64>,
    pub enable_same_sender_exclusion: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    pub enabled: bool,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/match-filter - Get match filter configuration and stats
pub async fn get_match_filter<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let response = MatchFilterResponse {
        config: engine.get_match_filter_config(),
        stats: engine.get_match_filter_stats(),
    };

    Json(response).into_response()
}

/// PUT /api/match-filter - Update match filter configuration
pub async fn update_match_filter<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<UpdateMatchFilterRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let mut config = engine.get_match_filter_config();

    if let Some(enable_stale) = req.enable_stale_filter {
        config.enable_stale_filter = enable_stale;
    }
    if let Some(max_age) = req.max_offer_age_days {
        config.max_offer_age_days = max_age;
    }
    if let Some(enable_same_sender) = req.enable_same_sender_exclusion {
        config.enable_same_sender_exclusion = enable_same_sender;
    }

    engine.set_match_filter_config(config.clone());

    let response = MatchFilterResponse {
        config,
        stats: engine.get_match_filter_stats(),
    };

    Json(response).into_response()
}

/// POST /api/match-filter/stale - Toggle stale filter
pub async fn toggle_stale_filter<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ToggleRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    engine.enable_stale_filter(req.enabled);

    Json(serde_json::json!({
        "stale_filter_enabled": req.enabled
    }))
    .into_response()
}

/// POST /api/match-filter/same-sender - Toggle same-sender exclusion
pub async fn toggle_same_sender<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ToggleRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    engine.enable_same_sender_exclusion(req.enabled);

    Json(serde_json::json!({
        "same_sender_exclusion_enabled": req.enabled
    }))
    .into_response()
}

/// POST /api/match-filter/stats/reset - Reset match filter statistics
pub async fn reset_stats<RQ, A, MM>(State(state): State<AppState<RQ, A, MM>>) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    engine.reset_match_filter_stats();

    Json(serde_json::json!({
        "message": "Match filter statistics reset"
    }))
    .into_response()
}
