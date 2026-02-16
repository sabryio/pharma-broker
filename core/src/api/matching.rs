//! Matching API Handlers
//!
//! Endpoints for manually triggering the matching process.

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::reparse::ItemType;
use super::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct RematchRequest {
    pub item_id: Uuid,
    pub item_type: ItemType,
}

#[derive(Debug, Serialize)]
pub struct RematchResponse {
    pub success: bool,
    pub message: String,
    pub matches_cleared: u64,
    pub items_queued: usize,
}

// ============================================================================
// Handlers
// ============================================================================

/// Manually trigger re-matching for an offer or request
///
/// This endpoint:
/// 1. Verifies the item exists
/// 2. Deletes all pending matches for the item
/// 3. Enqueues relevant items for re-matching
///
/// For offers: Enqueues all active requests (since matching is request-centric)
/// For requests: Enqueues just that request
pub async fn rematch_item<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<RematchRequest>,
) -> Result<Json<RematchResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    tracing::info!(
        item_id = %req.item_id,
        item_type = ?req.item_type,
        "Received rematch request"
    );

    let (matches_cleared, items_queued) = match req.item_type {
        ItemType::Offer => {
            // Verify offer exists and is active
            let offer = state
                .offer_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

            tracing::info!(
                offer_id = %req.item_id,
                medication = %offer.medication,
                status = ?offer.status,
                "Found offer for rematch"
            );

            // Delete pending matches for this offer
            let cleared = state
                .match_repo
                .delete_pending_matches_for_offer(req.item_id)
                .await
                .unwrap_or(0);

            tracing::info!(
                offer_id = %req.item_id,
                matches_cleared = cleared,
                "Cleared pending matches for offer"
            );

            // Trigger re-matching: Since workers are request-centric,
            // we enqueue all active requests that might match this offer.
            // The match processor will find this offer when processing each request.
            let mut queued = 0;
            if let Ok(active_requests) = state.request_repo.get_active(100, 0).await {
                for r in active_requests {
                    if state.match_queue_repo.enqueue(r.id, 0).await.is_ok() {
                        queued += 1;
                    }
                }
            }

            tracing::info!(
                offer_id = %req.item_id,
                requests_queued = queued,
                "Enqueued requests for rematch"
            );

            (cleared, queued)
        }
        ItemType::Request => {
            // Verify request exists and is active
            let request = state
                .request_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

            tracing::info!(
                request_id = %req.item_id,
                medication = %request.medication,
                status = ?request.status,
                "Found request for rematch"
            );

            // Delete pending matches for this request
            let cleared = state
                .match_repo
                .delete_pending_matches_for_request(req.item_id)
                .await
                .unwrap_or(0);

            tracing::info!(
                request_id = %req.item_id,
                matches_cleared = cleared,
                "Cleared pending matches for request"
            );

            // Trigger re-matching for this request
            let queued = if state.match_queue_repo.enqueue(req.item_id, 0).await.is_ok() {
                1
            } else {
                0
            };

            tracing::info!(
                request_id = %req.item_id,
                "Enqueued request for rematch"
            );

            (cleared, queued)
        }
    };

    let message = format!(
        "Rematch triggered: cleared {} pending matches, queued {} items for processing",
        matches_cleared, items_queued
    );

    tracing::info!(
        item_id = %req.item_id,
        item_type = ?req.item_type,
        matches_cleared = matches_cleared,
        items_queued = items_queued,
        "Rematch completed"
    );

    Ok(Json(RematchResponse {
        success: true,
        message,
        matches_cleared,
        items_queued,
    }))
}

// ============================================================================
// Pharmaceutical Validation Configuration API
// ============================================================================

use crate::matching::PharmaceuticalValidationStatsSnapshot;
use crate::matching::PharmaceuticalValidatorConfig;

#[derive(Debug, Serialize)]
pub struct PharmaceuticalConfigResponse {
    pub config: PharmaceuticalValidatorConfig,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePharmaceuticalConfigRequest {
    pub concentration_tolerance_percent: Option<f64>,
    pub concentration_reject_threshold_percent: Option<f64>,
    pub missing_concentration_penalty: Option<f64>,
    pub missing_form_penalty: Option<f64>,
    pub enable_concentration_check: Option<bool>,
    pub enable_form_check: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PharmaceuticalStatsResponse {
    pub stats: PharmaceuticalValidationStatsSnapshot,
}

/// GET /api/matching/pharmaceutical-config
///
/// Get current pharmaceutical validation configuration
pub async fn get_pharmaceutical_config<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<PharmaceuticalConfigResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let matching_engine = state.matching_engine.as_ref().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Matching engine not initialized".to_string(),
    ))?;

    let config = matching_engine.get_pharmaceutical_validator_config();

    tracing::debug!(
        concentration_tolerance = config.concentration_tolerance_percent,
        concentration_reject_threshold = config.concentration_reject_threshold_percent,
        concentration_check_enabled = config.enable_concentration_check,
        form_check_enabled = config.enable_form_check,
        "Retrieved pharmaceutical validation config"
    );

    Ok(Json(PharmaceuticalConfigResponse { config }))
}

/// PUT /api/matching/pharmaceutical-config
///
/// Update pharmaceutical validation configuration
pub async fn update_pharmaceutical_config<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<UpdatePharmaceuticalConfigRequest>,
) -> Result<Json<PharmaceuticalConfigResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let matching_engine = state.matching_engine.as_ref().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Matching engine not initialized".to_string(),
    ))?;

    // Get current config
    let mut config = matching_engine.get_pharmaceutical_validator_config();

    // Apply updates
    if let Some(tolerance) = req.concentration_tolerance_percent {
        if !(0.0..=100.0).contains(&tolerance) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Concentration tolerance must be between 0 and 100".to_string(),
            ));
        }
        config.concentration_tolerance_percent = tolerance;
    }

    if let Some(threshold) = req.concentration_reject_threshold_percent {
        if !(0.0..=100.0).contains(&threshold) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Concentration reject threshold must be between 0 and 100".to_string(),
            ));
        }
        config.concentration_reject_threshold_percent = threshold;
    }

    if let Some(penalty) = req.missing_concentration_penalty {
        if !(0.0..=1.0).contains(&penalty) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Missing concentration penalty must be between 0 and 1".to_string(),
            ));
        }
        config.missing_concentration_penalty = penalty;
    }

    if let Some(penalty) = req.missing_form_penalty {
        if !(0.0..=1.0).contains(&penalty) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Missing form penalty must be between 0 and 1".to_string(),
            ));
        }
        config.missing_form_penalty = penalty;
    }

    if let Some(enabled) = req.enable_concentration_check {
        config.enable_concentration_check = enabled;
    }

    if let Some(enabled) = req.enable_form_check {
        config.enable_form_check = enabled;
    }

    // Validate that reject threshold is greater than tolerance
    if config.concentration_reject_threshold_percent <= config.concentration_tolerance_percent {
        return Err((
            StatusCode::BAD_REQUEST,
            "Concentration reject threshold must be greater than tolerance".to_string(),
        ));
    }

    // Update config
    matching_engine.set_pharmaceutical_validator_config(config.clone());

    tracing::info!(
        concentration_tolerance = config.concentration_tolerance_percent,
        concentration_reject_threshold = config.concentration_reject_threshold_percent,
        concentration_check_enabled = config.enable_concentration_check,
        form_check_enabled = config.enable_form_check,
        "Updated pharmaceutical validation config"
    );

    Ok(Json(PharmaceuticalConfigResponse { config }))
}

/// GET /api/matching/pharmaceutical-stats
///
/// Get pharmaceutical validation statistics
pub async fn get_pharmaceutical_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<PharmaceuticalStatsResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let matching_engine = state.matching_engine.as_ref().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Matching engine not initialized".to_string(),
    ))?;

    let stats = matching_engine.get_pharmaceutical_validator_stats();

    tracing::debug!(
        total_validations = stats.total_validations,
        passed_validations = stats.passed_validations,
        concentration_rejections = stats.concentration_rejections,
        form_rejections = stats.form_rejections,
        rejection_rate = stats.rejection_rate,
        "Retrieved pharmaceutical validation stats"
    );

    Ok(Json(PharmaceuticalStatsResponse { stats }))
}

/// POST /api/matching/pharmaceutical-config/reset
///
/// Reset pharmaceutical validation configuration to defaults
pub async fn reset_pharmaceutical_config<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<PharmaceuticalConfigResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let matching_engine = state.matching_engine.as_ref().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Matching engine not initialized".to_string(),
    ))?;

    let config = PharmaceuticalValidatorConfig::default();
    matching_engine.set_pharmaceutical_validator_config(config.clone());

    tracing::info!("Reset pharmaceutical validation config to defaults");

    Ok(Json(PharmaceuticalConfigResponse { config }))
}
