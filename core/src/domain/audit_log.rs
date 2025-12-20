//! Audit Log Entity
//!
//! Stores audit trail for compliance and debugging.
//! All significant actions are logged for traceability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================================
// Audit Action Types
// ============================================================================

/// Types of auditable actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // Match actions
    MatchCreated,
    MatchConfirmed,
    MatchRejected,
    MatchAutoConfirmed,

    // Offer/Request actions
    OfferCreated,
    RequestCreated,
    OfferExpired,
    RequestExpired,

    // Weight/Config actions
    WeightsUpdated,
    WeightsRollback,
    ABTestCreated,
    ABTestEnded,

    // Review Queue actions
    ReviewQueued,
    ReviewApproved,
    ReviewRejected,
    ReviewSkipped,

    // System actions
    SystemStartup,
    ConfigChanged,

    // Generic
    Custom(String),
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MatchCreated => write!(f, "match_created"),
            Self::MatchConfirmed => write!(f, "match_confirmed"),
            Self::MatchRejected => write!(f, "match_rejected"),
            Self::MatchAutoConfirmed => write!(f, "match_auto_confirmed"),
            Self::OfferCreated => write!(f, "offer_created"),
            Self::RequestCreated => write!(f, "request_created"),
            Self::OfferExpired => write!(f, "offer_expired"),
            Self::RequestExpired => write!(f, "request_expired"),
            Self::WeightsUpdated => write!(f, "weights_updated"),
            Self::WeightsRollback => write!(f, "weights_rollback"),
            Self::ABTestCreated => write!(f, "ab_test_created"),
            Self::ABTestEnded => write!(f, "ab_test_ended"),
            Self::ReviewQueued => write!(f, "review_queued"),
            Self::ReviewApproved => write!(f, "review_approved"),
            Self::ReviewRejected => write!(f, "review_rejected"),
            Self::ReviewSkipped => write!(f, "review_skipped"),
            Self::SystemStartup => write!(f, "system_startup"),
            Self::ConfigChanged => write!(f, "config_changed"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

// ============================================================================
// Entity Types
// ============================================================================

/// Types of entities that can be audited
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Match,
    Offer,
    Request,
    Weights,
    ABTest,
    ReviewQueue,
    Group,
    System,
    Custom(String),
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Match => write!(f, "match"),
            Self::Offer => write!(f, "offer"),
            Self::Request => write!(f, "request"),
            Self::Weights => write!(f, "weights"),
            Self::ABTest => write!(f, "ab_test"),
            Self::ReviewQueue => write!(f, "review_queue"),
            Self::Group => write!(f, "group"),
            Self::System => write!(f, "system"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

// ============================================================================
// Audit Log Entry
// ============================================================================

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    /// Unique identifier
    pub id: Uuid,
    /// The action that was performed
    #[sqlx(rename = "action")]
    pub action: String,
    /// Type of entity affected
    pub entity_type: String,
    /// ID of the entity affected
    pub entity_id: String,
    /// Who performed the action (user ID, "system", etc.)
    pub actor: String,
    /// Additional context as JSON
    pub details: Option<serde_json::Value>,
    /// IP address of the actor (if applicable)
    pub ip_address: Option<String>,
    /// User agent (if applicable)
    pub user_agent: Option<String>,
    /// When the action occurred
    pub created_at: DateTime<Utc>,
}

impl AuditLog {
    /// Create a new audit log entry
    pub fn new(
        action: AuditAction,
        entity_type: EntityType,
        entity_id: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            action: action.to_string(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.into(),
            actor: actor.into(),
            details: None,
            ip_address: None,
            user_agent: None,
            created_at: Utc::now(),
        }
    }

    /// Add details to the log entry
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add IP address
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Add user agent
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Create a system audit entry
    pub fn system(
        action: AuditAction,
        entity_type: EntityType,
        entity_id: impl Into<String>,
    ) -> Self {
        Self::new(action, entity_type, entity_id, "system")
    }

    /// Create a match confirmed audit entry
    pub fn match_confirmed(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        score: f64,
    ) -> Self {
        Self::new(
            AuditAction::MatchConfirmed,
            EntityType::Match,
            match_id,
            user_id,
        )
        .with_details(serde_json::json!({ "score": score }))
    }

    /// Create a match rejected audit entry
    pub fn match_rejected(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        reason: Option<&str>,
    ) -> Self {
        let mut entry = Self::new(
            AuditAction::MatchRejected,
            EntityType::Match,
            match_id,
            user_id,
        );
        if let Some(r) = reason {
            entry = entry.with_details(serde_json::json!({ "reason": r }));
        }
        entry
    }

    /// Create a weights updated audit entry
    pub fn weights_updated(
        actor: impl Into<String>,
        source: &str,
        previous: Option<serde_json::Value>,
    ) -> Self {
        let mut details = serde_json::json!({ "source": source });
        if let Some(prev) = previous {
            details["previous"] = prev;
        }
        Self::new(
            AuditAction::WeightsUpdated,
            EntityType::Weights,
            "current",
            actor,
        )
        .with_details(details)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::MatchConfirmed.to_string(), "match_confirmed");
        assert_eq!(AuditAction::WeightsUpdated.to_string(), "weights_updated");
        assert_eq!(AuditAction::Custom("test".to_string()).to_string(), "test");
    }

    #[test]
    fn test_entity_type_display() {
        assert_eq!(EntityType::Match.to_string(), "match");
        assert_eq!(EntityType::Weights.to_string(), "weights");
    }

    #[test]
    fn test_audit_log_new() {
        let log = AuditLog::new(
            AuditAction::MatchConfirmed,
            EntityType::Match,
            "match-123",
            "user-456",
        );

        assert_eq!(log.action, "match_confirmed");
        assert_eq!(log.entity_type, "match");
        assert_eq!(log.entity_id, "match-123");
        assert_eq!(log.actor, "user-456");
        assert!(log.details.is_none());
    }

    #[test]
    fn test_audit_log_with_details() {
        let log = AuditLog::new(
            AuditAction::WeightsUpdated,
            EntityType::Weights,
            "current",
            "admin",
        )
        .with_details(serde_json::json!({ "source": "manual" }));

        assert!(log.details.is_some());
        assert_eq!(log.details.unwrap()["source"], "manual");
    }

    #[test]
    fn test_audit_log_match_confirmed() {
        let log = AuditLog::match_confirmed("match-abc", "operator@test.com", 0.87);

        assert_eq!(log.action, "match_confirmed");
        assert_eq!(log.entity_id, "match-abc");
        assert_eq!(log.actor, "operator@test.com");
        assert!(log.details.is_some());
    }

    #[test]
    fn test_audit_log_system() {
        let log = AuditLog::system(AuditAction::SystemStartup, EntityType::System, "core");

        assert_eq!(log.actor, "system");
        assert_eq!(log.entity_type, "system");
    }
}
