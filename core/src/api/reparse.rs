//! Re-parse API Handlers
//!
//! Endpoints for re-triggering AI parsing on offers/requests.
//! This allows operators to correct AI misidentifications.

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::ai::Intent;
use crate::domain::{AuditAction, AuditLog, EntityType};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};
use crate::ws::WsEvent;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Item type for reparse
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

/// Request body for re-parsing an item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReparseRequest {
    /// The ID of the item to re-parse
    pub item_id: Uuid,
    /// Type of the item (offer or request)
    pub item_type: ItemType,
    /// User performing the re-parse
    pub reparsed_by: Uuid,
    /// Optional hint for the AI (e.g., correct medication name)
    pub hint: Option<String>,
    /// Optional correction feedback explaining what the AI got wrong
    /// e.g., "3/26 is an expiry date (March 2026), not a dosage"
    pub correction: Option<String>,
}

/// Response after successful re-parse
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReparseResponse {
    pub success: bool,
    pub item_id: Uuid,
    pub item_type: ItemType,
    /// Previous medication name
    pub previous_medication: String,
    /// New medication name after re-parse
    pub new_medication: String,
    /// AI confidence for the new parse
    pub ai_confidence: f64,
    pub message: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Re-parse an offer or request with AI
/// POST /api/reparse
pub async fn reparse_item<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ReparseRequest>,
) -> Result<Json<ReparseResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get the raw message content based on item type
    let (raw_message_id, previous_medication, group_id, participant_id) = match req.item_type {
        ItemType::Offer => {
            let offer = state
                .offer_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;
            (
                offer.raw_message_id,
                offer.medication,
                offer.group_id,
                offer.participant_id,
            )
        }
        ItemType::Request => {
            let request = state
                .request_repo
                .get_by_id(req.item_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;
            (
                request.raw_message_id,
                request.medication,
                request.group_id,
                request.participant_id,
            )
        }
    };

    // Get the raw message
    let raw_message = state
        .raw_message_repo
        .get_by_id(raw_message_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Raw message not found".to_string()))?;

    // Get group name for context
    let group = state.group_repo.get_by_id(group_id).await.ok().flatten();
    let group_name = group
        .map(|g| g.name)
        .unwrap_or_else(|| "Unknown".to_string());

    // Get participant name for context
    let participant = state
        .participant_repo
        .get_by_id(participant_id)
        .await
        .ok()
        .flatten();
    let sender_name = participant.and_then(|p| p.display_name.or(p.push_name));

    // Build the message content with optional hint and correction
    let mut content = raw_message.content.clone();

    // Add correction feedback first (more specific)
    if let Some(correction) = &req.correction {
        content = format!("{}\n\n[CORRECTION FROM REVIEWER: {}]", content, correction);
    }

    // Add hint for correct medication name
    if let Some(hint) = &req.hint {
        content = format!("{}\n\n[CORRECT MEDICATION NAME: {}]", content, hint);
    }

    // Re-parse with AI
    let parse_result = state
        .ai_client
        .parse(&content, sender_name.as_deref(), &group_name, None, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AI parsing failed: {}", e),
            )
        })?;

    // Find the matching medication in the parse result
    // Heuristic-based matching to find the specific medication the user is trying to re-parse
    let target_intent = match req.item_type {
        ItemType::Offer => Intent::Offer,
        ItemType::Request => Intent::Request,
    };

    // Check if the intent matches
    if parse_result.intent != target_intent {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "AI identified intent as {:?}, but expected {:?}",
                parse_result.intent, target_intent
            ),
        ));
    }

    let parsed_medication = if parse_result.medications.len() == 1 {
        // Only one medication returned, assume it's the one
        parse_result.medications.into_iter().next().unwrap()
    } else if parse_result.medications.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "AI did not identify any medications in the message".to_string(),
        ));
    } else {
        // Multiple medications returned. Use heuristics to find the best match.
        let hint_lower = req.hint.as_ref().map(|h| h.to_lowercase());
        let correction_lower = req.correction.as_ref().map(|c| c.to_lowercase());
        let prev_lower = previous_medication.to_lowercase();

        parse_result
            .medications
            .into_iter()
            .max_by_key(|med| {
                let name = med.name.to_lowercase();
                let mut score = 0;

                // Priority 1: Exact match with hint
                if let Some(hint) = &hint_lower
                    && (name.contains(hint) || hint.contains(&name))
                {
                    score += 100;
                }

                // Priority 2: Contains correction keywords
                if let Some(correction) = &correction_lower
                    && (name.contains(correction) || correction.contains(&name))
                {
                    score += 50;
                }

                // Priority 3: Similarity to previous medication
                // (useful if correction didn't change the name but something else like expiry)
                if name.contains(&prev_lower) || prev_lower.contains(&name) {
                    score += 20;
                }

                score
            })
            .unwrap()
    };

    // Update the item with new medication info
    let new_medication = parsed_medication.name.clone();
    let ai_confidence = parsed_medication.confidence;

    match req.item_type {
        ItemType::Offer => {
            state
                .offer_repo
                .update_medication(
                    req.item_id,
                    &parsed_medication.name,
                    Some(parsed_medication.confidence),
                )
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // Trigger re-matching: Since workers are request-centric,
            // we enqueue all active requests that might match this new offer.
            // For now, we enqueue up to 100 active requests to avoid overwhelming the queue.
            if let Ok(active_requests) = state.request_repo.get_active(100, 0).await {
                for r in active_requests {
                    let _ = state.match_queue_repo.enqueue(r.id, 0).await;
                }
            }
        }
        ItemType::Request => {
            state
                .request_repo
                .update_medication(
                    req.item_id,
                    &parsed_medication.name,
                    Some(parsed_medication.confidence),
                )
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // Trigger re-matching for this request
            let _ = state.match_queue_repo.enqueue(req.item_id, 0).await;
        }
    }

    // Create audit log
    let audit_log = AuditLog::new(
        AuditAction::ItemReparsed,
        match req.item_type {
            ItemType::Offer => EntityType::Offer,
            ItemType::Request => EntityType::Request,
        },
        req.item_id,
        req.reparsed_by,
    )
    .with_details(serde_json::json!({
        "previous_medication": previous_medication,
        "new_medication": new_medication,
        "ai_confidence": ai_confidence,
        "hint": req.hint,
    }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, "Failed to save reparse audit log");
    }

    // Broadcast WebSocket event
    let ws_event = WsEvent::ItemReparsed {
        item_id: req.item_id,
        item_type: req.item_type.to_string(),
        previous_medication: previous_medication.clone(),
        new_medication: new_medication.clone(),
        user_id: req.reparsed_by,
    };
    let _ = state.ws_tx.send(ws_event);

    tracing::info!(
        item_id = %req.item_id,
        item_type = %req.item_type,
        previous = %previous_medication,
        new = %new_medication,
        confidence = ai_confidence,
        "Item re-parsed successfully"
    );

    Ok(Json(ReparseResponse {
        success: true,
        item_id: req.item_id,
        item_type: req.item_type,
        previous_medication,
        new_medication: new_medication.clone(),
        ai_confidence,
        message: format!("Successfully re-parsed as '{}'", new_medication),
    }))
}
