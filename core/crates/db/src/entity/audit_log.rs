//! AuditLog entity - Audit trail for compliance

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub actor: String,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    #[sea_orm(primary_key)]
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Re-export AuditAction and EntityType from domain for compatibility
pub use crate::audit_types::{AuditAction, EntityType};

impl Model {
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
            created_at: chrono::Utc::now(),
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
