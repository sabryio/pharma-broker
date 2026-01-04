//! Calibration Management API Handlers
//!
//! REST endpoints for managing confidence calibration:
//! - GET  /api/calibration          - Get calibration report
//! - PUT  /api/calibration          - Update calibration configuration
//! - POST /api/calibration/reset    - Reset calibration data
//! - POST /api/calibration/enable   - Enable/disable calibration

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::matching::{CalibrationConfig, CalibrationReport};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

use super::routes::AppState;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Response for calibration status
#[derive(Debug, Serialize)]
pub struct CalibrationResponse {
    pub report: CalibrationReport,
    pub config: CalibrationConfigDto,
}

/// DTO for calibration configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct CalibrationConfigDto {
    pub enabled: bool,
    pub num_bins: usize,
    pub min_samples_per_bin: usize,
    pub smoothing_factor: f64,
    pub window_size: usize,
}

impl From<CalibrationConfig> for CalibrationConfigDto {
    fn from(c: CalibrationConfig) -> Self {
        Self {
            enabled: c.enabled,
            num_bins: c.num_bins,
            min_samples_per_bin: c.min_samples_per_bin,
            smoothing_factor: c.smoothing_factor,
            window_size: c.window_size,
        }
    }
}

impl From<CalibrationConfigDto> for CalibrationConfig {
    fn from(dto: CalibrationConfigDto) -> Self {
        Self {
            enabled: dto.enabled,
            num_bins: dto.num_bins,
            min_samples_per_bin: dto.min_samples_per_bin,
            smoothing_factor: dto.smoothing_factor,
            window_size: dto.window_size,
        }
    }
}

/// Request to toggle calibration
#[derive(Debug, Deserialize)]
pub struct EnableCalibrationRequest {
    pub enabled: bool,
}

/// Request to update smoothing factor
#[derive(Debug, Deserialize)]
pub struct SmoothingRequest {
    pub smoothing_factor: f64,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/calibration - Get calibration report and configuration
pub async fn get_calibration<RQ, A, MM>(
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

    let report = engine.get_calibration_report();
    let config = engine.get_calibration_config();

    Ok(Json(CalibrationResponse {
        report,
        config: config.into(),
    }))
}

/// PUT /api/calibration - Update calibration configuration
pub async fn update_calibration<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<CalibrationConfigDto>,
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

    // Validate
    if req.num_bins == 0 || req.num_bins > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            "num_bins must be between 1 and 100".to_string(),
        ));
    }

    if req.smoothing_factor < 0.0 || req.smoothing_factor > 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "smoothing_factor must be between 0.0 and 1.0".to_string(),
        ));
    }

    let config: CalibrationConfig = req.into();
    engine.set_calibration_config(config.clone());

    tracing::info!(
        enabled = config.enabled,
        num_bins = config.num_bins,
        smoothing = config.smoothing_factor,
        "Calibration configuration updated via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Calibration configuration updated"
    })))
}

/// POST /api/calibration/reset - Reset all calibration data
pub async fn reset_calibration<RQ, A, MM>(
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

    engine.reset_calibration();

    tracing::info!("Calibration data reset via API");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Calibration data reset"
    })))
}

/// POST /api/calibration/enable - Enable or disable calibration
pub async fn toggle_calibration<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<EnableCalibrationRequest>,
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

    engine.enable_calibration(req.enabled);

    tracing::info!(enabled = req.enabled, "Calibration toggled via API");

    Ok(Json(serde_json::json!({
        "success": true,
        "enabled": req.enabled,
        "message": if req.enabled {
            "Calibration enabled"
        } else {
            "Calibration disabled"
        }
    })))
}

/// PUT /api/calibration/smoothing - Update smoothing factor
pub async fn update_smoothing<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<SmoothingRequest>,
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

    if req.smoothing_factor < 0.0 || req.smoothing_factor > 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "smoothing_factor must be between 0.0 and 1.0".to_string(),
        ));
    }

    engine.set_calibration_smoothing(req.smoothing_factor);

    tracing::info!(
        smoothing_factor = req.smoothing_factor,
        "Calibration smoothing factor updated via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "smoothing_factor": req.smoothing_factor
    })))
}

/// POST /api/calibration/calibrate - Calibrate a raw score (for testing)
pub async fn calibrate_score<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<serde_json::Value>,
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

    let raw_score = req
        .get("score")
        .and_then(|v| v.as_f64())
        .ok_or((StatusCode::BAD_REQUEST, "Missing 'score' field".to_string()))?;

    if !(0.0..=1.0).contains(&raw_score) {
        return Err((
            StatusCode::BAD_REQUEST,
            "score must be between 0.0 and 1.0".to_string(),
        ));
    }

    let calibrated = engine.calibrate_score(raw_score);

    Ok(Json(serde_json::json!({
        "raw_score": raw_score,
        "calibrated_score": calibrated,
        "adjustment": calibrated - raw_score
    })))
}
