//! Messaging API handlers
//!
//! Provides endpoints for sending WhatsApp messages via the Go bridge.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::jid::{JidError, validate_jid};
use crate::domain::{AuditAction, AuditLog, EntityType};
use crate::grpc::BridgeClientError;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

use super::routes::AppState;

/// Maximum message content length (WhatsApp limit)
const MAX_CONTENT_LENGTH: usize = 4096;

/// Request body for sending a message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    /// WhatsApp JID of the recipient
    pub recipient_jid: String,
    /// Message content to send
    pub content: String,
    /// Optional reference ID for tracking
    pub reference_id: Option<String>,
}

/// Response for send message operation
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    /// Whether the message was sent successfully
    pub success: bool,
    /// Message ID on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Error message on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Error code for programmatic handling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl SendMessageResponse {
    fn success(message_id: String) -> Self {
        Self {
            success: true,
            message_id: Some(message_id),
            error: None,
            code: None,
        }
    }

    fn error(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            message_id: None,
            error: Some(message.into()),
            code: Some(code.into()),
        }
    }
}

/// Validation error types
#[derive(Debug)]
enum ValidationError {
    MissingRecipientJid,
    MissingContent,
    EmptyContent,
    ContentTooLong(usize),
    InvalidJid(JidError),
}

impl ValidationError {
    fn to_response(&self) -> (StatusCode, SendMessageResponse) {
        match self {
            ValidationError::MissingRecipientJid => (
                StatusCode::BAD_REQUEST,
                SendMessageResponse::error("recipient_jid is required", "MISSING_RECIPIENT_JID"),
            ),
            ValidationError::MissingContent => (
                StatusCode::BAD_REQUEST,
                SendMessageResponse::error("content is required", "MISSING_CONTENT"),
            ),
            ValidationError::EmptyContent => (
                StatusCode::BAD_REQUEST,
                SendMessageResponse::error(
                    "content cannot be empty or whitespace-only",
                    "EMPTY_CONTENT",
                ),
            ),
            ValidationError::ContentTooLong(len) => (
                StatusCode::BAD_REQUEST,
                SendMessageResponse::error(
                    format!(
                        "content exceeds maximum length of {} characters (got {})",
                        MAX_CONTENT_LENGTH, len
                    ),
                    "CONTENT_TOO_LONG",
                ),
            ),
            ValidationError::InvalidJid(e) => (
                StatusCode::BAD_REQUEST,
                SendMessageResponse::error(format!("Invalid JID: {}", e), "INVALID_JID"),
            ),
        }
    }
}

/// Validate the send message request
fn validate_request(req: &SendMessageRequest) -> Result<(), ValidationError> {
    // Check recipient_jid
    if req.recipient_jid.is_empty() {
        return Err(ValidationError::MissingRecipientJid);
    }

    // Validate JID format
    validate_jid(&req.recipient_jid).map_err(ValidationError::InvalidJid)?;

    // Check content
    if req.content.is_empty() {
        return Err(ValidationError::MissingContent);
    }

    // Check for whitespace-only content
    if req.content.trim().is_empty() {
        return Err(ValidationError::EmptyContent);
    }

    // Check content length
    if req.content.len() > MAX_CONTENT_LENGTH {
        return Err(ValidationError::ContentTooLong(req.content.len()));
    }

    Ok(())
}

/// Send a WhatsApp message via the Go bridge
///
/// POST /api/messages/send
pub async fn send_message<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<SendMessageRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Validate request
    if let Err(e) = validate_request(&req) {
        let (status, response) = e.to_response();
        return (status, Json(response));
    }

    // Check if bridge client is available
    let bridge = match &state.bridge_client {
        Some(client) => client.clone(),
        None => {
            tracing::error!("Bridge client not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(SendMessageResponse::error(
                    "Messaging service not available",
                    "BRIDGE_NOT_CONFIGURED",
                )),
            );
        }
    };

    // Generate reference ID if not provided
    let reference_id = req
        .reference_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Send message via bridge
    match bridge
        .send_message(
            req.recipient_jid.clone(),
            req.content.clone(),
            Some(reference_id.clone()),
        )
        .await
    {
        Ok(message_id) => {
            tracing::info!(
                recipient = %req.recipient_jid,
                message_id = %message_id,
                "✅ Message sent successfully"
            );

            // Create audit log entry for successful send
            let content_preview = truncate_content(&req.content, 100);
            let audit_log = AuditLog::system(
                AuditAction::MessageSent,
                EntityType::Message,
                message_id.clone(),
            )
            .with_details(serde_json::json!({
                "recipient_jid": req.recipient_jid,
                "content_preview": content_preview,
                "reference_id": reference_id,
            }));

            if let Err(e) = state.audit_log_repo.save(&audit_log).await {
                tracing::warn!(error = %e, "Failed to save message sent audit log");
            }

            (
                StatusCode::OK,
                Json(SendMessageResponse::success(message_id)),
            )
        }
        Err(e) => {
            tracing::error!(
                recipient = %req.recipient_jid,
                error = %e,
                "❌ Failed to send message"
            );

            let (status, code) = match &e {
                BridgeClientError::NotConnected => {
                    (StatusCode::SERVICE_UNAVAILABLE, "BRIDGE_NOT_CONNECTED")
                }
                BridgeClientError::ConnectionError(_) => {
                    (StatusCode::SERVICE_UNAVAILABLE, "BRIDGE_CONNECTION_ERROR")
                }
                BridgeClientError::GrpcError(_) => (StatusCode::BAD_GATEWAY, "GRPC_ERROR"),
                BridgeClientError::BridgeError(_) => (StatusCode::BAD_GATEWAY, "BRIDGE_ERROR"),
            };

            // Create audit log entry for failed send
            let content_preview = truncate_content(&req.content, 100);
            let audit_log = AuditLog::system(
                AuditAction::MessageFailed,
                EntityType::Message,
                reference_id.clone(),
            )
            .with_details(serde_json::json!({
                "recipient_jid": req.recipient_jid,
                "content_preview": content_preview,
                "reference_id": reference_id,
                "error": e.to_string(),
                "error_code": code,
            }));

            if let Err(audit_err) = state.audit_log_repo.save(&audit_log).await {
                tracing::warn!(error = %audit_err, "Failed to save message failed audit log");
            }

            (
                status,
                Json(SendMessageResponse::error(e.to_string(), code)),
            )
        }
    }
}

/// Truncate content to a maximum length for audit logging
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Feature: send-message, Property 1: Valid Request Returns Tracking ID
    // Feature: send-message, Property 2: Missing Fields Rejected
    // Feature: send-message, Property 4: Whitespace Content Rejected
    // Validates: Requirements 1.1, 1.2, 1.5

    #[test]
    fn test_validate_valid_request() {
        let req = SendMessageRequest {
            recipient_jid: "201234567890@s.whatsapp.net".to_string(),
            content: "Hello, world!".to_string(),
            reference_id: None,
        };
        assert!(validate_request(&req).is_ok());
    }

    #[test]
    fn test_validate_missing_recipient_jid() {
        let req = SendMessageRequest {
            recipient_jid: "".to_string(),
            content: "Hello".to_string(),
            reference_id: None,
        };
        assert!(matches!(
            validate_request(&req),
            Err(ValidationError::MissingRecipientJid)
        ));
    }

    #[test]
    fn test_validate_missing_content() {
        let req = SendMessageRequest {
            recipient_jid: "201234567890@s.whatsapp.net".to_string(),
            content: "".to_string(),
            reference_id: None,
        };
        assert!(matches!(
            validate_request(&req),
            Err(ValidationError::MissingContent)
        ));
    }

    #[test]
    fn test_validate_whitespace_content() {
        let req = SendMessageRequest {
            recipient_jid: "201234567890@s.whatsapp.net".to_string(),
            content: "   \t\n  ".to_string(),
            reference_id: None,
        };
        assert!(matches!(
            validate_request(&req),
            Err(ValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_validate_content_too_long() {
        let req = SendMessageRequest {
            recipient_jid: "201234567890@s.whatsapp.net".to_string(),
            content: "a".repeat(MAX_CONTENT_LENGTH + 1),
            reference_id: None,
        };
        assert!(matches!(
            validate_request(&req),
            Err(ValidationError::ContentTooLong(_))
        ));
    }

    #[test]
    fn test_validate_content_at_limit() {
        let req = SendMessageRequest {
            recipient_jid: "201234567890@s.whatsapp.net".to_string(),
            content: "a".repeat(MAX_CONTENT_LENGTH),
            reference_id: None,
        };
        assert!(validate_request(&req).is_ok());
    }

    #[test]
    fn test_validate_invalid_jid() {
        let req = SendMessageRequest {
            recipient_jid: "invalid-jid".to_string(),
            content: "Hello".to_string(),
            reference_id: None,
        };
        assert!(matches!(
            validate_request(&req),
            Err(ValidationError::InvalidJid(_))
        ));
    }

    #[test]
    fn test_truncate_content_short() {
        let content = "Hello, world!";
        assert_eq!(truncate_content(content, 100), "Hello, world!");
    }

    #[test]
    fn test_truncate_content_exact() {
        let content = "a".repeat(100);
        assert_eq!(truncate_content(&content, 100), content);
    }

    #[test]
    fn test_truncate_content_long() {
        let content = "a".repeat(150);
        let truncated = truncate_content(&content, 100);
        assert_eq!(truncated.len(), 103); // 100 chars + "..."
        assert!(truncated.ends_with("..."));
    }

    // Feature: send-message, Property 9: Audit Log Completeness
    // Validates: Requirements 5.1, 5.2, 5.3
    // For any message send operation, the audit log entry should contain:
    // - recipient_jid
    // - content_preview (first 100 chars)
    // - reference_id
    // - For failures: error and error_code

    #[test]
    fn test_audit_log_success_has_required_fields() {
        use crate::domain::{AuditAction, AuditLog, EntityType};

        let recipient_jid = "201234567890@s.whatsapp.net";
        let content = "Hello, this is a test message";
        let reference_id = "ref-123";
        let message_id = "msg-456";

        let content_preview = truncate_content(content, 100);
        let audit_log = AuditLog::system(AuditAction::MessageSent, EntityType::Message, message_id)
            .with_details(serde_json::json!({
                "recipient_jid": recipient_jid,
                "content_preview": content_preview,
                "reference_id": reference_id,
            }));

        // Verify audit log has correct action and entity type
        assert_eq!(audit_log.action, "message_sent");
        assert_eq!(audit_log.entity_type, "message");
        assert_eq!(audit_log.entity_id, message_id);
        assert_eq!(audit_log.actor, "system");

        // Verify details contain required fields
        let details = audit_log.details.expect("details should be present");
        assert_eq!(details["recipient_jid"], recipient_jid);
        assert_eq!(details["content_preview"], content_preview);
        assert_eq!(details["reference_id"], reference_id);
    }

    #[test]
    fn test_audit_log_failure_has_required_fields() {
        use crate::domain::{AuditAction, AuditLog, EntityType};

        let recipient_jid = "201234567890@s.whatsapp.net";
        let content = "Hello, this is a test message";
        let reference_id = "ref-123";
        let error = "Bridge not connected";
        let error_code = "BRIDGE_NOT_CONNECTED";

        let content_preview = truncate_content(content, 100);
        let audit_log = AuditLog::system(
            AuditAction::MessageFailed,
            EntityType::Message,
            reference_id,
        )
        .with_details(serde_json::json!({
            "recipient_jid": recipient_jid,
            "content_preview": content_preview,
            "reference_id": reference_id,
            "error": error,
            "error_code": error_code,
        }));

        // Verify audit log has correct action and entity type
        assert_eq!(audit_log.action, "message_failed");
        assert_eq!(audit_log.entity_type, "message");
        assert_eq!(audit_log.entity_id, reference_id);
        assert_eq!(audit_log.actor, "system");

        // Verify details contain required fields including error info
        let details = audit_log.details.expect("details should be present");
        assert_eq!(details["recipient_jid"], recipient_jid);
        assert_eq!(details["content_preview"], content_preview);
        assert_eq!(details["reference_id"], reference_id);
        assert_eq!(details["error"], error);
        assert_eq!(details["error_code"], error_code);
    }

    #[test]
    fn test_audit_log_content_preview_truncation() {
        use crate::domain::{AuditAction, AuditLog, EntityType};

        // Create content longer than 100 chars
        let long_content = "a".repeat(200);
        let content_preview = truncate_content(&long_content, 100);

        let audit_log = AuditLog::system(AuditAction::MessageSent, EntityType::Message, "msg-123")
            .with_details(serde_json::json!({
                "recipient_jid": "201234567890@s.whatsapp.net",
                "content_preview": content_preview,
                "reference_id": "ref-123",
            }));

        let details = audit_log.details.expect("details should be present");
        let preview = details["content_preview"].as_str().unwrap();

        // Verify content is truncated to 100 chars + "..."
        assert_eq!(preview.len(), 103);
        assert!(preview.ends_with("..."));
    }
}
