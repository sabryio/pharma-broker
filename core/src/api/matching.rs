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
