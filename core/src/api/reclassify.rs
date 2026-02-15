//! Reclassification API Handlers
//!
//! Endpoints for reclassifying offers as requests and vice versa.
//! This allows operators to correct AI misclassifications.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::domain::{AuditAction, AuditLog, EntityType};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};
use crate::ws::WsEvent;
use pharma_db::entity::common::ItemStatus;
use pharma_db::entity::offer::Model as OfferModel;
use pharma_db::entity::request::Model as RequestModel;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Target type for reclassification
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Offer,
    Request,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Offer => write!(f, "offer"),
            ItemType::Request => write!(f, "request"),
        }
    }
}

/// Request body for reclassifying an item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReclassifyRequest {
    /// The ID of the item to reclassify
    pub source_id: Uuid,
    /// Current type of the item
    pub source_type: ItemType,
    /// Target type to convert to
    pub target_type: ItemType,
    /// User performing the reclassification
    pub reclassified_by: Uuid,
    /// Optional notes explaining the reclassification
    pub notes: Option<String>,
}

/// Response after successful reclassification
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReclassifyResponse {
    pub success: bool,
    /// ID of the original item (now marked as cancelled)
    pub source_id: Uuid,
    /// ID of the newly created item
    pub new_id: Uuid,
    /// Type of the new item
    pub new_type: ItemType,
    /// Message describing the action
    pub message: String,
}

/// Summary of an item for display
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummary {
    pub id: Uuid,
    pub item_type: ItemType,
    pub medication: String,
    pub medication_raw: String,
    pub quantity: Option<String>,
    pub price: Option<String>,
    pub status: String,
    pub created_at: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Reclassify an offer as a request or vice versa
/// POST /api/reclassify
pub async fn reclassify_item<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ReclassifyRequest>,
) -> Result<Json<ReclassifyResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Validate that source and target types are different
    if req.source_type == req.target_type {
        return Err((
            StatusCode::BAD_REQUEST,
            "Source and target types must be different".to_string(),
        ));
    }

    let (new_id, message) = match (req.source_type, req.target_type) {
        (ItemType::Offer, ItemType::Request) => reclassify_offer_to_request(&state, &req).await?,
        (ItemType::Request, ItemType::Offer) => reclassify_request_to_offer(&state, &req).await?,
        _ => unreachable!(),
    };

    // Create audit log
    let audit_log = AuditLog::new(
        AuditAction::ItemReclassified,
        match req.source_type {
            ItemType::Offer => EntityType::Offer,
            ItemType::Request => EntityType::Request,
        },
        req.source_id,
        req.reclassified_by,
    )
    .with_details(serde_json::json!({
        "source_type": req.source_type.to_string(),
        "target_type": req.target_type.to_string(),
        "new_id": new_id,
        "notes": req.notes,
    }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, "Failed to save reclassification audit log");
    }

    // Broadcast WebSocket event
    let ws_event = WsEvent::ItemReclassified {
        source_id: req.source_id,
        source_type: req.source_type.to_string(),
        new_id,
        new_type: req.target_type.to_string(),
        user_id: req.reclassified_by,
    };
    let _ = state.ws_tx.send(ws_event);

    tracing::info!(
        source_id = %req.source_id,
        source_type = %req.source_type,
        new_id = %new_id,
        new_type = %req.target_type,
        reclassified_by = %req.reclassified_by,
        "Item reclassified successfully"
    );

    Ok(Json(ReclassifyResponse {
        success: true,
        source_id: req.source_id,
        new_id,
        new_type: req.target_type,
        message,
    }))
}

/// Get an item by ID (either offer or request)
/// GET /api/items/:type/:id
pub async fn get_item<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path((item_type, id)): Path<(String, Uuid)>,
) -> Result<Json<ItemSummary>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    match item_type.to_lowercase().as_str() {
        "offer" => {
            let offer = state
                .offer_repo
                .get_by_id(id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

            Ok(Json(offer_to_summary(&offer)))
        }
        "request" => {
            let request = state
                .request_repo
                .get_by_id(id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

            Ok(Json(request_to_summary(&request)))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            "Invalid item type. Must be 'offer' or 'request'".to_string(),
        )),
    }
}

// ============================================================================
// Internal Functions
// ============================================================================

/// Convert an offer to a request
async fn reclassify_offer_to_request<RQ, A, MM>(
    state: &AppState<RQ, A, MM>,
    req: &ReclassifyRequest,
) -> Result<(Uuid, String), (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get the original offer
    let offer = state
        .offer_repo
        .get_by_id(req.source_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

    // Create a new request from the offer data
    let new_request = RequestModel {
        id: Uuid::new_v4(),
        raw_message_id: offer.raw_message_id,
        participant_id: offer.participant_id,
        group_id: offer.group_id,
        medication: offer.medication.clone(),
        medication_raw: offer.medication_raw.clone(),
        unit: offer.unit.clone(),
        urgency_level: offer.urgency_level,
        expiry_requirement: offer.expiry_info.clone(),
        ai_confidence: offer.ai_confidence,
        notes: Some(format!(
            "Reclassified from offer {}. {}",
            offer.id,
            req.notes.as_deref().unwrap_or("")
        )),
        status: ItemStatus::Active,
        content_embedding: offer.content_embedding.clone(),
        master_medication_id: offer.master_medication_id,
        medication_curated: offer.medication_curated,
        confirmed_match_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Save the new request
    state
        .request_repo
        .save(&new_request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Mark the original offer as cancelled
    state
        .offer_repo
        .update_status(req.source_id, ItemStatus::Cancelled)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Cancel all pending matches that reference this offer
    if let Err(e) = state
        .match_repo
        .cancel_matches_for_offer(req.source_id)
        .await
    {
        tracing::warn!(error = %e, offer_id = %req.source_id, "Failed to cancel matches for reclassified offer");
    }

    // Broadcast new request event
    let _ = state.ws_tx.send(WsEvent::NewRequest(new_request.clone()));

    Ok((
        new_request.id,
        format!("Offer '{}' reclassified as request", offer.medication),
    ))
}

/// Convert a request to an offer
async fn reclassify_request_to_offer<RQ, A, MM>(
    state: &AppState<RQ, A, MM>,
    req: &ReclassifyRequest,
) -> Result<(Uuid, String), (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get the original request
    let request = state
        .request_repo
        .get_by_id(req.source_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Create a new offer from the request data
    let new_offer = OfferModel {
        id: Uuid::new_v4(),
        raw_message_id: request.raw_message_id,
        participant_id: request.participant_id,
        group_id: request.group_id,
        medication: request.medication.clone(),
        medication_raw: request.medication_raw.clone(),
        unit: request.unit.clone(),
        batch_number: None,
        notes: Some(format!(
            "Reclassified from request {}. {}",
            request.id,
            req.notes.as_deref().unwrap_or("")
        )),
        status: ItemStatus::Active,
        urgency_level: request.urgency_level,
        expiry_info: request.expiry_requirement.clone(),
        ai_confidence: request.ai_confidence,
        content_embedding: request.content_embedding.clone(),
        master_medication_id: request.master_medication_id,
        medication_curated: request.medication_curated,
        confirmed_match_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Save the new offer
    state
        .offer_repo
        .save(&new_offer)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Mark the original request as cancelled
    state
        .request_repo
        .update_status(req.source_id, ItemStatus::Cancelled)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Cancel all pending matches that reference this request
    if let Err(e) = state
        .match_repo
        .cancel_matches_for_request(req.source_id)
        .await
    {
        tracing::warn!(error = %e, request_id = %req.source_id, "Failed to cancel matches for reclassified request");
    }

    // Broadcast new offer event
    let _ = state.ws_tx.send(WsEvent::NewOffer(new_offer.clone()));

    Ok((
        new_offer.id,
        format!("Request '{}' reclassified as offer", request.medication),
    ))
}

fn offer_to_summary(offer: &OfferModel) -> ItemSummary {
    ItemSummary {
        id: offer.id,
        item_type: ItemType::Offer,
        medication: offer.medication.clone(),
        medication_raw: offer.medication_raw.clone(),
        quantity: None, // Removed from schema
        price: None,    // Removed from schema
        status: format!("{:?}", offer.status),
        created_at: offer.created_at.to_rfc3339(),
    }
}

fn request_to_summary(request: &RequestModel) -> ItemSummary {
    ItemSummary {
        id: request.id,
        item_type: ItemType::Request,
        medication: request.medication.clone(),
        medication_raw: request.medication_raw.clone(),
        quantity: None, // Removed from schema
        price: None,    // Removed from schema
        status: format!("{:?}", request.status),
        created_at: request.created_at.to_rfc3339(),
    }
}
