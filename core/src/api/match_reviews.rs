//! Match Reviews API Handlers
//!
//! Endpoints for reviewing and managing offer-request matches.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{Datelike, Utc};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::domain::{AuditAction, AuditLog, EntityType, FeedbackRecord, MatchStatus};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};
use crate::ws::{MatchStatusEvent, WsEvent};
use pharma_db::traits::{MatchReviewItem, MatchReviewStats, OfferSummary, RequestSummary};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct MatchReviewPagination {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    pub status: Option<String>,
    pub min_score: Option<f64>,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReviewListResponse {
    pub items: Vec<MatchReviewItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMatchReviewRequest {
    pub action: String,
    pub reviewed_by: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMatchReviewResponse {
    pub success: bool,
    pub id: Uuid,
    pub new_status: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkUpdateRequest {
    pub ids: Vec<Uuid>,
    pub action: String,
    pub reviewed_by: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateResponse {
    pub success: bool,
    pub updated_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReAuditResponse {
    pub success: bool,
    pub match_id: Uuid,
    pub ai_status: Option<String>,
    pub ai_confidence: Option<f64>,
    pub ai_explanation: Option<String>,
    pub suggested_action: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecalculateConfidenceResponse {
    pub success: bool,
    pub match_id: Uuid,
    pub old_score: f64,
    pub new_score: f64,
    pub medication_similarity: f64,
    pub raw_similarity: f64,
    pub embedding_similarity: Option<f64>,
    pub reasoning: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// List match reviews with pagination
/// GET /api/match-reviews
pub async fn list_match_reviews<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(pagination): Query<MatchReviewPagination>,
) -> Result<Json<MatchReviewListResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let matches = state
        .match_repo
        .get_pending(pagination.limit, pagination.offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state
        .match_repo
        .count_pending()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut items = Vec::with_capacity(matches.len());

    for m in matches {
        if let Some(min) = pagination.min_score
            && m.score < min
        {
            continue;
        }

        let offer = state
            .offer_repo
            .get_by_id(m.offer_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let request = state
            .request_repo
            .get_by_id(m.request_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let (offer, request) = match (offer, request) {
            (Some(o), Some(r)) => (o, r),
            _ => continue,
        };

        let issues = parse_issues(m.reasoning.as_deref());

        // Fetch group names for source_group field
        let offer_group = state
            .group_repo
            .get_by_id(offer.group_id)
            .await
            .ok()
            .flatten();
        let request_group = state
            .group_repo
            .get_by_id(request.group_id)
            .await
            .ok()
            .flatten();

        // Fetch participant info for sender details
        let offer_participant = state
            .participant_repo
            .get_by_id(offer.participant_id)
            .await
            .ok()
            .flatten();
        let request_participant = state
            .participant_repo
            .get_by_id(request.participant_id)
            .await
            .ok()
            .flatten();

        // Fetch raw message content
        let offer_raw_message = state
            .raw_message_repo
            .get_by_id(offer.raw_message_id)
            .await
            .ok()
            .flatten();
        let request_raw_message = state
            .raw_message_repo
            .get_by_id(request.raw_message_id)
            .await
            .ok()
            .flatten();

        // Fetch curation metadata
        tracing::debug!(
            ">>> match_reviews: looking up offer alias for medication_raw: '{}'",
            &offer.medication_raw
        );
        let offer_curation = state
            .medication_alias_repo
            .get_by_name(&offer.medication_raw)
            .await
            .ok()
            .flatten();
        tracing::debug!(
            ">>> match_reviews: offer_curation found: {}",
            offer_curation.is_some()
        );

        let offer_summary = OfferSummary {
            id: offer.id,
            product: offer.medication.clone(),
            medication_raw: Some(offer.medication_raw.clone()),
            source: format!("Offer #{}", &offer.id.to_string()[..8]),
            source_group: offer_group.map(|g| g.name),
            sender_name: offer_participant
                .as_ref()
                .and_then(|p| p.display_name.clone().or_else(|| p.push_name.clone())),
            sender_jid: offer_participant.map(|p| p.jid),
            raw_message: offer_raw_message.map(|m| m.content),
            quantity: offer
                .quantity
                .map(|q| format!("{} {}", q, offer.unit.as_deref().unwrap_or("units"))),
            price: offer
                .price
                .and_then(|p| p.to_f64())
                .map(|p| format!("{:.0} {}", p, offer.currency.as_deref().unwrap_or("EGP"))),
            expiry: offer
                .expiry_date
                .map(|d| format!("{:02}/{}", d.month(), d.year())),
            master_id: offer_curation.as_ref().and_then(|a| a.master_medication_id),
            medication_alias_id: offer_curation.as_ref().map(|a| a.id),
            curation_status: offer_curation.map(|a| format!("{:?}", a.curation_status)),
        };

        let request_curation = state
            .medication_alias_repo
            .get_by_name(&request.medication_raw)
            .await
            .ok()
            .flatten();

        let request_summary = RequestSummary {
            id: request.id,
            product: request.medication.clone(),
            medication_raw: Some(request.medication_raw.clone()),
            source: format!("Request #{}", &request.id.to_string()[..8]),
            source_group: request_group.map(|g| g.name),
            sender_name: request_participant
                .as_ref()
                .and_then(|p| p.display_name.clone().or_else(|| p.push_name.clone())),
            sender_jid: request_participant.map(|p| p.jid),
            raw_message: request_raw_message.map(|m| m.content),
            quantity: request
                .quantity
                .map(|q| format!("{} {}", q, request.unit.as_deref().unwrap_or("units"))),
            max_price: request
                .max_price
                .and_then(|p| p.to_f64())
                .map(|p| format!("{:.0} {}", p, request.currency.as_deref().unwrap_or("EGP"))),
            urgency: format!("{:?}", request.urgency_level),
            master_id: request_curation
                .as_ref()
                .and_then(|a| a.master_medication_id),
            medication_alias_id: request_curation.as_ref().map(|a| a.id),
            curation_status: request_curation.map(|a| format!("{:?}", a.curation_status)),
        };

        items.push(MatchReviewItem {
            id: m.id,
            confidence: m.score * 100.0,
            status: m.status,
            reasoning: m.reasoning,
            issues,
            offer: offer_summary,
            request: request_summary,
            created_at: m.created_at,
            confirmed_at: m.confirmed_at,
            notes: m.notes,
            ai_status: m.ai_status,
            ai_confidence: m.ai_confidence,
            ai_explanation: m.ai_explanation,
        });
    }

    Ok(Json(MatchReviewListResponse {
        items,
        total,
        limit: pagination.limit,
        offset: pagination.offset,
    }))
}

/// Get a single match review by ID
/// GET /api/match-reviews/:id
pub async fn get_match_review<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<MatchReviewItem>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let m = state
        .match_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Match {} not found", id)))?;

    let offer = state
        .offer_repo
        .get_by_id(m.offer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

    let request = state
        .request_repo
        .get_by_id(m.request_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    let issues = parse_issues(m.reasoning.as_deref());

    // Fetch group names for source_group field
    let offer_group = state
        .group_repo
        .get_by_id(offer.group_id)
        .await
        .ok()
        .flatten();
    let request_group = state
        .group_repo
        .get_by_id(request.group_id)
        .await
        .ok()
        .flatten();

    // Fetch participant info for sender details
    let offer_participant = state
        .participant_repo
        .get_by_id(offer.participant_id)
        .await
        .ok()
        .flatten();
    let request_participant = state
        .participant_repo
        .get_by_id(request.participant_id)
        .await
        .ok()
        .flatten();

    // Fetch raw message content
    let offer_raw_message = state
        .raw_message_repo
        .get_by_id(offer.raw_message_id)
        .await
        .ok()
        .flatten();
    let request_raw_message = state
        .raw_message_repo
        .get_by_id(request.raw_message_id)
        .await
        .ok()
        .flatten();

    // Fetch curation metadata
    let offer_curation = state
        .medication_alias_repo
        .get_by_name(&offer.medication_raw)
        .await
        .ok()
        .flatten();

    let offer_summary = OfferSummary {
        id: offer.id,
        product: offer.medication.clone(),
        medication_raw: Some(offer.medication_raw.clone()),
        source: format!("Offer #{}", &offer.id.to_string()[..8]),
        source_group: offer_group.map(|g| g.name),
        sender_name: offer_participant
            .as_ref()
            .and_then(|p| p.display_name.clone().or_else(|| p.push_name.clone())),
        sender_jid: offer_participant.map(|p| p.jid),
        raw_message: offer_raw_message.map(|m| m.content),
        quantity: offer
            .quantity
            .map(|q| format!("{} {}", q, offer.unit.as_deref().unwrap_or("units"))),
        price: offer
            .price
            .and_then(|p| p.to_f64())
            .map(|p| format!("{:.0} {}", p, offer.currency.as_deref().unwrap_or("EGP"))),
        expiry: offer
            .expiry_date
            .map(|d| format!("{:02}/{}", d.month(), d.year())),
        master_id: offer_curation.as_ref().and_then(|a| a.master_medication_id),
        medication_alias_id: offer_curation.as_ref().map(|a| a.id),
        curation_status: offer_curation.map(|a| format!("{:?}", a.curation_status)),
    };

    let request_curation = state
        .medication_alias_repo
        .get_by_name(&request.medication_raw)
        .await
        .ok()
        .flatten();

    let request_summary = RequestSummary {
        id: request.id,
        product: request.medication.clone(),
        medication_raw: Some(request.medication_raw.clone()),
        source: format!("Request #{}", &request.id.to_string()[..8]),
        source_group: request_group.map(|g| g.name),
        sender_name: request_participant
            .as_ref()
            .and_then(|p| p.display_name.clone().or_else(|| p.push_name.clone())),
        sender_jid: request_participant.map(|p| p.jid),
        raw_message: request_raw_message.map(|m| m.content),
        quantity: request
            .quantity
            .map(|q| format!("{} {}", q, request.unit.as_deref().unwrap_or("units"))),
        max_price: request
            .max_price
            .and_then(|p| p.to_f64())
            .map(|p| format!("{:.0} {}", p, request.currency.as_deref().unwrap_or("EGP"))),
        urgency: format!("{:?}", request.urgency_level),
        master_id: request_curation
            .as_ref()
            .and_then(|a| a.master_medication_id),
        medication_alias_id: request_curation.as_ref().map(|a| a.id),
        curation_status: request_curation.map(|a| format!("{:?}", a.curation_status)),
    };

    Ok(Json(MatchReviewItem {
        id: m.id,
        confidence: m.score * 100.0,
        status: m.status,
        reasoning: m.reasoning,
        issues,
        offer: offer_summary,
        request: request_summary,
        created_at: m.created_at,
        confirmed_at: m.confirmed_at,
        notes: m.notes,
        ai_status: m.ai_status,
        ai_confidence: m.ai_confidence,
        ai_explanation: m.ai_explanation,
    }))
}

/// Update match review status (approve/reject)
/// PUT /api/match-reviews/:id/status
pub async fn update_match_review_status<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateMatchReviewRequest>,
) -> Result<Json<UpdateMatchReviewResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    tracing::info!(
        match_id = %id,
        action = %req.action,
        reviewed_by = %req.reviewed_by,
        ">>> [DEBUG] update_match_review_status called"
    );

    let status = parse_action(&req.action)?;
    tracing::info!(status = ?status, ">>> [DEBUG] Parsed status");

    // Get the match first to access offer_id and request_id
    let match_entity = state
        .match_repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, ">>> [DEBUG] Failed to get match by id");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| {
            tracing::error!(match_id = %id, ">>> [DEBUG] Match not found");
            (StatusCode::NOT_FOUND, "Match not found".to_string())
        })?;

    tracing::info!(
        match_id = %match_entity.id,
        current_status = ?match_entity.status,
        offer_id = %match_entity.offer_id,
        request_id = %match_entity.request_id,
        ">>> [DEBUG] Found match entity"
    );

    let params = crate::repository::UpdateMatchStatusParams::new(
        id,
        status,
        req.reviewed_by,
        req.notes.as_deref().unwrap_or(""),
    );

    tracing::info!(">>> [DEBUG] Calling match_repo.update_status");
    state.match_repo.update_status(params).await.map_err(|e| {
        tracing::error!(error = %e, ">>> [DEBUG] Failed to update match status");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    tracing::info!(">>> [DEBUG] Successfully updated match status in database");

    // If confirmed, increment match counts, record feedback and broadcast event
    // NOTE: We do NOT update offer/request status to MATCHED because:
    // - One offer can be matched with multiple requests
    // - One request can be matched with multiple offers
    if status == MatchStatus::Confirmed {
        tracing::info!(">>> [DEBUG] Processing confirmation - incrementing match counts");
        // Increment confirmed match count for both offer and request
        if let Err(e) = state
            .offer_repo
            .increment_match_count(match_entity.offer_id)
            .await
        {
            tracing::warn!(error = %e, offer_id = %match_entity.offer_id, "Failed to increment offer match count");
        }
        if let Err(e) = state
            .request_repo
            .increment_match_count(match_entity.request_id)
            .await
        {
            tracing::warn!(error = %e, request_id = %match_entity.request_id, "Failed to increment request match count");
        }

        // Record feedback for learning system
        let feedback = FeedbackRecord::new(
            id,
            req.reviewed_by,
            true,                      // confirmed = positive feedback
            match_entity.score * 0.9,  // Estimate medication score
            match_entity.score * 0.8,  // Estimate dosage score
            match_entity.score * 0.85, // Estimate quantity score
            match_entity.score * 0.95, // Estimate price score
            0.7,                       // Default recency score
            match_entity.score * 0.8,  // Estimate AI logic score
            match_entity.score,
        );

        if let Err(e) = state.feedback_repo.save(&feedback).await {
            tracing::warn!(error = %e, match_id = %id, "Failed to record confirmation feedback");
        }

        // Broadcast WebSocket event
        let ws_event = WsEvent::MatchConfirmed(MatchStatusEvent {
            match_id: id,
            user_id: req.reviewed_by,
            notes: req.notes.clone(),
            reason: None,
        });
        if let Err(e) = state.ws_tx.send(ws_event) {
            tracing::warn!(error = %e, "Failed to broadcast match confirmation event");
        }
    } else if status == MatchStatus::Rejected {
        tracing::info!(">>> [DEBUG] Processing rejection");
        // Record negative feedback for learning system
        let feedback = FeedbackRecord::new(
            id,
            req.reviewed_by,
            false, // rejected = negative feedback
            match_entity.score * 0.9,
            match_entity.score * 0.8,
            match_entity.score * 0.85,
            match_entity.score * 0.95,
            0.7,
            match_entity.score * 0.8,
            match_entity.score,
        );

        if let Err(e) = state.feedback_repo.save(&feedback).await {
            tracing::warn!(error = %e, match_id = %id, "Failed to record rejection feedback");
        }

        // Broadcast WebSocket event
        let ws_event = WsEvent::MatchRejected(MatchStatusEvent {
            match_id: id,
            user_id: req.reviewed_by,
            notes: req.notes.clone(),
            reason: Some("Rejected by reviewer".to_string()),
        });
        if let Err(e) = state.ws_tx.send(ws_event) {
            tracing::warn!(error = %e, "Failed to broadcast match rejection event");
        }
    }

    let audit_action = match status {
        MatchStatus::Confirmed => AuditAction::MatchConfirmed,
        MatchStatus::Rejected => AuditAction::MatchRejected,
        _ => AuditAction::MatchCreated,
    };

    let audit_log = AuditLog::new(audit_action, EntityType::Match, id, req.reviewed_by)
        .with_details(serde_json::json!({
            "action": req.action,
            "notes": req.notes
        }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, id = %id, "Failed to save audit log for match review");
    }

    tracing::info!(
        id = %id,
        action = %req.action,
        reviewed_by = %req.reviewed_by,
        "Match review status updated"
    );

    Ok(Json(UpdateMatchReviewResponse {
        success: true,
        id,
        new_status: format!("{:?}", status),
        reviewed_at: Some(Utc::now().to_rfc3339()),
    }))
}

/// Bulk update match reviews
/// POST /api/match-reviews/bulk
pub async fn bulk_update_match_reviews<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BulkUpdateRequest>,
) -> Result<Json<BulkUpdateResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let status = parse_action(&req.action)?;
    let mut updated_count = 0;

    for id in &req.ids {
        // Get the match first to access offer_id and request_id
        let match_entity = match state.match_repo.get_by_id(*id).await {
            Ok(Some(m)) => m,
            _ => continue,
        };

        let params =
            crate::repository::UpdateMatchStatusParams::new(*id, status, req.reviewed_by, "");

        if state.match_repo.update_status(params).await.is_ok() {
            updated_count += 1;

            // Record feedback and increment match counts (no offer/request status update for many-to-many matching)
            if status == MatchStatus::Confirmed {
                // Increment confirmed match count for both offer and request
                if let Err(e) = state
                    .offer_repo
                    .increment_match_count(match_entity.offer_id)
                    .await
                {
                    tracing::warn!(error = %e, offer_id = %match_entity.offer_id, "Failed to increment offer match count (bulk)");
                }
                if let Err(e) = state
                    .request_repo
                    .increment_match_count(match_entity.request_id)
                    .await
                {
                    tracing::warn!(error = %e, request_id = %match_entity.request_id, "Failed to increment request match count (bulk)");
                }

                let feedback = FeedbackRecord::new(
                    *id,
                    req.reviewed_by,
                    true,
                    match_entity.score * 0.9,
                    match_entity.score * 0.8,
                    match_entity.score * 0.85,
                    match_entity.score * 0.95,
                    0.7,
                    match_entity.score * 0.8,
                    match_entity.score,
                );
                let _ = state.feedback_repo.save(&feedback).await;
            } else if status == MatchStatus::Rejected {
                let feedback = FeedbackRecord::new(
                    *id,
                    req.reviewed_by,
                    false,
                    match_entity.score * 0.9,
                    match_entity.score * 0.8,
                    match_entity.score * 0.85,
                    match_entity.score * 0.95,
                    0.7,
                    match_entity.score * 0.8,
                    match_entity.score,
                );
                let _ = state.feedback_repo.save(&feedback).await;
            }

            let audit_action = match status {
                MatchStatus::Confirmed => AuditAction::MatchConfirmed,
                MatchStatus::Rejected => AuditAction::MatchRejected,
                _ => AuditAction::MatchCreated,
            };

            let audit_log = AuditLog::new(audit_action, EntityType::Match, *id, req.reviewed_by)
                .with_details(serde_json::json!({ "bulk": true, "action": req.action }));

            let _ = state.audit_log_repo.save(&audit_log).await;
        }
    }

    // Broadcast bulk update event
    if updated_count > 0 {
        let ws_event = if status == MatchStatus::Confirmed {
            WsEvent::BulkMatchUpdate {
                action: "confirmed".to_string(),
                count: updated_count,
                user_id: req.reviewed_by,
            }
        } else {
            WsEvent::BulkMatchUpdate {
                action: "rejected".to_string(),
                count: updated_count,
                user_id: req.reviewed_by,
            }
        };
        let _ = state.ws_tx.send(ws_event);
    }

    tracing::info!(
        count = updated_count,
        action = %req.action,
        reviewed_by = %req.reviewed_by,
        "Bulk match review completed"
    );

    Ok(Json(BulkUpdateResponse {
        success: true,
        updated_count,
    }))
}

/// Get match review statistics
/// GET /api/match-reviews/stats
pub async fn get_match_review_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<MatchReviewStats>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let pending = state
        .match_repo
        .count_pending()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let matches = state
        .match_repo
        .get_pending(100, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let avg_confidence = if matches.is_empty() {
        0.0
    } else {
        let sum: f64 = matches.iter().map(|m| m.score).sum();
        (sum / matches.len() as f64) * 100.0
    };

    let confirmed_today = state
        .match_repo
        .count_confirmed_today()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rejected_today = state
        .match_repo
        .count_rejected_today()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::debug!(
        pending = pending,
        confirmed_today = confirmed_today,
        rejected_today = rejected_today,
        avg_confidence = avg_confidence,
        ">>> [DEBUG] get_match_review_stats returning"
    );

    Ok(Json(MatchReviewStats {
        pending,
        confirmed_today,
        rejected_today,
        total_pending: pending,
        avg_confidence,
    }))
}

/// Re-trigger AI audit for a match
/// POST /api/match-reviews/:id/re-audit
pub async fn re_audit_match<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReAuditResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    tracing::info!(match_id = %id, "Re-auditing match with AI");

    // Get the match
    let match_entity = state
        .match_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Match not found".to_string()))?;

    // Get the offer and request
    let offer = state
        .offer_repo
        .get_by_id(match_entity.offer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

    let request = state
        .request_repo
        .get_by_id(match_entity.request_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Get the matching engine
    let matching_engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    // Call AI reviewer
    let review_result = matching_engine
        .ai_reviewer
        .audit_match(
            &offer,
            &request,
            match_entity.score,
            match_entity
                .reasoning
                .as_deref()
                .unwrap_or("Re-audit requested"),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AI audit failed: {}", e),
            )
        })?;

    tracing::info!(
        match_id = %id,
        status = ?review_result.status,
        confidence = %review_result.confidence,
        explanation = %review_result.explanation,
        "AI re-audit completed"
    );

    // Update the match with new AI results
    // We need to update the match entity with the new AI results
    let ai_status = format!("{:?}", review_result.status);
    let ai_confidence = review_result.confidence as f64;
    let ai_explanation = review_result.explanation.clone();

    // Update match in database with new AI results
    if let Err(e) = state
        .match_repo
        .update_ai_review(id, &ai_status, ai_confidence, &ai_explanation)
        .await
    {
        tracing::warn!(error = %e, match_id = %id, "Failed to update match with AI review results");
    }

    // Create audit log
    let audit_log = AuditLog::system(AuditAction::MatchReAudited, EntityType::Match, id)
        .with_details(serde_json::json!({
            "ai_status": ai_status,
            "ai_confidence": ai_confidence,
            "ai_explanation": ai_explanation
        }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, "Failed to save re-audit audit log");
    }

    Ok(Json(ReAuditResponse {
        success: true,
        match_id: id,
        ai_status: Some(ai_status),
        ai_confidence: Some(ai_confidence),
        ai_explanation: Some(ai_explanation),
        suggested_action: review_result.suggested_action,
    }))
}

/// Recalculate match confidence score
/// POST /api/match-reviews/:id/recalculate
pub async fn recalculate_confidence<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecalculateConfidenceResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    tracing::info!(match_id = %id, "Recalculating match confidence");

    // Get the match
    let match_entity = state
        .match_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Match not found".to_string()))?;

    let old_score = match_entity.score;

    // Get the offer and request
    let offer = state
        .offer_repo
        .get_by_id(match_entity.offer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

    let request = state
        .request_repo
        .get_by_id(match_entity.request_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Calculate medication similarity with raw text validation
    let medication_sim =
        crate::matching::medication_similarity(&offer.medication, &request.medication);
    let raw_sim =
        crate::matching::medication_similarity(&offer.medication_raw, &request.medication_raw);

    // Calculate combined similarity using raw text validation
    let combined_sim = crate::matching::medication_similarity_with_raw(
        &offer.medication,
        &request.medication,
        Some(&offer.medication_raw),
        Some(&request.medication_raw),
    );

    // Calculate embedding similarity if available
    let embedding_sim = match (&offer.content_embedding, &request.content_embedding) {
        (Some(o), Some(r)) => crate::matching::cosine_similarity(o.as_slice(), r.as_slice()).ok(),
        _ => None,
    };

    // Calculate new score
    // Use combined similarity (which includes raw text validation) as the primary factor
    let new_score = if let Some(emb_sim) = embedding_sim {
        // If embedding similarity is high but combined is low, trust combined
        if emb_sim > 0.8 && combined_sim < 0.5 {
            tracing::warn!(
                match_id = %id,
                embedding_sim = %emb_sim,
                combined_sim = %combined_sim,
                "High embedding but low combined similarity - using combined"
            );
            combined_sim
        } else {
            // Weighted: 40% embedding, 60% combined (with raw validation)
            emb_sim * 0.4 + combined_sim * 0.6
        }
    } else {
        combined_sim
    };

    // Build reasoning
    let reasoning = format!(
        "Medication: {:.1}%; Raw: {:.1}%; Combined: {:.1}%{}",
        medication_sim * 100.0,
        raw_sim * 100.0,
        combined_sim * 100.0,
        embedding_sim
            .map(|e| format!("; Embedding: {:.1}%", e * 100.0))
            .unwrap_or_default()
    );

    tracing::info!(
        match_id = %id,
        old_score = %old_score,
        new_score = %new_score,
        medication_sim = %medication_sim,
        raw_sim = %raw_sim,
        combined_sim = %combined_sim,
        embedding_sim = ?embedding_sim,
        "Recalculated match confidence"
    );

    // Update the match score in database
    if let Err(e) = state
        .match_repo
        .update_score(id, new_score, &reasoning)
        .await
    {
        tracing::warn!(error = %e, match_id = %id, "Failed to update match score");
    }

    // Create audit log for recalculation
    let audit_log = AuditLog::system(AuditAction::MatchRecalculated, EntityType::Match, id)
        .with_details(serde_json::json!({
            "old_score": old_score,
            "new_score": new_score,
            "medication_similarity": medication_sim,
            "raw_similarity": raw_sim,
            "embedding_similarity": embedding_sim,
            "reasoning": reasoning
        }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, "Failed to save recalculate audit log");
    }

    Ok(Json(RecalculateConfidenceResponse {
        success: true,
        match_id: id,
        old_score,
        new_score,
        medication_similarity: medication_sim,
        raw_similarity: raw_sim,
        embedding_similarity: embedding_sim,
        reasoning,
    }))
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_action(action: &str) -> Result<MatchStatus, (StatusCode, String)> {
    match action.to_lowercase().as_str() {
        "approved" | "confirmed" => Ok(MatchStatus::Confirmed),
        "rejected" => Ok(MatchStatus::Rejected),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid action '{}'. Must be 'approved' or 'rejected'",
                action
            ),
        )),
    }
}

fn parse_issues(reasoning: Option<&str>) -> Vec<String> {
    reasoning
        .map(|r| {
            r.split(';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        assert_eq!(parse_action("approved").unwrap(), MatchStatus::Confirmed);
        assert_eq!(parse_action("APPROVED").unwrap(), MatchStatus::Confirmed);
        assert_eq!(parse_action("rejected").unwrap(), MatchStatus::Rejected);
        assert!(parse_action("invalid").is_err());
    }

    #[test]
    fn test_parse_issues() {
        let issues = parse_issues(Some("Price mismatch; Quantity low"));
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0], "Price mismatch");
        assert_eq!(issues[1], "Quantity low");

        let empty = parse_issues(None);
        assert!(empty.is_empty());
    }
}
