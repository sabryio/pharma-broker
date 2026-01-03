//! Audit Records API
//!
//! Endpoints for accessing match audit records for debugging and replay.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::matching::{
    AuditRecorderStatsSnapshot, FrontendAuditRecord, MatchAuditRecord, ReplayContext,
};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ListAuditRecordsQuery {
    /// Maximum number of records to return
    pub limit: Option<usize>,
    /// Filter by session ID
    pub session_id: Option<String>,
    /// Filter by minimum score
    pub min_score: Option<f64>,
    /// Filter by AI involvement
    pub ai_involved: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AuditRecordsResponse {
    pub records: Vec<FrontendAuditRecord>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct AuditRecordDetailResponse {
    pub record: MatchAuditRecord,
    pub replay_context: Option<ReplayContext>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditRecorderStatusResponse {
    pub enabled: bool,
    pub buffer_size: usize,
    pub current_buffer_len: usize,
    pub stats: AuditRecorderStatsSnapshot,
    pub config: AuditRecorderConfigResponse,
}

#[derive(Debug, Serialize)]
pub struct AuditRecorderConfigResponse {
    pub buffer_size: usize,
    pub persist_to_db: bool,
    pub min_score_threshold: Option<f64>,
    pub sample_rate: f64,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/audit-records - List recent audit records
pub async fn list_audit_records<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(query): Query<ListAuditRecordsQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();
    let limit = query.limit.unwrap_or(50);

    let records: Vec<FrontendAuditRecord> = if let Some(session_id) = &query.session_id {
        recorder
            .get_by_session(session_id)
            .iter()
            .map(FrontendAuditRecord::from)
            .collect()
    } else {
        recorder
            .get_recent(limit)
            .iter()
            .filter(|r| {
                // Apply filters
                if let Some(min_score) = query.min_score
                    && r.final_score < min_score
                {
                    return false;
                }
                if let Some(ai_involved) = query.ai_involved
                    && r.ai_involved != ai_involved
                {
                    return false;
                }
                true
            })
            .map(FrontendAuditRecord::from)
            .collect()
    };

    let total = records.len();
    Ok(Json(AuditRecordsResponse { records, total }))
}

/// GET /api/audit-records/:match_id - Get audit record by match ID
pub async fn get_audit_record<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();

    let record = recorder.get_by_match_id(match_id).ok_or((
        StatusCode::NOT_FOUND,
        format!("Audit record not found for match {}", match_id),
    ))?;

    // Try to create replay context
    let replay_context = record.to_replay_context().ok();

    Ok(Json(AuditRecordDetailResponse {
        record,
        replay_context,
    }))
}

/// PUT /api/audit-records/:match_id/review - Update review status
pub async fn update_audit_review<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
    Json(req): Json<UpdateReviewRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();

    // TODO: Get actual user ID from auth context
    let reviewer_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    let updated =
        recorder.update_review_status(match_id, &req.status, reviewer_id, req.notes.as_deref());

    if updated {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Review status updated"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("Audit record not found for match {}", match_id),
        ))
    }
}

/// GET /api/audit-records/status - Get audit recorder status
pub async fn get_audit_recorder_status<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();
    let config = recorder.config();

    Ok(Json(AuditRecorderStatusResponse {
        enabled: recorder.is_enabled(),
        buffer_size: config.buffer_size,
        current_buffer_len: recorder.buffer_len(),
        stats: recorder.stats(),
        config: AuditRecorderConfigResponse {
            buffer_size: config.buffer_size,
            persist_to_db: config.persist_to_db,
            min_score_threshold: config.min_score_threshold,
            sample_rate: config.sample_rate,
        },
    }))
}

/// GET /api/audit-records/session/:session_id - Get records by session
pub async fn get_session_records<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();
    let records: Vec<FrontendAuditRecord> = recorder
        .get_by_session(&session_id)
        .iter()
        .map(FrontendAuditRecord::from)
        .collect();

    Ok(Json(AuditRecordsResponse {
        total: records.len(),
        records,
    }))
}
