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
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

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
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let limit = query.limit.unwrap_or(50);

    // Try to use PersistentAuditRecorder first (queries both buffer and database)
    // Feature: debug-recordings-persistence (Requirements 3.1)
    if let Some(persistent_recorder) = engine.get_persistent_audit_recorder() {
        let records: Vec<FrontendAuditRecord> = if let Some(session_id) = &query.session_id {
            // Query by session from both buffer and database
            match persistent_recorder.get_by_session(session_id).await {
                Ok(session_records) => session_records
                    .iter()
                    .map(FrontendAuditRecord::from)
                    .collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to query persistent recorder, falling back to in-memory");
                    engine
                        .get_audit_recorder()
                        .get_by_session(session_id)
                        .iter()
                        .map(FrontendAuditRecord::from)
                        .collect()
                }
            }
        } else {
            // List recent from both buffer and database
            match persistent_recorder.list_audit_records(limit, 0).await {
                Ok(all_records) => all_records
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
                    .collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to query persistent recorder, falling back to in-memory");
                    engine
                        .get_audit_recorder()
                        .get_recent(limit)
                        .iter()
                        .filter(|r| {
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
                }
            }
        };

        let total = records.len();
        return Ok(Json(AuditRecordsResponse { records, total }));
    }

    // Fallback to in-memory AuditRecorder if PersistentAuditRecorder is not configured
    let recorder = engine.get_audit_recorder();

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
    MM: MedicationMasterRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    // Try to use PersistentAuditRecorder first (queries both buffer and database)
    // Feature: debug-recordings-persistence (Requirements 3.1)
    if let Some(persistent_recorder) = engine.get_persistent_audit_recorder() {
        match persistent_recorder.get_by_match_id(match_id).await {
            Ok(Some(record)) => {
                // Try to create replay context
                let replay_context = record.to_replay_context().ok();

                // Extract session info for response
                let session_id = record.session_id.clone();
                let client_metadata = record.client_metadata.clone();

                return Ok(Json(AuditRecordDetailResponse {
                    record,
                    replay_context,
                    session_id,
                    client_metadata,
                }));
            }
            Ok(None) => {
                return Err((
                    StatusCode::NOT_FOUND,
                    format!("Audit record not found for match {}", match_id),
                ));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to query persistent recorder, falling back to in-memory");
                // Fall through to in-memory recorder
            }
        }
    }

    // Fallback to in-memory AuditRecorder
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
    MM: MedicationMasterRepository + 'static,
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
    MM: MedicationMasterRepository + 'static,
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
/// Requirements: 3.1, 3.2, 3.3
///
/// Queries both in-memory buffer and database for records associated with
/// the given session_id, merging and deduplicating results.
///
/// Feature: debug-recordings-persistence
/// - Uses PersistentAuditRecorder to query both buffer and database (Req 3.1)
/// - Deduplicates records by id field (Req 3.2)
/// - Orders results by created_at timestamp descending (Req 3.3)
pub async fn get_session_records<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(session_id): Path<String>,
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

    // Try to use PersistentAuditRecorder first (queries both buffer and database)
    // Feature: debug-recordings-persistence (Requirements 3.1)
    if let Some(persistent_recorder) = engine.get_persistent_audit_recorder() {
        // Get records from both in-memory buffer and database
        let buffer_records = persistent_recorder
            .get_by_session_from_buffer(&session_id)
            .await;
        let from_buffer = buffer_records.len();

        // Query database - handle errors gracefully with partial results (Req 3.1)
        let (db_records, from_database) = match persistent_recorder
            .get_by_session(&session_id)
            .await
        {
            Ok(all_records) => {
                // get_by_session returns merged results, calculate db-only count
                let db_count = all_records.len().saturating_sub(from_buffer);
                (all_records, db_count)
            }
            Err(e) => {
                // Log error but continue with buffer-only results
                tracing::warn!(
                    error = %e,
                    session_id = %session_id,
                    "Failed to query database for session records, returning buffer-only results"
                );
                (buffer_records.clone(), 0)
            }
        };

        // Deduplicate records by id field (Req 3.2)
        let deduplicated = deduplicate_records(db_records);

        // Sort by created_at timestamp descending (Req 3.3)
        let sorted = sort_records_by_timestamp(deduplicated);

        // Convert to frontend format
        let records: Vec<FrontendAuditRecord> =
            sorted.iter().map(FrontendAuditRecord::from).collect();

        return Ok(Json(SessionAuditRecordsResponse {
            session_id,
            total: records.len(),
            records,
            source: RecordSource {
                from_buffer,
                from_database,
            },
        }));
    }

    // Fallback to in-memory AuditRecorder if PersistentAuditRecorder is not configured
    let recorder = engine.get_audit_recorder();
    let buffer_records = recorder.get_by_session(&session_id);
    let from_buffer = buffer_records.len();

    // Sort by created_at timestamp descending (Req 3.3)
    let sorted = sort_records_by_timestamp(buffer_records);

    // Convert to frontend format
    let records: Vec<FrontendAuditRecord> = sorted.iter().map(FrontendAuditRecord::from).collect();

    Ok(Json(SessionAuditRecordsResponse {
        session_id,
        total: records.len(),
        records,
        source: RecordSource {
            from_buffer,
            from_database: 0,
        },
    }))
}

/// Deduplicate records by id field
/// Feature: debug-recordings-persistence (Requirements 3.2)
///
/// Uses the `id` field to identify duplicates and keeps a single instance
/// of each unique record.
fn deduplicate_records(records: Vec<MatchAuditRecord>) -> Vec<MatchAuditRecord> {
    use std::collections::HashSet;

    let mut seen_ids: HashSet<uuid::Uuid> = HashSet::new();
    let mut deduplicated = Vec::with_capacity(records.len());

    for record in records {
        if seen_ids.insert(record.id) {
            deduplicated.push(record);
        }
    }

    deduplicated
}

/// Sort records by created_at timestamp in descending order (most recent first)
/// Feature: debug-recordings-persistence (Requirements 3.3)
fn sort_records_by_timestamp(mut records: Vec<MatchAuditRecord>) -> Vec<MatchAuditRecord> {
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    records
}
