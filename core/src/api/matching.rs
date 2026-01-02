//! Matching API Handlers
//!
//! Endpoints for manually triggering the matching process.

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::reparse::ItemType;
use super::routes::AppState;

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
}

// ============================================================================
// Handlers
// ============================================================================

/// Manually trigger re-matching for an offer or request
pub async fn rematch_item<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<RematchRequest>,
) -> Result<Json<RematchResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    match req.item_type {
        ItemType::Offer => {
            // Verify offer exists
            let _ = state
                .offer_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

            // Delete pending matches to allow re-matching
            let _ = state
                .match_repo
                .delete_pending_matches_for_offer(req.item_id)
                .await;

            // Trigger re-matching: Since workers are request-centric,
            // we enqueue all active requests that might match this offer.
            if let Ok(active_requests) = state.request_repo.get_active(100, 0).await {
                for r in active_requests {
                    let _ = state.match_queue_repo.enqueue(r.id, 0).await;
                }
            }
        }
        ItemType::Request => {
            // Verify request exists
            let _ = state
                .request_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

            // Delete pending matches to allow re-matching
            let _ = state
                .match_repo
                .delete_pending_matches_for_request(req.item_id)
                .await;

            // Trigger re-matching for this request
            let _ = state.match_queue_repo.enqueue(req.item_id, 0).await;
        }
    }

    Ok(Json(RematchResponse {
        success: true,
        message: "Rematch triggered successfully".to_string(),
    }))
}
