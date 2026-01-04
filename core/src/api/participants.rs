//! Participants API Handlers
//!
//! Endpoints for participant/sender information and statistics.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;
use uuid::Uuid;

use super::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};
use pharma_db::traits::ParticipantStats;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantStatsResponse {
    pub participant_id: Uuid,
    pub display_name: Option<String>,
    pub phone: Option<String>,
    pub jid: Option<String>,
    #[serde(flatten)]
    pub stats: ParticipantStats,
}

// ============================================================================
// Handlers
// ============================================================================

/// Get participant statistics
/// GET /api/participants/:id/stats
pub async fn get_participant_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ParticipantStatsResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get participant info
    let participant = state
        .participant_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Participant {} not found", id),
            )
        })?;

    // Get stats
    let stats = state
        .participant_repo
        .get_stats(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ParticipantStatsResponse {
        participant_id: id,
        display_name: participant.display_name.or(participant.push_name),
        phone: Some(participant.phone),
        jid: Some(participant.jid),
        stats,
    }))
}

/// Get participant by JID
/// GET /api/participants/by-jid/:jid
pub async fn get_participant_by_jid<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(jid): Path<String>,
) -> Result<Json<ParticipantStatsResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get participant by JID
    let participant = state
        .participant_repo
        .get_by_jid(&jid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Participant with JID {} not found", jid),
            )
        })?;

    // Get stats
    let stats = state
        .participant_repo
        .get_stats(participant.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ParticipantStatsResponse {
        participant_id: participant.id,
        display_name: participant.display_name.or(participant.push_name),
        phone: Some(participant.phone),
        jid: Some(participant.jid),
        stats,
    }))
}
