//! Review Queue API Handlers
//!
//! Endpoints for viewing and managing AI parse results that require human review.
//! Part of Task 3.3: Review Queue implementation.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::domain::{
    AuditAction, AuditLog, EntityType, ReviewQueueItem, ReviewQueueStats, ReviewStatus,
};
use crate::repository::{
    AuditLogRepository, FeedbackRecordRepository, GroupRepository, MatchRepository,
    OfferRepository, RequestRepository, ReviewQueueRepository,
};

// ============================================================================
// Request/Response Types
// ============================================================================

/// Pagination query parameters
#[derive(Debug, Deserialize)]
pub struct ReviewPagination {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    /// Filter by status (optional)
    pub status: Option<String>,
}

fn default_limit() -> i64 {
    20
}

/// Response for list endpoints
#[derive(Debug, Serialize)]
pub struct ReviewQueueListResponse {
    pub items: Vec<ReviewQueueItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Request to update review status
#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    /// New status: "approved", "rejected", or "skipped"
    pub status: String,
    /// ID of the reviewer
    pub reviewed_by: String,
    /// Optional notes
    pub notes: Option<String>,
}

/// Response after updating a review
#[derive(Debug, Serialize)]
pub struct UpdateReviewResponse {
    pub success: bool,
    pub id: String,
    pub new_status: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// Get review queue items (paginated)
/// GET /api/review-queue
///
/// Query parameters:
/// - limit: Max items to return (default 20)
/// - offset: Number of items to skip (default 0)
/// - status: Filter by status (optional: pending, approved, rejected, skipped)
pub async fn get_review_queue<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Query(pagination): Query<ReviewPagination>,
) -> Result<Json<ReviewQueueListResponse>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let items = match pagination.status.as_deref() {
        Some("pending") => {
            state
                .review_queue_repo
                .get_pending(pagination.limit, pagination.offset)
                .await
        }
        Some(status_str) => {
            let status = parse_status(status_str)?;
            state
                .review_queue_repo
                .get_by_status(status, pagination.limit, pagination.offset)
                .await
        }
        None => {
            state
                .review_queue_repo
                .get_pending(pagination.limit, pagination.offset)
                .await
        }
    };

    let items = items.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state
        .review_queue_repo
        .count_pending()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ReviewQueueListResponse {
        items,
        total,
        limit: pagination.limit,
        offset: pagination.offset,
    }))
}

/// Get a single review queue item by ID
/// GET /api/review-queue/:id
pub async fn get_review_item<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Path(id): Path<String>,
) -> Result<Json<ReviewQueueItem>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let item = state
        .review_queue_repo
        .get_by_id(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match item {
        Some(item) => Ok(Json(item)),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Review item {} not found", id),
        )),
    }
}

/// Update review item status (approve, reject, or skip)
/// POST /api/review-queue/:id/review
pub async fn update_review_status<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateReviewRequest>,
) -> Result<Json<UpdateReviewResponse>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    // Parse status
    let status = parse_status(&req.status)?;

    // Update the status
    state
        .review_queue_repo
        .update_status(&id, status.clone(), &req.reviewed_by, req.notes.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Task 5.3: Audit Logging
    let audit_action = match status {
        ReviewStatus::Approved => AuditAction::ReviewApproved,
        ReviewStatus::Rejected => AuditAction::ReviewRejected,
        ReviewStatus::Skipped => AuditAction::ReviewSkipped,
        ReviewStatus::Pending => AuditAction::ReviewQueued, // Should not happen here but for completeness
    };

    let audit_log = AuditLog::new(audit_action, EntityType::ReviewQueue, &id, &req.reviewed_by)
        .with_details(serde_json::json!({
            "status": req.status,
            "notes": req.notes
        }));

    if let Err(e) = state.audit_log_repo.save(&audit_log).await {
        tracing::warn!(error = %e, id = %id, "Failed to save audit log for review status update");
    }

    tracing::info!(
        id = %id,
        status = %req.status,
        reviewed_by = %req.reviewed_by,
        "Review queue item status updated"
    );

    Ok(Json(UpdateReviewResponse {
        success: true,
        id,
        new_status: req.status,
    }))
}

/// Get review queue statistics
/// GET /api/review-queue/stats
pub async fn get_review_stats<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
) -> Result<Json<ReviewQueueStats>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let stats = state
        .review_queue_repo
        .get_stats()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(stats))
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse status string to ReviewStatus enum
fn parse_status(status: &str) -> Result<ReviewStatus, (StatusCode, String)> {
    match status.to_lowercase().as_str() {
        "pending" => Ok(ReviewStatus::Pending),
        "approved" => Ok(ReviewStatus::Approved),
        "rejected" => Ok(ReviewStatus::Rejected),
        "skipped" => Ok(ReviewStatus::Skipped),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid status '{}'. Must be one of: pending, approved, rejected, skipped",
                status
            ),
        )),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_valid() {
        assert_eq!(parse_status("pending").unwrap(), ReviewStatus::Pending);
        assert_eq!(parse_status("APPROVED").unwrap(), ReviewStatus::Approved);
        assert_eq!(parse_status("Rejected").unwrap(), ReviewStatus::Rejected);
        assert_eq!(parse_status("skipped").unwrap(), ReviewStatus::Skipped);
    }

    #[test]
    fn test_parse_status_invalid() {
        let result = parse_status("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 20);
    }
}
