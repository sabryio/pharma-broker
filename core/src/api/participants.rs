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

/// Get common groups between two participants
/// GET /api/participants/common-groups/:jid1/:jid2
pub async fn get_common_groups<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path((jid1, jid2)): Path<(String, String)>,
) -> Result<Json<CommonGroupsResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get both participants
    let participant1 = state
        .participant_repo
        .get_by_jid(&jid1)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Participant with JID {} not found", jid1),
            )
        })?;

    let participant2 = state
        .participant_repo
        .get_by_jid(&jid2)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Participant with JID {} not found", jid2),
            )
        })?;

    // Get groups for both participants
    let groups1 = state
        .participant_repo
        .get_groups(participant1.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let groups2 = state
        .participant_repo
        .get_groups(participant2.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Find common groups (by JID)
    let groups1_jids: std::collections::HashSet<_> = groups1.iter().map(|g| &g.jid).collect();
    let common_groups: Vec<_> = groups2
        .into_iter()
        .filter(|g| groups1_jids.contains(&g.jid))
        .collect();

    let total = common_groups.len();

    Ok(Json(CommonGroupsResponse {
        success: true,
        common_groups,
        total,
    }))
}

#[derive(Debug, Serialize)]
pub struct CommonGroupsResponse {
    pub success: bool,
    pub common_groups: Vec<crate::domain::Group>,
    pub total: usize,
}
