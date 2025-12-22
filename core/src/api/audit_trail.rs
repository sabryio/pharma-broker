//! Audit Trail API Endpoints
//!
//! REST API for querying match action audit logs

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::matching::{AuditEntry, AuditTrailConfig};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct AuditQueryParams {
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct AuditTrailResponse {
    pub config: AuditTrailConfig,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AuditHistoryResponse {
    pub match_id: String,
    pub entries: Vec<AuditEntry>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RecentActionsResponse {
    pub entries: Vec<AuditEntry>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub enabled: Option<bool>,
    pub log_to_tracing: Option<bool>,
    pub retention_days: Option<u32>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/audit-trail - Get audit trail configuration
pub async fn get_audit_trail<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let config = engine.get_audit_trail_config();
    let response = AuditTrailResponse {
        enabled: config.enabled,
        config,
    };

    Json(response).into_response()
}

/// PUT /api/audit-trail - Update audit trail configuration
pub async fn update_audit_trail<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<UpdateConfigRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let mut config = engine.get_audit_trail_config();

    if let Some(enabled) = req.enabled {
        config.enabled = enabled;
    }
    if let Some(log_to_tracing) = req.log_to_tracing {
        config.log_to_tracing = log_to_tracing;
    }
    if let Some(retention_days) = req.retention_days {
        config.retention_days = retention_days;
    }

    engine.set_audit_trail_config(config.clone());

    let response = AuditTrailResponse {
        enabled: config.enabled,
        config,
    };

    Json(response).into_response()
}

/// POST /api/audit-trail/enable - Toggle audit trail
pub async fn toggle_audit_trail<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ToggleRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    engine.enable_audit_trail(req.enabled);

    Json(serde_json::json!({
        "audit_trail_enabled": req.enabled
    }))
    .into_response()
}

/// GET /api/audit-trail/match/{match_id} - Get audit history for a match
pub async fn get_match_history<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<String>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    match engine.get_match_audit_history(&match_id).await {
        Ok(entries) => {
            let response = AuditHistoryResponse {
                match_id,
                count: entries.len(),
                entries,
            };
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to get audit history: {}", e)})),
        )
            .into_response(),
    }
}

/// GET /api/audit-trail/recent - Get recent audit actions
pub async fn get_recent_actions<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(params): Query<AuditQueryParams>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let limit = params.limit.unwrap_or(50);

    match engine.get_recent_audit_actions(limit).await {
        Ok(entries) => {
            let response = RecentActionsResponse {
                count: entries.len(),
                entries,
            };
            Json(response).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to get recent actions: {}", e)})),
        )
            .into_response(),
    }
}
