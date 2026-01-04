//! Supervision Audit Trail Module
//!
//! Extended audit logging for AI supervised auto-approval decisions.
//! Provides comprehensive tracking of auto-approvals, overrides, and configuration changes.
//!
//! Requirements: 2.1, 2.2, 2.3, 5.4

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;
use uuid::Uuid;

use super::auto_approve::{AutoApproveAction, AutoApproveConfig, SafetyCheckResult};

// =============================================================================
// Supervision Event Types
// =============================================================================

/// Type of supervision audit event
/// Requirements: 2.1, 2.2
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupervisionEventType {
    /// Match was automatically approved by AI
    AutoApproved,
    /// Match was queued for human review
    QueuedForReview,
    /// Match was blocked by safety guardrails
    Blocked,
    /// AI decision was overridden by human
    Overridden,
    /// AI approval was undone
    UndoApproval,
    /// Configuration was changed
    ConfigChanged,
    /// System was paused
    SystemPaused,
    /// System was resumed
    SystemResumed,
}

impl std::fmt::Display for SupervisionEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AutoApproved => write!(f, "AUTO_APPROVED"),
            Self::QueuedForReview => write!(f, "QUEUED_FOR_REVIEW"),
            Self::Blocked => write!(f, "BLOCKED"),
            Self::Overridden => write!(f, "OVERRIDDEN"),
            Self::UndoApproval => write!(f, "UNDO_APPROVAL"),
            Self::ConfigChanged => write!(f, "CONFIG_CHANGED"),
            Self::SystemPaused => write!(f, "SYSTEM_PAUSED"),
            Self::SystemResumed => write!(f, "SYSTEM_RESUMED"),
        }
    }
}

// =============================================================================
// Supervision Audit Entry
// =============================================================================

/// Extended audit entry for AI auto-approval supervision
/// Requirements: 2.1, 2.2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionAuditEntry {
    /// Unique entry ID
    pub id: Uuid,
    /// Match ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_id: Option<Uuid>,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Type of supervision event
    pub event_type: SupervisionEventType,
    /// AI confidence score (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_confidence: Option<f64>,
    /// AI explanation/reasoning (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_explanation: Option<String>,
    /// Decision/action taken
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<AutoApproveAction>,
    /// Results of safety checks
    #[serde(default)]
    pub safety_checks: Vec<SafetyCheckResult>,
    /// Whether this decision was overridden
    pub overridden: bool,
    /// User who performed the override (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_by: Option<Uuid>,
    /// Reason for the override (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_reason: Option<String>,
    /// When the override occurred (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_at: Option<DateTime<Utc>>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl SupervisionAuditEntry {
    /// Create a new supervision audit entry
    pub fn new(event_type: SupervisionEventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: None,
            timestamp: Utc::now(),
            event_type,
            ai_confidence: None,
            ai_explanation: None,
            decision: None,
            safety_checks: Vec::new(),
            overridden: false,
            override_by: None,
            override_reason: None,
            override_at: None,
            metadata: None,
        }
    }

    /// Create an entry for an auto-approval event
    /// Requirements: 2.1
    pub fn auto_approved(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: Some(match_id),
            timestamp: Utc::now(),
            event_type: SupervisionEventType::AutoApproved,
            ai_confidence: Some(ai_confidence),
            ai_explanation: Some(ai_explanation),
            decision: Some(AutoApproveAction::Approved),
            safety_checks,
            overridden: false,
            override_by: None,
            override_reason: None,
            override_at: None,
            metadata: None,
        }
    }

    /// Create an entry for a queued-for-review event
    pub fn queued_for_review(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: Some(match_id),
            timestamp: Utc::now(),
            event_type: SupervisionEventType::QueuedForReview,
            ai_confidence: Some(ai_confidence),
            ai_explanation: Some(ai_explanation),
            decision: Some(AutoApproveAction::QueuedForReview { reason }),
            safety_checks,
            overridden: false,
            override_by: None,
            override_reason: None,
            override_at: None,
            metadata: None,
        }
    }

    /// Create an entry for a blocked event
    pub fn blocked(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: Some(match_id),
            timestamp: Utc::now(),
            event_type: SupervisionEventType::Blocked,
            ai_confidence: Some(ai_confidence),
            ai_explanation: Some(ai_explanation),
            decision: Some(AutoApproveAction::Blocked { reason }),
            safety_checks,
            overridden: false,
            override_by: None,
            override_reason: None,
            override_at: None,
            metadata: None,
        }
    }

    /// Create an entry for an override event
    /// Requirements: 2.2
    pub fn overridden(
        match_id: Uuid,
        user_id: Uuid,
        reason: String,
        original_confidence: f64,
        original_explanation: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: Some(match_id),
            timestamp: Utc::now(),
            event_type: SupervisionEventType::Overridden,
            ai_confidence: Some(original_confidence),
            ai_explanation: Some(original_explanation),
            decision: Some(AutoApproveAction::Approved), // Original decision was approval
            safety_checks: Vec::new(),
            overridden: true,
            override_by: Some(user_id),
            override_reason: Some(reason),
            override_at: Some(Utc::now()),
            metadata: None,
        }
    }

    /// Create an entry for an undo event
    pub fn undo_approval(match_id: Uuid, user_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: Some(match_id),
            timestamp: Utc::now(),
            event_type: SupervisionEventType::UndoApproval,
            ai_confidence: None,
            ai_explanation: None,
            decision: None,
            safety_checks: Vec::new(),
            overridden: true,
            override_by: Some(user_id),
            override_reason: Some("Approval undone".to_string()),
            override_at: Some(Utc::now()),
            metadata: None,
        }
    }

    /// Create an entry for a configuration change event
    /// Requirements: 5.4
    pub fn config_changed(
        user_id: Uuid,
        old_config: &AutoApproveConfig,
        new_config: &AutoApproveConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: None,
            timestamp: Utc::now(),
            event_type: SupervisionEventType::ConfigChanged,
            ai_confidence: None,
            ai_explanation: None,
            decision: None,
            safety_checks: Vec::new(),
            overridden: false,
            override_by: Some(user_id),
            override_reason: None,
            override_at: None,
            metadata: Some(serde_json::json!({
                "old_config": old_config,
                "new_config": new_config,
            })),
        }
    }

    /// Create an entry for a system paused event
    pub fn system_paused(reason: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: None,
            timestamp: Utc::now(),
            event_type: SupervisionEventType::SystemPaused,
            ai_confidence: None,
            ai_explanation: None,
            decision: None,
            safety_checks: Vec::new(),
            overridden: false,
            override_by: None,
            override_reason: Some(reason),
            override_at: None,
            metadata: None,
        }
    }

    /// Create an entry for a system resumed event
    pub fn system_resumed() -> Self {
        Self {
            id: Uuid::new_v4(),
            match_id: None,
            timestamp: Utc::now(),
            event_type: SupervisionEventType::SystemResumed,
            ai_confidence: None,
            ai_explanation: None,
            decision: None,
            safety_checks: Vec::new(),
            overridden: false,
            override_by: None,
            override_reason: None,
            override_at: None,
            metadata: None,
        }
    }

    // Builder methods

    /// Set match ID
    pub fn with_match_id(mut self, match_id: Uuid) -> Self {
        self.match_id = Some(match_id);
        self
    }

    /// Set AI confidence
    pub fn with_ai_confidence(mut self, confidence: f64) -> Self {
        self.ai_confidence = Some(confidence);
        self
    }

    /// Set AI explanation
    pub fn with_ai_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.ai_explanation = Some(explanation.into());
        self
    }

    /// Set decision
    pub fn with_decision(mut self, decision: AutoApproveAction) -> Self {
        self.decision = Some(decision);
        self
    }

    /// Set safety checks
    pub fn with_safety_checks(mut self, checks: Vec<SafetyCheckResult>) -> Self {
        self.safety_checks = checks;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Check if this entry has complete auto-approval audit data
    /// Requirements: 2.1
    pub fn has_complete_auto_approval_data(&self) -> bool {
        self.match_id.is_some()
            && self.ai_confidence.is_some()
            && self.ai_explanation.is_some()
            && self.decision.is_some()
    }

    /// Check if this entry has complete override audit data
    /// Requirements: 2.2
    pub fn has_complete_override_data(&self) -> bool {
        self.overridden
            && self.override_by.is_some()
            && self.override_reason.is_some()
            && self.override_at.is_some()
    }

    /// Check if this entry has complete config change audit data
    /// Requirements: 5.4
    pub fn has_complete_config_change_data(&self) -> bool {
        if self.event_type != SupervisionEventType::ConfigChanged {
            return false;
        }
        if let Some(ref metadata) = self.metadata {
            metadata.get("old_config").is_some() && metadata.get("new_config").is_some()
        } else {
            false
        }
    }
}

// =============================================================================
// Supervision Audit Filter
// =============================================================================

/// Filter for querying supervision audit logs
/// Requirements: 2.3
#[derive(Debug, Clone, Default)]
pub struct SupervisionAuditFilter {
    /// Filter by match ID
    pub match_id: Option<Uuid>,
    /// Filter by event type
    pub event_type: Option<SupervisionEventType>,
    /// Filter by date range start (inclusive)
    pub start_time: Option<DateTime<Utc>>,
    /// Filter by date range end (inclusive)
    pub end_time: Option<DateTime<Utc>>,
    /// Filter by minimum confidence
    pub min_confidence: Option<f64>,
    /// Filter by maximum confidence
    pub max_confidence: Option<f64>,
    /// Filter by override status
    pub overridden: Option<bool>,
    /// Filter by user who performed override
    pub override_by: Option<Uuid>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

impl SupervisionAuditFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by match ID
    pub fn for_match(mut self, match_id: Uuid) -> Self {
        self.match_id = Some(match_id);
        self
    }

    /// Filter by event type
    pub fn of_type(mut self, event_type: SupervisionEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Filter by date range
    pub fn in_date_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// Filter by confidence range
    pub fn in_confidence_range(mut self, min: f64, max: f64) -> Self {
        self.min_confidence = Some(min);
        self.max_confidence = Some(max);
        self
    }

    /// Filter by override status
    pub fn with_override_status(mut self, overridden: bool) -> Self {
        self.overridden = Some(overridden);
        self
    }

    /// Filter by user who performed override
    pub fn overridden_by(mut self, user_id: Uuid) -> Self {
        self.override_by = Some(user_id);
        self
    }

    /// Limit results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset for pagination
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Check if an entry matches this filter
    pub fn matches(&self, entry: &SupervisionAuditEntry) -> bool {
        // Match ID filter
        if let Some(match_id) = self.match_id
            && entry.match_id != Some(match_id)
        {
            return false;
        }

        // Event type filter
        if let Some(event_type) = self.event_type
            && entry.event_type != event_type
        {
            return false;
        }

        // Date range filter
        if let Some(start) = self.start_time
            && entry.timestamp < start
        {
            return false;
        }
        if let Some(end) = self.end_time
            && entry.timestamp > end
        {
            return false;
        }

        // Confidence range filter
        if let Some(min) = self.min_confidence {
            if let Some(confidence) = entry.ai_confidence {
                if confidence < min {
                    return false;
                }
            } else {
                return false; // No confidence, doesn't match min filter
            }
        }
        if let Some(max) = self.max_confidence {
            if let Some(confidence) = entry.ai_confidence {
                if confidence > max {
                    return false;
                }
            } else {
                return false; // No confidence, doesn't match max filter
            }
        }

        // Override status filter
        if let Some(overridden) = self.overridden
            && entry.overridden != overridden
        {
            return false;
        }

        // Override by filter
        if let Some(override_by) = self.override_by
            && entry.override_by != Some(override_by)
        {
            return false;
        }

        true
    }
}

// =============================================================================
// Supervision Audit Repository
// =============================================================================

/// Error type for supervision audit operations
#[derive(Debug, thiserror::Error)]
pub enum SupervisionAuditError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Entry not found: {0}")]
    NotFound(Uuid),
}

/// Trait for supervision audit storage backends
#[async_trait::async_trait]
pub trait SupervisionAuditRepository: Send + Sync {
    /// Save an audit entry
    async fn save(&self, entry: &SupervisionAuditEntry) -> Result<(), SupervisionAuditError>;

    /// Query audit entries with filter
    async fn query(
        &self,
        filter: &SupervisionAuditFilter,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError>;

    /// Get a single entry by ID
    async fn get_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<SupervisionAuditEntry>, SupervisionAuditError>;

    /// Get entries for a specific match
    async fn get_for_match(
        &self,
        match_id: Uuid,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError>;

    /// Count entries matching filter
    async fn count(&self, filter: &SupervisionAuditFilter) -> Result<usize, SupervisionAuditError>;
}

// =============================================================================
// In-Memory Supervision Audit Repository
// =============================================================================

/// In-memory implementation of supervision audit repository
/// Useful for testing and development
pub struct MemorySupervisionAuditRepository {
    entries: RwLock<VecDeque<SupervisionAuditEntry>>,
    max_size: usize,
}

impl MemorySupervisionAuditRepository {
    /// Create a new in-memory repository
    pub fn new(max_size: usize) -> Self {
        let max_size = if max_size == 0 { 10000 } else { max_size };
        Self {
            entries: RwLock::new(VecDeque::with_capacity(max_size)),
            max_size,
        }
    }

    /// Get all entries (for testing)
    pub fn get_all(&self) -> Vec<SupervisionAuditEntry> {
        self.entries.read().unwrap().iter().cloned().collect()
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
    }
}

impl Default for MemorySupervisionAuditRepository {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[async_trait::async_trait]
impl SupervisionAuditRepository for MemorySupervisionAuditRepository {
    async fn save(&self, entry: &SupervisionAuditEntry) -> Result<(), SupervisionAuditError> {
        let mut entries = self.entries.write().unwrap();

        // Evict oldest if at capacity
        if entries.len() >= self.max_size {
            entries.pop_front();
        }

        entries.push_back(entry.clone());
        Ok(())
    }

    async fn query(
        &self,
        filter: &SupervisionAuditFilter,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError> {
        let entries = self.entries.read().unwrap();
        let limit = filter.limit.unwrap_or(100);
        let offset = filter.offset.unwrap_or(0);

        let results: Vec<_> = entries
            .iter()
            .rev() // Most recent first
            .filter(|e| filter.matches(e))
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();

        Ok(results)
    }

    async fn get_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<SupervisionAuditEntry>, SupervisionAuditError> {
        let entries = self.entries.read().unwrap();
        Ok(entries.iter().find(|e| e.id == id).cloned())
    }

    async fn get_for_match(
        &self,
        match_id: Uuid,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError> {
        let filter = SupervisionAuditFilter::new().for_match(match_id);
        self.query(&filter).await
    }

    async fn count(&self, filter: &SupervisionAuditFilter) -> Result<usize, SupervisionAuditError> {
        let entries = self.entries.read().unwrap();
        Ok(entries.iter().filter(|e| filter.matches(e)).count())
    }
}

// =============================================================================
// Supervision Audit Trail Manager
// =============================================================================

/// Configuration for supervision audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisionAuditConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Log to tracing
    pub log_to_tracing: bool,
    /// Retention period in days
    pub retention_days: u32,
}

impl Default for SupervisionAuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_to_tracing: true,
            retention_days: 90,
        }
    }
}

/// Manager for supervision audit trail
pub struct SupervisionAuditTrail<R: SupervisionAuditRepository = MemorySupervisionAuditRepository> {
    config: RwLock<SupervisionAuditConfig>,
    repository: R,
}

impl SupervisionAuditTrail<MemorySupervisionAuditRepository> {
    /// Create a new supervision audit trail with in-memory repository
    pub fn new(config: SupervisionAuditConfig) -> Self {
        Self {
            config: RwLock::new(config),
            repository: MemorySupervisionAuditRepository::new(10000),
        }
    }
}

impl<R: SupervisionAuditRepository> SupervisionAuditTrail<R> {
    /// Create a new supervision audit trail with custom repository
    pub fn with_repository(config: SupervisionAuditConfig, repository: R) -> Self {
        Self {
            config: RwLock::new(config),
            repository,
        }
    }

    /// Check if audit is enabled
    fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Check if tracing is enabled
    fn should_trace(&self) -> bool {
        self.config.read().unwrap().log_to_tracing
    }

    /// Log an auto-approval event
    /// Requirements: 2.1
    pub async fn log_auto_approval(
        &self,
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::auto_approved(
                match_id,
                ai_confidence,
                ai_explanation,
                safety_checks,
            ));
        }

        let entry = SupervisionAuditEntry::auto_approved(
            match_id,
            ai_confidence,
            ai_explanation.clone(),
            safety_checks,
        );

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                match_id = %match_id,
                ai_confidence = %ai_confidence,
                "📝 Supervision Audit: Auto-approval logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log an override event
    /// Requirements: 2.2
    pub async fn log_override(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        reason: String,
        original_confidence: f64,
        original_explanation: String,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::overridden(
                match_id,
                user_id,
                reason,
                original_confidence,
                original_explanation,
            ));
        }

        let entry = SupervisionAuditEntry::overridden(
            match_id,
            user_id,
            reason.clone(),
            original_confidence,
            original_explanation,
        );

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                match_id = %match_id,
                user_id = %user_id,
                reason = %reason,
                "📝 Supervision Audit: Override logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log a configuration change event
    /// Requirements: 5.4
    pub async fn log_config_change(
        &self,
        user_id: Uuid,
        old_config: &AutoApproveConfig,
        new_config: &AutoApproveConfig,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::config_changed(
                user_id, old_config, new_config,
            ));
        }

        let entry = SupervisionAuditEntry::config_changed(user_id, old_config, new_config);

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                user_id = %user_id,
                old_threshold = %old_config.confidence_threshold,
                new_threshold = %new_config.confidence_threshold,
                "📝 Supervision Audit: Configuration change logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log a queued-for-review event
    pub async fn log_queued_for_review(
        &self,
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::queued_for_review(
                match_id,
                ai_confidence,
                ai_explanation,
                reason,
                safety_checks,
            ));
        }

        let entry = SupervisionAuditEntry::queued_for_review(
            match_id,
            ai_confidence,
            ai_explanation,
            reason.clone(),
            safety_checks,
        );

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                match_id = %match_id,
                ai_confidence = %ai_confidence,
                reason = %reason,
                "📝 Supervision Audit: Queued for review logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log a blocked event
    pub async fn log_blocked(
        &self,
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::blocked(
                match_id,
                ai_confidence,
                ai_explanation,
                reason,
                safety_checks,
            ));
        }

        let entry = SupervisionAuditEntry::blocked(
            match_id,
            ai_confidence,
            ai_explanation,
            reason.clone(),
            safety_checks,
        );

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                match_id = %match_id,
                reason = %reason,
                "📝 Supervision Audit: Blocked logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log an undo event
    pub async fn log_undo(
        &self,
        match_id: Uuid,
        user_id: Uuid,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::undo_approval(match_id, user_id));
        }

        let entry = SupervisionAuditEntry::undo_approval(match_id, user_id);

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                match_id = %match_id,
                user_id = %user_id,
                "📝 Supervision Audit: Undo logged"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log a system paused event
    pub async fn log_system_paused(
        &self,
        reason: String,
    ) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::system_paused(reason));
        }

        let entry = SupervisionAuditEntry::system_paused(reason.clone());

        if self.should_trace() {
            tracing::warn!(
                entry_id = %entry.id,
                reason = %reason,
                "📝 Supervision Audit: System paused"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Log a system resumed event
    pub async fn log_system_resumed(&self) -> Result<SupervisionAuditEntry, SupervisionAuditError> {
        if !self.is_enabled() {
            return Ok(SupervisionAuditEntry::system_resumed());
        }

        let entry = SupervisionAuditEntry::system_resumed();

        if self.should_trace() {
            tracing::info!(
                entry_id = %entry.id,
                "📝 Supervision Audit: System resumed"
            );
        }

        self.repository.save(&entry).await?;
        Ok(entry)
    }

    /// Query audit entries
    /// Requirements: 2.3
    pub async fn query(
        &self,
        filter: &SupervisionAuditFilter,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError> {
        self.repository.query(filter).await
    }

    /// Get entries for a specific match
    pub async fn get_match_history(
        &self,
        match_id: Uuid,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError> {
        self.repository.get_for_match(match_id).await
    }

    /// Get recent entries
    pub async fn get_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<SupervisionAuditEntry>, SupervisionAuditError> {
        let filter = SupervisionAuditFilter::new().with_limit(limit);
        self.repository.query(&filter).await
    }

    /// Get configuration
    pub fn get_config(&self) -> SupervisionAuditConfig {
        self.config.read().unwrap().clone()
    }

    /// Set configuration
    pub fn set_config(&self, config: SupervisionAuditConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable audit trail
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervision_event_type_display() {
        assert_eq!(
            SupervisionEventType::AutoApproved.to_string(),
            "AUTO_APPROVED"
        );
        assert_eq!(SupervisionEventType::Overridden.to_string(), "OVERRIDDEN");
        assert_eq!(
            SupervisionEventType::ConfigChanged.to_string(),
            "CONFIG_CHANGED"
        );
    }

    #[test]
    fn test_supervision_audit_entry_auto_approved() {
        let match_id = Uuid::new_v4();
        let entry = SupervisionAuditEntry::auto_approved(
            match_id,
            0.92,
            "High confidence match".to_string(),
            vec![SafetyCheckResult::passed("blocklist")],
        );

        assert_eq!(entry.match_id, Some(match_id));
        assert_eq!(entry.event_type, SupervisionEventType::AutoApproved);
        assert_eq!(entry.ai_confidence, Some(0.92));
        assert!(entry.ai_explanation.is_some());
        assert!(entry.has_complete_auto_approval_data());
        assert!(!entry.overridden);
    }

    #[test]
    fn test_supervision_audit_entry_overridden() {
        let match_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let entry = SupervisionAuditEntry::overridden(
            match_id,
            user_id,
            "Incorrect match".to_string(),
            0.88,
            "Original explanation".to_string(),
        );

        assert_eq!(entry.match_id, Some(match_id));
        assert_eq!(entry.event_type, SupervisionEventType::Overridden);
        assert!(entry.overridden);
        assert_eq!(entry.override_by, Some(user_id));
        assert!(entry.override_reason.is_some());
        assert!(entry.override_at.is_some());
        assert!(entry.has_complete_override_data());
    }

    #[test]
    fn test_supervision_audit_entry_config_changed() {
        let user_id = Uuid::new_v4();
        let old_config = AutoApproveConfig::default();
        let new_config = AutoApproveConfig {
            confidence_threshold: 0.90,
            ..Default::default()
        };

        let entry = SupervisionAuditEntry::config_changed(user_id, &old_config, &new_config);

        assert_eq!(entry.event_type, SupervisionEventType::ConfigChanged);
        assert!(entry.has_complete_config_change_data());
        assert!(entry.metadata.is_some());
    }

    #[test]
    fn test_supervision_audit_filter_matches() {
        let match_id = Uuid::new_v4();
        let entry =
            SupervisionAuditEntry::auto_approved(match_id, 0.92, "Test".to_string(), vec![]);

        // Match by match_id
        let filter = SupervisionAuditFilter::new().for_match(match_id);
        assert!(filter.matches(&entry));

        // Match by event type
        let filter = SupervisionAuditFilter::new().of_type(SupervisionEventType::AutoApproved);
        assert!(filter.matches(&entry));

        // Match by confidence range
        let filter = SupervisionAuditFilter::new().in_confidence_range(0.90, 0.95);
        assert!(filter.matches(&entry));

        // No match - wrong event type
        let filter = SupervisionAuditFilter::new().of_type(SupervisionEventType::Overridden);
        assert!(!filter.matches(&entry));

        // No match - confidence out of range
        let filter = SupervisionAuditFilter::new().in_confidence_range(0.95, 1.0);
        assert!(!filter.matches(&entry));
    }

    #[tokio::test]
    async fn test_memory_repository_save_and_query() {
        let repo = MemorySupervisionAuditRepository::new(100);
        let match_id = Uuid::new_v4();

        let entry =
            SupervisionAuditEntry::auto_approved(match_id, 0.92, "Test".to_string(), vec![]);

        repo.save(&entry).await.unwrap();
        assert_eq!(repo.len(), 1);

        let results = repo.query(&SupervisionAuditFilter::new()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, entry.id);
    }

    #[tokio::test]
    async fn test_memory_repository_circular_buffer() {
        let repo = MemorySupervisionAuditRepository::new(3);

        for i in 0..5 {
            let entry = SupervisionAuditEntry::auto_approved(
                Uuid::new_v4(),
                0.90 + (i as f64 * 0.01),
                format!("Entry {}", i),
                vec![],
            );
            repo.save(&entry).await.unwrap();
        }

        assert_eq!(repo.len(), 3);

        // Should have entries 2, 3, 4 (oldest evicted)
        let all = repo.get_all();
        assert!(all[0].ai_explanation.as_ref().unwrap().contains("Entry 2"));
        assert!(all[2].ai_explanation.as_ref().unwrap().contains("Entry 4"));
    }

    #[tokio::test]
    async fn test_supervision_audit_trail_log_auto_approval() {
        let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
        let match_id = Uuid::new_v4();

        let entry = trail
            .log_auto_approval(
                match_id,
                0.92,
                "High confidence".to_string(),
                vec![SafetyCheckResult::passed("blocklist")],
            )
            .await
            .unwrap();

        assert_eq!(entry.match_id, Some(match_id));
        assert!(entry.has_complete_auto_approval_data());

        let history = trail.get_match_history(match_id).await.unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn test_supervision_audit_trail_log_override() {
        let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
        let match_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = trail
            .log_override(
                match_id,
                user_id,
                "Incorrect match".to_string(),
                0.88,
                "Original explanation".to_string(),
            )
            .await
            .unwrap();

        assert!(entry.has_complete_override_data());
    }

    #[tokio::test]
    async fn test_supervision_audit_trail_log_config_change() {
        let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());
        let user_id = Uuid::new_v4();
        let old_config = AutoApproveConfig::default();
        let new_config = AutoApproveConfig {
            confidence_threshold: 0.90,
            ..Default::default()
        };

        let entry = trail
            .log_config_change(user_id, &old_config, &new_config)
            .await
            .unwrap();

        assert!(entry.has_complete_config_change_data());
    }

    #[tokio::test]
    async fn test_supervision_audit_trail_query_with_filter() {
        let trail = SupervisionAuditTrail::new(SupervisionAuditConfig::default());

        // Log some entries
        let match_id1 = Uuid::new_v4();
        let match_id2 = Uuid::new_v4();

        trail
            .log_auto_approval(match_id1, 0.92, "Test 1".to_string(), vec![])
            .await
            .unwrap();
        trail
            .log_auto_approval(match_id2, 0.88, "Test 2".to_string(), vec![])
            .await
            .unwrap();
        trail
            .log_override(
                match_id1,
                Uuid::new_v4(),
                "Override".to_string(),
                0.92,
                "Test 1".to_string(),
            )
            .await
            .unwrap();

        // Query all
        let all = trail.query(&SupervisionAuditFilter::new()).await.unwrap();
        assert_eq!(all.len(), 3);

        // Query by event type
        let overrides = trail
            .query(&SupervisionAuditFilter::new().of_type(SupervisionEventType::Overridden))
            .await
            .unwrap();
        assert_eq!(overrides.len(), 1);

        // Query by match
        let match1_history = trail.get_match_history(match_id1).await.unwrap();
        assert_eq!(match1_history.len(), 2);
    }
}
