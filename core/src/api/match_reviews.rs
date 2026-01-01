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
use crate::domain::{AuditAction, AuditLog, EntityType, MatchStatus};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};
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
    pub reviewed_by: String,
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
    pub reviewed_by: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdateResponse {
    pub success: bool,
    pub updated_count: usize,
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
    let status = parse_action(&req.action)?;

    let params = crate::repository::UpdateMatchStatusParams::new(
        id,
        status,
        &req.reviewed_by,
        req.notes.as_deref().unwrap_or(""),
    );

    state
        .match_repo
        .update_status(params)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let audit_action = match status {
        MatchStatus::Confirmed => AuditAction::MatchConfirmed,
        MatchStatus::Rejected => AuditAction::MatchRejected,
        _ => AuditAction::MatchCreated,
    };

    let audit_log = AuditLog::new(audit_action, EntityType::Match, id, &req.reviewed_by)
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
        let params =
            crate::repository::UpdateMatchStatusParams::new(*id, status, &req.reviewed_by, "");

        if state.match_repo.update_status(params).await.is_ok() {
            updated_count += 1;

            let audit_action = match status {
                MatchStatus::Confirmed => AuditAction::MatchConfirmed,
                MatchStatus::Rejected => AuditAction::MatchRejected,
                _ => AuditAction::MatchCreated,
            };

            let audit_log = AuditLog::new(audit_action, EntityType::Match, *id, &req.reviewed_by)
                .with_details(serde_json::json!({ "bulk": true, "action": req.action }));

            let _ = state.audit_log_repo.save(&audit_log).await;
        }
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

    Ok(Json(MatchReviewStats {
        pending,
        confirmed_today: 0,
        rejected_today: 0,
        total_pending: pending,
        avg_confidence,
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
