//! Confidence Management API Handlers
//!
//! REST endpoints for managing dynamic confidence thresholds:
//! - GET  /api/confidence          - Get confidence stats and thresholds
//! - PUT  /api/confidence          - Update confidence configuration
//! - POST /api/confidence/reset    - Reset thresholds to base values
//! - POST /api/confidence/adaptive - Enable/disable adaptive mode

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::matching::{ConfidenceConfig, ConfidenceManagerStats};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

use super::routes::AppState;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Response for confidence stats and configuration
#[derive(Debug, Serialize)]
pub struct ConfidenceResponse {
    pub stats: ConfidenceManagerStats,
    pub config: ConfidenceConfigDto,
}

/// DTO for confidence configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfidenceConfigDto {
    pub base_strict: f64,
    pub base_relaxed: f64,
    pub enable_adaptive: bool,
    pub adjustment_step: f64,
    pub min_threshold: f64,
    pub max_threshold: f64,
    pub evaluation_window: usize,
    pub target_accept_rate: f64,
    pub accept_rate_tolerance: f64,
}

impl From<ConfidenceConfig> for ConfidenceConfigDto {
    fn from(c: ConfidenceConfig) -> Self {
        Self {
            base_strict: c.base_strict,
            base_relaxed: c.base_relaxed,
            enable_adaptive: c.enable_adaptive,
            adjustment_step: c.adjustment_step,
            min_threshold: c.min_threshold,
            max_threshold: c.max_threshold,
            evaluation_window: c.evaluation_window,
            target_accept_rate: c.target_accept_rate,
            accept_rate_tolerance: c.accept_rate_tolerance,
        }
    }
}

impl From<ConfidenceConfigDto> for ConfidenceConfig {
    fn from(dto: ConfidenceConfigDto) -> Self {
        Self {
            base_strict: dto.base_strict,
            base_relaxed: dto.base_relaxed,
            enable_adaptive: dto.enable_adaptive,
            adjustment_step: dto.adjustment_step,
            min_threshold: dto.min_threshold,
            max_threshold: dto.max_threshold,
            evaluation_window: dto.evaluation_window,
            target_accept_rate: dto.target_accept_rate,
            accept_rate_tolerance: dto.accept_rate_tolerance,
        }
    }
}

/// Request to update confidence thresholds
#[derive(Debug, Deserialize)]
pub struct UpdateThresholdsRequest {
    pub strict: Option<f64>,
    pub relaxed: Option<f64>,
}

/// Request to toggle adaptive mode
#[derive(Debug, Deserialize)]
pub struct AdaptiveModeRequest {
    pub enabled: bool,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/confidence - Get confidence stats and configuration
pub async fn get_confidence<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let stats = engine.get_confidence_stats();
    let config = engine.get_confidence_config();

    Ok(Json(ConfidenceResponse {
        stats,
        config: config.into(),
    }))
}

/// PUT /api/confidence - Update confidence configuration
pub async fn update_confidence<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ConfidenceConfigDto>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    // Validate thresholds
    if req.base_strict < req.min_threshold || req.base_strict > req.max_threshold {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "base_strict must be between {} and {}",
                req.min_threshold, req.max_threshold
            ),
        ));
    }

    if req.base_relaxed < req.min_threshold || req.base_relaxed > req.max_threshold {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "base_relaxed must be between {} and {}",
                req.min_threshold, req.max_threshold
            ),
        ));
    }

    if req.target_accept_rate <= 0.0 || req.target_accept_rate >= 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "target_accept_rate must be between 0 and 1".to_string(),
        ));
    }

    let config: ConfidenceConfig = req.into();
    engine.set_confidence_config(config.clone());

    tracing::info!(
        base_strict = config.base_strict,
        base_relaxed = config.base_relaxed,
        adaptive = config.enable_adaptive,
        "Confidence configuration updated via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Confidence configuration updated"
    })))
}

/// PUT /api/confidence/thresholds - Update thresholds manually
pub async fn update_thresholds<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<UpdateThresholdsRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    if let Some(strict) = req.strict {
        engine.set_strict_threshold(strict);
    }

    if let Some(relaxed) = req.relaxed {
        engine.set_relaxed_threshold(relaxed);
    }

    let new_strict = engine.get_strict_threshold();
    let new_relaxed = engine.get_relaxed_threshold();

    tracing::info!(
        strict = new_strict,
        relaxed = new_relaxed,
        "Confidence thresholds updated via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "strict_threshold": new_strict,
        "relaxed_threshold": new_relaxed
    })))
}

/// POST /api/confidence/reset - Reset thresholds to base values
pub async fn reset_confidence<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    engine.reset_confidence_thresholds();

    let new_strict = engine.get_strict_threshold();
    let new_relaxed = engine.get_relaxed_threshold();

    tracing::info!(
        strict = new_strict,
        relaxed = new_relaxed,
        "Confidence thresholds reset to base values via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Thresholds reset to base values",
        "strict_threshold": new_strict,
        "relaxed_threshold": new_relaxed
    })))
}

/// POST /api/confidence/adaptive - Enable/disable adaptive mode
pub async fn toggle_adaptive<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<AdaptiveModeRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    engine.enable_adaptive_confidence(req.enabled);

    tracing::info!(
        enabled = req.enabled,
        "Adaptive confidence mode toggled via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "adaptive_enabled": req.enabled,
        "message": if req.enabled {
            "Adaptive confidence adjustment enabled"
        } else {
            "Adaptive confidence adjustment disabled"
        }
    })))
}

/// POST /api/confidence/stats/reset - Reset confidence statistics
pub async fn reset_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    engine.reset_confidence_stats();

    tracing::info!("Confidence statistics reset via API");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Confidence statistics reset"
    })))
}
