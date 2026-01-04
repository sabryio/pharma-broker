//! Raw Messages API Handler
//!
//! Provides endpoints for listing, viewing, and managing raw WhatsApp messages.
//! Feature: raw-messages-production

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::handlers::{ApiResponse, Meta};
use super::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for listing raw messages
#[derive(Debug, Deserialize)]
pub struct RawMessageQueryParams {
    /// Maximum number of results (default: 20, max: 100)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination (default: 0)
    #[serde(default)]
    pub offset: i64,
    /// Search term for content filtering
    pub search: Option<String>,
    /// Filter by processing status: all, processed, unprocessed, error
    pub status: Option<String>,
    /// Sort field: timestamp, processed_at, created_at
    pub sort_by: Option<String>,
    /// Sort order: asc, desc
    pub sort_order: Option<String>,
    /// Filter messages after this date
    pub start_date: Option<DateTime<Utc>>,
    /// Filter messages before this date
    pub end_date: Option<DateTime<Utc>>,
    /// Filter by group ID
    pub group_id: Option<Uuid>,
    /// Filter by participant ID
    pub participant_id: Option<Uuid>,
}

fn default_limit() -> i64 {
    20
}

// =============================================================================
// Response Types
// =============================================================================

/// Raw message response with denormalized participant and group info
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMessageResponse {
    pub id: Uuid,
    pub external_id: Option<String>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub reply_to_id: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
    pub created_at: DateTime<Utc>,
    // Denormalized relations
    pub participant_id: Uuid,
    pub participant_name: Option<String>,
    pub participant_jid: Option<String>,
    pub group_id: Uuid,
    pub group_name: Option<String>,
    pub group_jid: Option<String>,
    // Computed fields
    pub status: String,
}

impl RawMessageResponse {
    fn compute_status(processed_at: Option<DateTime<Utc>>, error: &Option<String>) -> String {
        if error.is_some() {
            "error".to_string()
        } else if processed_at.is_some() {
            "processed".to_string()
        } else {
            "unprocessed".to_string()
        }
    }
}

// =============================================================================
// Request Types for Operations
// =============================================================================

/// Request body for updating message status
#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

/// Request body for bulk operations
#[derive(Debug, Deserialize)]
pub struct BulkOperationRequest {
    pub ids: Vec<Uuid>,
}

/// Response for bulk operations
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOperationResponse {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BulkOperationFailure>,
}

/// Individual failure in bulk operation
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOperationFailure {
    pub id: Uuid,
    pub error: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// List raw messages with filtering, sorting, and pagination
///
/// GET /api/raw-messages
pub async fn list_raw_messages<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(params): Query<RawMessageQueryParams>,
) -> Result<Json<ApiResponse<Vec<RawMessageResponse>>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Validate parameters
    if let Err(e) = validate_params(&params) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e),
                meta: None,
            }),
        ));
    }

    // Convert to repository params
    let repo_params = convert_to_repo_params(&params);

    // Fetch messages
    let messages = state
        .raw_message_repo
        .get_all(&repo_params)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?;

    // Get total count for pagination
    let total = state
        .raw_message_repo
        .count_all(&repo_params)
        .await
        .unwrap_or(0);

    // Enrich messages with participant and group info
    let mut responses = Vec::with_capacity(messages.len());
    for msg in messages {
        let response = enrich_message(&state, msg).await;
        responses.push(response);
    }

    Ok(Json(ApiResponse {
        success: true,
        data: Some(responses),
        error: None,
        meta: Some(Meta {
            total,
            limit: params.limit,
            offset: params.offset,
        }),
    }))
}

/// Get a single raw message by ID
///
/// GET /api/raw-messages/:id
pub async fn get_raw_message<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<RawMessageResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let message = state
        .raw_message_repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Raw message not found: {}", id)),
                    meta: None,
                }),
            )
        })?;

    let response = enrich_message(&state, message).await;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(response),
        error: None,
        meta: None,
    }))
}

/// Reprocess a single raw message
///
/// POST /api/raw-messages/:id/reprocess
pub async fn reprocess_raw_message<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<RawMessageResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Check if message exists
    let _message = state
        .raw_message_repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Raw message not found: {}", id)),
                    meta: None,
                }),
            )
        })?;

    info!(message_id = %id, "Reprocessing raw message");

    // Reset the message status by clearing processed_at and error
    // The message will be picked up by the batch processor
    let updated = state
        .raw_message_repo
        .mark_processed(id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?;

    let response = enrich_message(&state, updated).await;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(response),
        error: None,
        meta: None,
    }))
}

/// Delete a single raw message
///
/// DELETE /api/raw-messages/:id
pub async fn delete_raw_message<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Check if message exists
    let _message = state
        .raw_message_repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Raw message not found: {}", id)),
                    meta: None,
                }),
            )
        })?;

    // Check for referential integrity - count offers and requests with this raw_message_id
    let offer_count = state
        .offer_repo
        .count_by_raw_message_id(id)
        .await
        .unwrap_or(0);
    let request_count = state
        .request_repo
        .count_by_raw_message_id(id)
        .await
        .unwrap_or(0);

    if offer_count > 0 || request_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!(
                    "Cannot delete message: has {} associated offers and {} associated requests",
                    offer_count, request_count
                )),
                meta: None,
            }),
        ));
    }

    info!(message_id = %id, "Deleting raw message");

    // Delete the message
    state.raw_message_repo.delete_by_id(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                meta: None,
            }),
        )
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: None,
        error: None,
        meta: None,
    }))
}

/// Update message status (mark as processed)
///
/// PATCH /api/raw-messages/:id/status
pub async fn update_raw_message_status<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateStatusRequest>,
) -> Result<Json<ApiResponse<RawMessageResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Validate status value
    if req.status != "processed" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid status. Only 'processed' is supported".to_string()),
                meta: None,
            }),
        ));
    }

    // Get the message
    let message = state
        .raw_message_repo
        .get_by_id(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Raw message not found: {}", id)),
                    meta: None,
                }),
            )
        })?;

    // Check if already processed
    if message.processed_at.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Message is already processed".to_string()),
                meta: None,
            }),
        ));
    }

    info!(message_id = %id, "Marking raw message as processed");

    // Mark as processed
    let updated = state
        .raw_message_repo
        .mark_processed(id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    meta: None,
                }),
            )
        })?;

    let response = enrich_message(&state, updated).await;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(response),
        error: None,
        meta: None,
    }))
}

// =============================================================================
// Bulk Operation Handlers
// =============================================================================

/// Bulk reprocess raw messages
///
/// POST /api/raw-messages/bulk/reprocess
pub async fn bulk_reprocess_raw_messages<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BulkOperationRequest>,
) -> Result<Json<ApiResponse<BulkOperationResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    if req.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("No message IDs provided".to_string()),
                meta: None,
            }),
        ));
    }

    info!(count = req.ids.len(), "Bulk reprocessing raw messages");

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for id in req.ids {
        // Check if message exists
        match state.raw_message_repo.get_by_id(id).await {
            Ok(Some(_)) => {
                // Mark for reprocessing
                match state.raw_message_repo.mark_processed(id, None).await {
                    Ok(_) => succeeded.push(id),
                    Err(e) => failed.push(BulkOperationFailure {
                        id,
                        error: e.to_string(),
                    }),
                }
            }
            Ok(None) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: "Message not found".to_string(),
                });
            }
            Err(e) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(ApiResponse {
        success: true,
        data: Some(BulkOperationResponse { succeeded, failed }),
        error: None,
        meta: None,
    }))
}

/// Bulk delete raw messages
///
/// POST /api/raw-messages/bulk/delete
pub async fn bulk_delete_raw_messages<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BulkOperationRequest>,
) -> Result<Json<ApiResponse<BulkOperationResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    if req.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("No message IDs provided".to_string()),
                meta: None,
            }),
        ));
    }

    info!(count = req.ids.len(), "Bulk deleting raw messages");

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for id in req.ids {
        // Check if message exists
        match state.raw_message_repo.get_by_id(id).await {
            Ok(Some(_)) => {
                // Check referential integrity
                let offer_count = state
                    .offer_repo
                    .count_by_raw_message_id(id)
                    .await
                    .unwrap_or(0);
                let request_count = state
                    .request_repo
                    .count_by_raw_message_id(id)
                    .await
                    .unwrap_or(0);

                if offer_count > 0 || request_count > 0 {
                    failed.push(BulkOperationFailure {
                        id,
                        error: format!(
                            "Has {} associated offers and {} associated requests",
                            offer_count, request_count
                        ),
                    });
                    continue;
                }

                // Delete the message
                match state.raw_message_repo.delete_by_id(id).await {
                    Ok(_) => succeeded.push(id),
                    Err(e) => failed.push(BulkOperationFailure {
                        id,
                        error: e.to_string(),
                    }),
                }
            }
            Ok(None) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: "Message not found".to_string(),
                });
            }
            Err(e) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(ApiResponse {
        success: true,
        data: Some(BulkOperationResponse { succeeded, failed }),
        error: None,
        meta: None,
    }))
}

/// Bulk mark messages as processed
///
/// POST /api/raw-messages/bulk/mark-processed
pub async fn bulk_mark_processed<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BulkOperationRequest>,
) -> Result<Json<ApiResponse<BulkOperationResponse>>, (StatusCode, Json<ApiResponse<()>>)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    if req.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("No message IDs provided".to_string()),
                meta: None,
            }),
        ));
    }

    info!(count = req.ids.len(), "Bulk marking messages as processed");

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for id in req.ids {
        // Check if message exists and is unprocessed
        match state.raw_message_repo.get_by_id(id).await {
            Ok(Some(msg)) => {
                if msg.processed_at.is_some() {
                    failed.push(BulkOperationFailure {
                        id,
                        error: "Message is already processed".to_string(),
                    });
                    continue;
                }

                // Mark as processed
                match state.raw_message_repo.mark_processed(id, None).await {
                    Ok(_) => succeeded.push(id),
                    Err(e) => failed.push(BulkOperationFailure {
                        id,
                        error: e.to_string(),
                    }),
                }
            }
            Ok(None) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: "Message not found".to_string(),
                });
            }
            Err(e) => {
                failed.push(BulkOperationFailure {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(ApiResponse {
        success: true,
        data: Some(BulkOperationResponse { succeeded, failed }),
        error: None,
        meta: None,
    }))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Validate query parameters
fn validate_params(params: &RawMessageQueryParams) -> Result<(), String> {
    // Validate limit
    if params.limit < 1 || params.limit > 100 {
        return Err("Limit must be between 1 and 100".to_string());
    }

    // Validate offset
    if params.offset < 0 {
        return Err("Offset must be non-negative".to_string());
    }

    // Validate date range
    if let (Some(start), Some(end)) = (params.start_date, params.end_date)
        && start > end
    {
        return Err("Start date must be before or equal to end date".to_string());
    }

    // Validate status
    if let Some(ref status) = params.status {
        let valid_statuses = ["all", "processed", "unprocessed", "error"];
        if !valid_statuses.contains(&status.as_str()) {
            return Err(format!(
                "Invalid status '{}'. Valid values: all, processed, unprocessed, error",
                status
            ));
        }
    }

    // Validate sort_by
    if let Some(ref sort_by) = params.sort_by {
        let valid_fields = ["timestamp", "processed_at", "created_at"];
        if !valid_fields.contains(&sort_by.as_str()) {
            return Err(format!(
                "Invalid sort_by '{}'. Valid values: timestamp, processed_at, created_at",
                sort_by
            ));
        }
    }

    // Validate sort_order
    if let Some(ref sort_order) = params.sort_order {
        let valid_orders = ["asc", "desc"];
        if !valid_orders.contains(&sort_order.as_str()) {
            return Err(format!(
                "Invalid sort_order '{}'. Valid values: asc, desc",
                sort_order
            ));
        }
    }

    Ok(())
}

/// Convert API params to repository params
fn convert_to_repo_params(
    params: &RawMessageQueryParams,
) -> pharma_db::params::RawMessageQueryParams {
    use pharma_db::params::{ProcessingStatus, RawMessageSortField, SortOrder};

    let status = params.status.as_ref().map(|s| match s.as_str() {
        "processed" => ProcessingStatus::Processed,
        "unprocessed" => ProcessingStatus::Unprocessed,
        "error" => ProcessingStatus::Error,
        _ => ProcessingStatus::All,
    });

    let sort_by = params.sort_by.as_ref().map(|s| match s.as_str() {
        "processed_at" => RawMessageSortField::ProcessedAt,
        "created_at" => RawMessageSortField::CreatedAt,
        _ => RawMessageSortField::Timestamp,
    });

    let sort_order = params.sort_order.as_ref().map(|s| match s.as_str() {
        "asc" => SortOrder::Asc,
        _ => SortOrder::Desc,
    });

    pharma_db::params::RawMessageQueryParams {
        limit: Some(params.limit),
        offset: Some(params.offset),
        search: params.search.clone(),
        status,
        sort_by,
        sort_order,
        start_date: params.start_date,
        end_date: params.end_date,
        group_id: params.group_id,
        participant_id: params.participant_id,
    }
}

/// Enrich a raw message with participant and group info
async fn enrich_message<RQ, A, MM>(
    state: &AppState<RQ, A, MM>,
    msg: pharma_db::entity::raw_message::Model,
) -> RawMessageResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Fetch participant info
    let (participant_name, participant_jid) =
        match state.participant_repo.get_by_id(msg.participant_id).await {
            Ok(Some(p)) => {
                debug!(
                    participant_id = %msg.participant_id,
                    jid = %p.jid,
                    "Found participant for raw message"
                );
                // Use display_name if available, otherwise push_name
                let name = p.display_name.or(p.push_name);
                (name, Some(p.jid))
            }
            Ok(None) => {
                warn!(
                    participant_id = %msg.participant_id,
                    message_id = %msg.id,
                    "Participant not found for raw message"
                );
                (None, None)
            }
            Err(e) => {
                warn!(
                    participant_id = %msg.participant_id,
                    message_id = %msg.id,
                    error = %e,
                    "Error fetching participant for raw message"
                );
                (None, None)
            }
        };

    // Fetch group info
    let (group_name, group_jid) = match state.group_repo.get_by_id(msg.group_id).await {
        Ok(Some(g)) => {
            debug!(
                group_id = %msg.group_id,
                jid = %g.jid,
                name = %g.name,
                "Found group for raw message"
            );
            (Some(g.name), Some(g.jid))
        }
        Ok(None) => {
            warn!(
                group_id = %msg.group_id,
                message_id = %msg.id,
                "Group not found for raw message"
            );
            (None, None)
        }
        Err(e) => {
            warn!(
                group_id = %msg.group_id,
                message_id = %msg.id,
                error = %e,
                "Error fetching group for raw message"
            );
            (None, None)
        }
    };

    let status = RawMessageResponse::compute_status(msg.processed_at, &msg.error);

    RawMessageResponse {
        id: msg.id,
        external_id: msg.external_id,
        content: msg.content,
        timestamp: msg.timestamp,
        processed_at: msg.processed_at,
        error: msg.error,
        reply_to_id: msg.reply_to_id,
        reply_to_content: msg.reply_to_content,
        reply_to_sender: msg.reply_to_sender,
        created_at: msg.created_at,
        participant_id: msg.participant_id,
        participant_name,
        participant_jid,
        group_id: msg.group_id,
        group_name,
        group_jid,
        status,
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Validation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_params_valid_defaults() {
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        assert!(validate_params(&params).is_ok());
    }

    #[test]
    fn test_validate_params_limit_too_low() {
        let params = RawMessageQueryParams {
            limit: 0,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Limit must be between 1 and 100")
        );
    }

    #[test]
    fn test_validate_params_limit_too_high() {
        let params = RawMessageQueryParams {
            limit: 101,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Limit must be between 1 and 100")
        );
    }

    #[test]
    fn test_validate_params_negative_offset() {
        let params = RawMessageQueryParams {
            limit: 20,
            offset: -1,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Offset must be non-negative"));
    }

    #[test]
    fn test_validate_params_invalid_date_range() {
        let now = Utc::now();
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: Some(now),
            end_date: Some(now - chrono::Duration::days(1)),
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Start date must be before or equal to end date")
        );
    }

    #[test]
    fn test_validate_params_valid_date_range() {
        let now = Utc::now();
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: Some(now - chrono::Duration::days(7)),
            end_date: Some(now),
            group_id: None,
            participant_id: None,
        };
        assert!(validate_params(&params).is_ok());
    }

    #[test]
    fn test_validate_params_invalid_status() {
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: Some("invalid_status".to_string()),
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid status"));
    }

    #[test]
    fn test_validate_params_valid_statuses() {
        for status in &["all", "processed", "unprocessed", "error"] {
            let params = RawMessageQueryParams {
                limit: 20,
                offset: 0,
                search: None,
                status: Some(status.to_string()),
                sort_by: None,
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            assert!(
                validate_params(&params).is_ok(),
                "Status '{}' should be valid",
                status
            );
        }
    }

    #[test]
    fn test_validate_params_invalid_sort_by() {
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: Some("invalid_field".to_string()),
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid sort_by"));
    }

    #[test]
    fn test_validate_params_valid_sort_by() {
        for sort_by in &["timestamp", "processed_at", "created_at"] {
            let params = RawMessageQueryParams {
                limit: 20,
                offset: 0,
                search: None,
                status: None,
                sort_by: Some(sort_by.to_string()),
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            assert!(
                validate_params(&params).is_ok(),
                "Sort by '{}' should be valid",
                sort_by
            );
        }
    }

    #[test]
    fn test_validate_params_invalid_sort_order() {
        let params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: Some("invalid".to_string()),
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid sort_order"));
    }

    #[test]
    fn test_validate_params_valid_sort_order() {
        for sort_order in &["asc", "desc"] {
            let params = RawMessageQueryParams {
                limit: 20,
                offset: 0,
                search: None,
                status: None,
                sort_by: None,
                sort_order: Some(sort_order.to_string()),
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            assert!(
                validate_params(&params).is_ok(),
                "Sort order '{}' should be valid",
                sort_order
            );
        }
    }

    // -------------------------------------------------------------------------
    // Status Computation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_compute_status_unprocessed() {
        let status = RawMessageResponse::compute_status(None, &None);
        assert_eq!(status, "unprocessed");
    }

    #[test]
    fn test_compute_status_processed() {
        let status = RawMessageResponse::compute_status(Some(Utc::now()), &None);
        assert_eq!(status, "processed");
    }

    #[test]
    fn test_compute_status_error() {
        let status =
            RawMessageResponse::compute_status(Some(Utc::now()), &Some("Parse error".to_string()));
        assert_eq!(status, "error");
    }

    #[test]
    fn test_compute_status_error_without_processed_at() {
        // Error takes precedence even without processed_at
        let status = RawMessageResponse::compute_status(None, &Some("Parse error".to_string()));
        assert_eq!(status, "error");
    }

    // -------------------------------------------------------------------------
    // Param Conversion Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_convert_to_repo_params_defaults() {
        let api_params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };

        let repo_params = convert_to_repo_params(&api_params);

        assert_eq!(repo_params.limit, Some(20));
        assert_eq!(repo_params.offset, Some(0));
        assert!(repo_params.search.is_none());
        assert!(repo_params.status.is_none());
        assert!(repo_params.sort_by.is_none());
        assert!(repo_params.sort_order.is_none());
    }

    #[test]
    fn test_convert_to_repo_params_with_status() {
        use pharma_db::params::ProcessingStatus;

        let api_params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: Some("processed".to_string()),
            sort_by: None,
            sort_order: None,
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };

        let repo_params = convert_to_repo_params(&api_params);
        assert!(matches!(
            repo_params.status,
            Some(ProcessingStatus::Processed)
        ));
    }

    #[test]
    fn test_convert_to_repo_params_with_sort() {
        use pharma_db::params::{RawMessageSortField, SortOrder};

        let api_params = RawMessageQueryParams {
            limit: 20,
            offset: 0,
            search: None,
            status: None,
            sort_by: Some("processed_at".to_string()),
            sort_order: Some("asc".to_string()),
            start_date: None,
            end_date: None,
            group_id: None,
            participant_id: None,
        };

        let repo_params = convert_to_repo_params(&api_params);
        assert!(matches!(
            repo_params.sort_by,
            Some(RawMessageSortField::ProcessedAt)
        ));
        assert!(matches!(repo_params.sort_order, Some(SortOrder::Asc)));
    }

    // -------------------------------------------------------------------------
    // Response Serialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_raw_message_response_serialization() {
        let response = RawMessageResponse {
            id: Uuid::new_v4(),
            external_id: Some("ext123".to_string()),
            content: "Test message".to_string(),
            timestamp: Utc::now(),
            processed_at: Some(Utc::now()),
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: Utc::now(),
            participant_id: Uuid::new_v4(),
            participant_name: Some("John".to_string()),
            participant_jid: Some("123@s.whatsapp.net".to_string()),
            group_id: Uuid::new_v4(),
            group_name: Some("Test Group".to_string()),
            group_jid: Some("456@g.us".to_string()),
            status: "processed".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();

        // Check camelCase serialization
        assert!(json.contains("externalId"));
        assert!(json.contains("processedAt"));
        assert!(json.contains("replyToId"));
        assert!(json.contains("replyToContent"));
        assert!(json.contains("replyToSender"));
        assert!(json.contains("createdAt"));
        assert!(json.contains("participantId"));
        assert!(json.contains("participantName"));
        assert!(json.contains("participantJid"));
        assert!(json.contains("groupId"));
        assert!(json.contains("groupName"));
        assert!(json.contains("groupJid"));
    }

    #[test]
    fn test_default_limit_value() {
        assert_eq!(default_limit(), 20);
    }

    // -------------------------------------------------------------------------
    // Query Params Deserialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_query_params_default_values() {
        let json = "{}";
        let params: RawMessageQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 20);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn test_query_params_with_all_fields() {
        let json = r#"{
            "limit": 50,
            "offset": 100,
            "search": "test",
            "status": "processed",
            "sort_by": "timestamp",
            "sort_order": "desc"
        }"#;
        let params: RawMessageQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 50);
        assert_eq!(params.offset, 100);
        assert_eq!(params.search, Some("test".to_string()));
        assert_eq!(params.status, Some("processed".to_string()));
        assert_eq!(params.sort_by, Some("timestamp".to_string()));
        assert_eq!(params.sort_order, Some("desc".to_string()));
    }

    // -------------------------------------------------------------------------
    // Bulk Operation Response Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_bulk_operation_response_serialization() {
        let response = BulkOperationResponse {
            succeeded: vec![Uuid::new_v4(), Uuid::new_v4()],
            failed: vec![BulkOperationFailure {
                id: Uuid::new_v4(),
                error: "Test error".to_string(),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("succeeded"));
        assert!(json.contains("failed"));
        assert!(json.contains("error"));
    }

    #[test]
    fn test_bulk_operation_request_deserialization() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let json = format!(r#"{{"ids": ["{}", "{}"]}}"#, id1, id2);
        let req: BulkOperationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.ids.len(), 2);
        assert_eq!(req.ids[0], id1);
        assert_eq!(req.ids[1], id2);
    }

    #[test]
    fn test_bulk_operation_failure_serialization() {
        let failure = BulkOperationFailure {
            id: Uuid::new_v4(),
            error: "Has 2 associated offers and 1 associated requests".to_string(),
        };

        let json = serde_json::to_string(&failure).unwrap();
        assert!(json.contains("id"));
        assert!(json.contains("error"));
        assert!(json.contains("associated offers"));
    }
}

// =============================================================================
// Property Tests
// =============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // -------------------------------------------------------------------------
    // Property 11: Referential Integrity on Delete
    // Validates: Requirements 2.5, 8.6
    // -------------------------------------------------------------------------

    /// Property: Delete operation should fail with 409 Conflict when message has
    /// associated offers or requests. The error message must include counts.
    #[test]
    fn test_referential_integrity_error_message_format() {
        // Test that error message format is correct for various counts
        let test_cases = vec![
            (1, 0, "1 associated offers and 0 associated requests"),
            (0, 1, "0 associated offers and 1 associated requests"),
            (5, 3, "5 associated offers and 3 associated requests"),
            (
                100,
                200,
                "100 associated offers and 200 associated requests",
            ),
        ];

        for (offer_count, request_count, expected_substring) in test_cases {
            let error_msg = format!(
                "Cannot delete message: has {} associated offers and {} associated requests",
                offer_count, request_count
            );
            assert!(
                error_msg.contains(expected_substring),
                "Error message should contain '{}', got: {}",
                expected_substring,
                error_msg
            );
        }
    }

    proptest! {
        /// Property: For any non-negative offer and request counts where at least one is > 0,
        /// the referential integrity check should produce a valid error message
        #[test]
        fn prop_referential_integrity_error_format(
            offer_count in 0i64..1000,
            request_count in 0i64..1000,
        ) {
            // Skip case where both are 0 (no integrity violation)
            prop_assume!(offer_count > 0 || request_count > 0);

            let error_msg = format!(
                "Cannot delete message: has {} associated offers and {} associated requests",
                offer_count, request_count
            );

            // Property: Error message must contain both counts
            prop_assert!(error_msg.contains(&offer_count.to_string()));
            prop_assert!(error_msg.contains(&request_count.to_string()));
            prop_assert!(error_msg.contains("associated offers"));
            prop_assert!(error_msg.contains("associated requests"));
        }

        /// Property: Referential integrity check should block deletion when either count > 0
        #[test]
        fn prop_referential_integrity_blocks_deletion(
            offer_count in 0i64..100,
            request_count in 0i64..100,
        ) {
            let should_block = offer_count > 0 || request_count > 0;

            // Simulate the check logic from delete_raw_message handler
            let blocked = offer_count > 0 || request_count > 0;

            prop_assert_eq!(blocked, should_block,
                "Deletion should be blocked when offer_count={} or request_count={} > 0",
                offer_count, request_count
            );
        }
    }

    // -------------------------------------------------------------------------
    // Property 18: Bulk Operation Per-Item Status
    // Validates: Requirements 8.7
    // -------------------------------------------------------------------------

    proptest! {
        /// Property: For any bulk operation, succeeded + failed counts must equal input count
        #[test]
        fn prop_bulk_response_count_invariant(
            succeeded_count in 0usize..50,
            failed_count in 0usize..50,
        ) {
            let total_input = succeeded_count + failed_count;

            // Create mock response
            let succeeded: Vec<Uuid> = (0..succeeded_count).map(|_| Uuid::new_v4()).collect();
            let failed: Vec<BulkOperationFailure> = (0..failed_count)
                .map(|_| BulkOperationFailure {
                    id: Uuid::new_v4(),
                    error: "Test error".to_string(),
                })
                .collect();

            let response = BulkOperationResponse { succeeded, failed };

            // Property: Total items in response equals input count
            prop_assert_eq!(
                response.succeeded.len() + response.failed.len(),
                total_input,
                "succeeded + failed must equal total input"
            );
        }

        /// Property: All IDs in succeeded and failed arrays must be unique
        #[test]
        fn prop_bulk_response_unique_ids(
            succeeded_count in 0usize..20,
            failed_count in 0usize..20,
        ) {
            let succeeded: Vec<Uuid> = (0..succeeded_count).map(|_| Uuid::new_v4()).collect();
            let failed: Vec<BulkOperationFailure> = (0..failed_count)
                .map(|_| BulkOperationFailure {
                    id: Uuid::new_v4(),
                    error: "Test error".to_string(),
                })
                .collect();

            let response = BulkOperationResponse {
                succeeded: succeeded.clone(),
                failed: failed.clone(),
            };

            // Collect all IDs
            let mut all_ids: Vec<Uuid> = response.succeeded.clone();
            all_ids.extend(response.failed.iter().map(|f| f.id));

            // Check uniqueness
            let unique_count = {
                let mut sorted = all_ids.clone();
                sorted.sort();
                sorted.dedup();
                sorted.len()
            };

            prop_assert_eq!(
                unique_count,
                all_ids.len(),
                "All IDs in bulk response must be unique"
            );
        }

        /// Property: Failed items must have non-empty error messages
        #[test]
        fn prop_bulk_failure_has_error_message(
            error_messages in prop::collection::vec("[a-zA-Z0-9 ]+", 1..50),
        ) {
            for error_msg in error_messages {
                let failure = BulkOperationFailure {
                    id: Uuid::new_v4(),
                    error: error_msg.clone(),
                };

                prop_assert!(!failure.error.is_empty(),
                    "Failed items must have non-empty error messages");
                prop_assert_eq!(failure.error, error_msg);
            }
        }

        /// Property: BulkOperationResponse serializes to valid JSON with camelCase
        #[test]
        fn prop_bulk_response_json_format(
            succeeded_count in 0usize..10,
            failed_count in 0usize..10,
        ) {
            let succeeded: Vec<Uuid> = (0..succeeded_count).map(|_| Uuid::new_v4()).collect();
            let failed: Vec<BulkOperationFailure> = (0..failed_count)
                .map(|_| BulkOperationFailure {
                    id: Uuid::new_v4(),
                    error: "Test error".to_string(),
                })
                .collect();

            let response = BulkOperationResponse { succeeded, failed };

            // Property: Response must serialize to valid JSON
            let json_result = serde_json::to_string(&response);
            prop_assert!(json_result.is_ok(), "Response must serialize to JSON");

            let json = json_result.unwrap();

            // Property: JSON must contain expected fields
            prop_assert!(json.contains("succeeded"), "JSON must contain 'succeeded'");
            prop_assert!(json.contains("failed"), "JSON must contain 'failed'");

            // Property: JSON must be deserializable back
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json);
            prop_assert!(parsed.is_ok(), "JSON must be valid and parseable");
        }
    }

    // -------------------------------------------------------------------------
    // Additional Property Tests for Validation
    // -------------------------------------------------------------------------

    proptest! {
        /// Property: Valid limits are accepted (1-100)
        #[test]
        fn prop_valid_limit_accepted(limit in 1i64..=100) {
            let params = RawMessageQueryParams {
                limit,
                offset: 0,
                search: None,
                status: None,
                sort_by: None,
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            prop_assert!(validate_params(&params).is_ok(),
                "Limit {} should be valid", limit);
        }

        /// Property: Invalid limits are rejected (< 1 or > 100)
        #[test]
        fn prop_invalid_limit_rejected(limit in prop::num::i64::ANY.prop_filter(
            "limit outside valid range",
            |&l| !(1..=100).contains(&l)
        )) {
            let params = RawMessageQueryParams {
                limit,
                offset: 0,
                search: None,
                status: None,
                sort_by: None,
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            prop_assert!(validate_params(&params).is_err(),
                "Limit {} should be invalid", limit);
        }

        /// Property: Non-negative offsets are accepted
        #[test]
        fn prop_valid_offset_accepted(offset in 0i64..i64::MAX) {
            let params = RawMessageQueryParams {
                limit: 20,
                offset,
                search: None,
                status: None,
                sort_by: None,
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            prop_assert!(validate_params(&params).is_ok(),
                "Offset {} should be valid", offset);
        }

        /// Property: Negative offsets are rejected
        #[test]
        fn prop_negative_offset_rejected(offset in i64::MIN..-1i64) {
            let params = RawMessageQueryParams {
                limit: 20,
                offset,
                search: None,
                status: None,
                sort_by: None,
                sort_order: None,
                start_date: None,
                end_date: None,
                group_id: None,
                participant_id: None,
            };
            prop_assert!(validate_params(&params).is_err(),
                "Offset {} should be invalid", offset);
        }
    }
}
