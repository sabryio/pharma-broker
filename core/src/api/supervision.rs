//! AI Supervision API Handlers
//!
//! REST endpoints for managing AI-supervised auto-approval:
//! - GET  /api/supervision/stats       - Get auto-approve statistics
//! - GET  /api/supervision/audit       - Get supervision audit log
//! - GET  /api/supervision/config      - Get auto-approve configuration
//! - PUT  /api/supervision/config      - Update auto-approve configuration
//! - POST /api/supervision/override/:id - Override an AI decision
//! - POST /api/supervision/undo/:id    - Undo an auto-approval
//!
//! Requirements: 3.2, 4.1, 4.2, 5.1

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::matching::{
    AutoApproveConfig, AutoApproveStats, SupervisionAuditEntry, SupervisionAuditFilter,
    SupervisionEventType, SystemStatus,
};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

use super::routes::AppState;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Response for supervision statistics
/// Requirements: 3.2
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionStatsResponse {
    pub total_approved_today: u64,
    pub total_queued_today: u64,
    pub total_blocked_today: u64,
    pub override_rate: f64,
    pub average_confidence: f64,
    pub pending_review_count: u64,
    pub system_status: String,
    pub pause_reason: Option<String>,
}

impl From<AutoApproveStats> for SupervisionStatsResponse {
    fn from(stats: AutoApproveStats) -> Self {
        Self {
            total_approved_today: stats.total_approved_today,
            total_queued_today: stats.total_queued_today,
            total_blocked_today: stats.total_blocked_today,
            override_rate: stats.override_rate,
            average_confidence: stats.average_confidence,
            pending_review_count: stats.pending_review_count,
            system_status: match stats.system_status {
                SystemStatus::Active => "active".to_string(),
                SystemStatus::Paused => "paused".to_string(),
                SystemStatus::Disabled => "disabled".to_string(),
            },
            pause_reason: stats.pause_reason,
        }
    }
}

/// DTO for auto-approve configuration
/// Requirements: 5.1
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApproveConfigDto {
    pub enabled: bool,
    pub confidence_threshold: f64,
    pub batch_size: usize,
    pub processing_interval_secs: u64,
    pub undo_window_mins: u64,
    pub override_rate_pause_threshold: f64,
    pub consecutive_override_limit: u32,
    pub override_cooldown_mins: u64,
    pub category_thresholds: HashMap<String, f64>,
    pub schedule: Option<String>,
}

impl From<AutoApproveConfig> for AutoApproveConfigDto {
    fn from(config: AutoApproveConfig) -> Self {
        Self {
            enabled: config.enabled,
            confidence_threshold: config.confidence_threshold,
            batch_size: config.batch_size,
            processing_interval_secs: config.processing_interval_secs,
            undo_window_mins: config.undo_window_mins,
            override_rate_pause_threshold: config.override_rate_pause_threshold,
            consecutive_override_limit: config.consecutive_override_limit,
            override_cooldown_mins: config.override_cooldown_mins,
            category_thresholds: config.category_thresholds,
            schedule: config.schedule,
        }
    }
}

impl From<AutoApproveConfigDto> for AutoApproveConfig {
    fn from(dto: AutoApproveConfigDto) -> Self {
        Self {
            enabled: dto.enabled,
            confidence_threshold: dto.confidence_threshold,
            batch_size: dto.batch_size,
            processing_interval_secs: dto.processing_interval_secs,
            undo_window_mins: dto.undo_window_mins,
            override_rate_pause_threshold: dto.override_rate_pause_threshold,
            consecutive_override_limit: dto.consecutive_override_limit,
            override_cooldown_mins: dto.override_cooldown_mins,
            category_thresholds: dto.category_thresholds,
            schedule: dto.schedule,
        }
    }
}

/// Response for configuration endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub config: AutoApproveConfigDto,
    pub stats: SupervisionStatsResponse,
}

/// Query parameters for audit log
/// Requirements: 2.3
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQueryParams {
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by match ID
    pub match_id: Option<Uuid>,
    /// Filter by minimum confidence
    pub min_confidence: Option<f64>,
    /// Filter by maximum confidence
    pub max_confidence: Option<f64>,
    /// Filter by override status
    pub overridden: Option<bool>,
    /// Start date for date range filter
    pub start_date: Option<DateTime<Utc>>,
    /// End date for date range filter
    pub end_date: Option<DateTime<Utc>>,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

impl From<AuditQueryParams> for SupervisionAuditFilter {
    fn from(params: AuditQueryParams) -> Self {
        let mut filter = SupervisionAuditFilter::new();

        if let Some(et) = params.event_type.and_then(|e| parse_event_type(&e)) {
            filter = filter.of_type(et);
        }

        if let Some(match_id) = params.match_id {
            filter = filter.for_match(match_id);
        }

        if let (Some(min), Some(max)) = (params.min_confidence, params.max_confidence) {
            filter = filter.in_confidence_range(min, max);
        }

        if let Some(overridden) = params.overridden {
            filter = filter.with_override_status(overridden);
        }

        if let (Some(start), Some(end)) = (params.start_date, params.end_date) {
            filter = filter.in_date_range(start, end);
        }

        if let Some(limit) = params.limit {
            filter = filter.with_limit(limit);
        }

        filter
    }
}

/// Parse event type string to enum
fn parse_event_type(s: &str) -> Option<SupervisionEventType> {
    match s.to_lowercase().as_str() {
        "auto_approved" | "autoapproved" => Some(SupervisionEventType::AutoApproved),
        "queued_for_review" | "queuedforreview" => Some(SupervisionEventType::QueuedForReview),
        "blocked" => Some(SupervisionEventType::Blocked),
        "overridden" => Some(SupervisionEventType::Overridden),
        "undo_approval" | "undoapproval" => Some(SupervisionEventType::UndoApproval),
        "config_changed" | "configchanged" => Some(SupervisionEventType::ConfigChanged),
        "system_paused" | "systempaused" => Some(SupervisionEventType::SystemPaused),
        "system_resumed" | "systemresumed" => Some(SupervisionEventType::SystemResumed),
        _ => None,
    }
}

/// DTO for audit entry response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntryDto {
    pub id: Uuid,
    pub match_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub ai_confidence: Option<f64>,
    pub ai_explanation: Option<String>,
    pub decision: Option<String>,
    pub overridden: bool,
    pub override_by: Option<Uuid>,
    pub override_reason: Option<String>,
    pub override_at: Option<DateTime<Utc>>,
}

impl From<SupervisionAuditEntry> for AuditEntryDto {
    fn from(entry: SupervisionAuditEntry) -> Self {
        Self {
            id: entry.id,
            match_id: entry.match_id,
            timestamp: entry.timestamp,
            event_type: format!("{:?}", entry.event_type),
            ai_confidence: entry.ai_confidence,
            ai_explanation: entry.ai_explanation,
            decision: entry.decision.map(|d| format!("{:?}", d)),
            overridden: entry.overridden,
            override_by: entry.override_by,
            override_reason: entry.override_reason,
            override_at: entry.override_at,
        }
    }
}

/// Response for audit log endpoint
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogResponse {
    pub entries: Vec<AuditEntryDto>,
    pub total: usize,
}

/// Request to override an AI decision
/// Requirements: 4.1
/// Feature: ai-supervision-persistence (Requirements 1.5, 3.5)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideRequest {
    pub user_id: Uuid,
    pub reason: String,
    /// Original AI confidence score for audit trail
    #[serde(default)]
    pub original_confidence: Option<f64>,
    /// Original AI explanation for audit trail
    #[serde(default)]
    pub original_explanation: Option<String>,
}

/// Request to undo an auto-approval
/// Requirements: 4.2
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoRequest {
    pub user_id: Uuid,
}

/// Generic success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/supervision/stats - Get auto-approve statistics
/// Requirements: 3.2
pub async fn get_stats<RQ, A, MM>(
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

    // Get stats from the auto-approve processor
    let stats = engine.get_auto_approve_stats().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get stats: {}", e),
        )
    })?;

    Ok(Json(SupervisionStatsResponse::from(stats)))
}

/// GET /api/supervision/audit - Get supervision audit log
/// Requirements: 2.3
pub async fn get_audit<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(params): Query<AuditQueryParams>,
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

    let filter: SupervisionAuditFilter = params.into();

    let entries = engine
        .get_supervision_audit_log(&filter)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get audit log: {}", e),
            )
        })?;

    let total = entries.len();
    let dto_entries: Vec<AuditEntryDto> = entries.into_iter().map(|e| e.into()).collect();

    Ok(Json(AuditLogResponse {
        entries: dto_entries,
        total,
    }))
}

/// GET /api/supervision/config - Get auto-approve configuration
/// Requirements: 5.1
pub async fn get_config<RQ, A, MM>(
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

    let config = engine.get_auto_approve_config().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get config: {}", e),
        )
    })?;

    let stats = engine.get_auto_approve_stats().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get stats: {}", e),
        )
    })?;

    Ok(Json(ConfigResponse {
        config: config.into(),
        stats: stats.into(),
    }))
}

/// PUT /api/supervision/config - Update auto-approve configuration
/// Requirements: 5.1
pub async fn update_config<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<AutoApproveConfigDto>,
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

    // Validate the configuration
    let config: AutoApproveConfig = req.into();
    if let Err(e) = config.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Invalid configuration: {}", e),
        ));
    }

    // Update the configuration
    engine
        .update_auto_approve_config(config.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update config: {}", e),
            )
        })?;

    tracing::info!(
        enabled = config.enabled,
        threshold = config.confidence_threshold,
        "Auto-approve configuration updated via API"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: "Configuration updated successfully".to_string(),
    }))
}

/// POST /api/supervision/override/:id - Override an AI decision
/// Requirements: 4.1
pub async fn override_decision<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
    Json(req): Json<OverrideRequest>,
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

    // Validate reason is not empty
    if req.reason.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Override reason cannot be empty".to_string(),
        ));
    }

    // Perform the override
    engine
        .override_auto_approve_decision(
            match_id,
            req.user_id,
            &req.reason,
            req.original_confidence.unwrap_or(0.0),
            req.original_explanation.as_deref().unwrap_or("Unknown"),
        )
        .await
        .map_err(|e| {
            let status = match e.to_string().as_str() {
                s if s.contains("not found") => StatusCode::NOT_FOUND,
                s if s.contains("not auto-approved") => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, format!("Failed to override decision: {}", e))
        })?;

    tracing::info!(
        match_id = %match_id,
        user_id = %req.user_id,
        reason = %req.reason,
        "AI decision overridden via API"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Decision for match {} overridden successfully", match_id),
    }))
}

/// POST /api/supervision/undo/:id - Undo an auto-approval
/// Requirements: 4.2
pub async fn undo_approval<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
    Json(req): Json<UndoRequest>,
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

    // Perform the undo
    engine
        .undo_auto_approval(match_id, req.user_id)
        .await
        .map_err(|e| {
            let status = match e.to_string().as_str() {
                s if s.contains("not found") => StatusCode::NOT_FOUND,
                s if s.contains("window expired") => StatusCode::BAD_REQUEST,
                s if s.contains("not auto-approved") => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, format!("Failed to undo approval: {}", e))
        })?;

    tracing::info!(
        match_id = %match_id,
        user_id = %req.user_id,
        "Auto-approval undone via API"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Approval for match {} undone successfully", match_id),
    }))
}

/// POST /api/supervision/pause - Pause auto-approval system
pub async fn pause_system<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<PauseRequest>,
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

    engine
        .pause_auto_approve(req.user_id, &req.reason)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to pause system: {}", e),
            )
        })?;

    tracing::info!(
        user_id = %req.user_id,
        reason = %req.reason,
        "Auto-approve system paused via API"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: "Auto-approve system paused".to_string(),
    }))
}

/// POST /api/supervision/resume - Resume auto-approval system
pub async fn resume_system<RQ, A, MM>(
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

    engine.resume_auto_approve().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to resume system: {}", e),
        )
    })?;

    tracing::info!("Auto-approve system resumed via API");

    Ok(Json(SuccessResponse {
        success: true,
        message: "Auto-approve system resumed".to_string(),
    }))
}

/// Request to pause the system
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseRequest {
    pub user_id: Uuid,
    pub reason: String,
}
