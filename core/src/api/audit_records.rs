//! Audit Records API
//!
//! Endpoints for accessing match audit records for debugging and replay.
//!
//! Session Synchronization (Requirements 3.1, 3.2, 3.3):
//! - Frontend recordings can be linked to backend audit records via session_id
//! - Session-based queries return all records associated with a session
//! - Client metadata provides additional context for debugging

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
    AuditRecorderStatsSnapshot, ClientMetadata, FrontendAuditRecord, MatchAuditRecord,
    ReplayContext,
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

/// Request to create an audit record with session context
/// Requirements: 3.1, 3.2
#[derive(Debug, Deserialize)]
pub struct CreateAuditRecordRequest {
    /// Session ID for frontend correlation
    pub session_id: Option<String>,
    /// Client metadata for debugging context
    pub client_metadata: Option<ClientMetadataRequest>,
}

/// Client metadata from frontend
/// Requirements: 3.2
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientMetadataRequest {
    /// User agent string
    pub user_agent: Option<String>,
    /// Client application version
    pub client_version: Option<String>,
    /// Frontend recording ID
    pub recording_id: Option<String>,
}

impl From<ClientMetadataRequest> for ClientMetadata {
    fn from(req: ClientMetadataRequest) -> Self {
        ClientMetadata {
            user_agent: req.user_agent,
            client_version: req.client_version,
            recording_id: req.recording_id,
        }
    }
}

/// Response for session-based audit record queries
/// Requirements: 3.3
#[derive(Debug, Serialize)]
pub struct SessionAuditRecordsResponse {
    /// Session ID that was queried
    pub session_id: String,
    /// All audit records associated with this session
    pub records: Vec<FrontendAuditRecord>,
    /// Total count of records found
    pub total: usize,
    /// Whether records were found in buffer, database, or both
    pub source: RecordSource,
}

/// Indicates where records were retrieved from
#[derive(Debug, Serialize)]
pub struct RecordSource {
    /// Number of records from in-memory buffer
    pub from_buffer: usize,
    /// Number of records from database
    pub from_database: usize,
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
    /// Session ID if present
    pub session_id: Option<String>,
    /// Client metadata if present
    pub client_metadata: Option<ClientMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    pub status: String,
    pub notes: Option<String>,
    /// Optional session ID to associate with the review
    pub session_id: Option<String>,
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

    // Extract session info for response
    let session_id = record.session_id.clone();
    let client_metadata = record.client_metadata.clone();

    Ok(Json(AuditRecordDetailResponse {
        record,
        replay_context,
        session_id,
        client_metadata,
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
/// Requirements: 3.3
///
/// Queries both in-memory buffer and database for records associated with
/// the given session_id, merging and deduplicating results.
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

    // Get records from in-memory buffer
    let buffer_records = recorder.get_by_session(&session_id);
    let from_buffer = buffer_records.len();

    // Convert to frontend format
    let records: Vec<FrontendAuditRecord> = buffer_records
        .iter()
        .map(FrontendAuditRecord::from)
        .collect();

    // Note: For full database integration, the PersistentAuditRecorder
    // should be used instead. This endpoint currently only queries the
    // in-memory buffer. Database queries would be added when the
    // PersistentAuditRecorder is integrated into the AppState.
    let from_database = 0;

    Ok(Json(SessionAuditRecordsResponse {
        session_id,
        total: records.len(),
        records,
        source: RecordSource {
            from_buffer,
            from_database,
        },
    }))
}
