//! Raw Messages API Handler
//!
//! Provides endpoints for listing and viewing raw WhatsApp messages.
//! Feature: raw-messages-display

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
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
}
