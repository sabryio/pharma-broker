//! Enhanced Audit Trail Module
//!
//! Ported from legacy/parsing/audit.go
//!
//! Provides comprehensive audit logging for match actions, configuration changes,
//! and calibration events. Supports multiple backends (memory, file, database).

use std::collections::VecDeque;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::MatchStatus;
use crate::matching::MatchAction;

/// Type alias for compatibility with Go code
pub type ActionType = MatchAction;

// =============================================================================
// Audit Event Types
// =============================================================================

/// Type of audit event
/// Ported from Go: AuditEventType (audit.go:17-26)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditEventType {
    /// Match was created
    MatchCreated,
    /// Match was auto-confirmed
    MatchAutoConfirmed,
    /// Match was suggested to operator
    MatchSuggested,
    /// Match was queued for review
    MatchQueuedReview,
    /// Match was ignored
    MatchIgnored,
    /// Manual action taken on match
    MatchManualAction,
    /// Configuration was changed
    ConfigChanged,
    /// Calibration was reset
    CalibrationReset,
    /// Threshold was adjusted
    ThresholdAdjusted,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MatchCreated => write!(f, "MATCH_CREATED"),
            Self::MatchAutoConfirmed => write!(f, "MATCH_AUTO_CONFIRMED"),
            Self::MatchSuggested => write!(f, "MATCH_SUGGESTED"),
            Self::MatchQueuedReview => write!(f, "MATCH_QUEUED_REVIEW"),
            Self::MatchIgnored => write!(f, "MATCH_IGNORED"),
            Self::MatchManualAction => write!(f, "MATCH_MANUAL_ACTION"),
            Self::ConfigChanged => write!(f, "CONFIG_CHANGED"),
            Self::CalibrationReset => write!(f, "CALIBRATION_RESET"),
            Self::ThresholdAdjusted => write!(f, "THRESHOLD_ADJUSTED"),
        }
    }
}

// =============================================================================
// Audit Entry
// =============================================================================

/// A single audit log entry
/// Ported from Go: AuditEntry (audit.go:32-45)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: String,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Type of event
    pub event_type: AuditEventType,
    /// Match ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_id: Option<String>,
    /// Offer ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    /// Request ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Action taken
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<ActionType>,
    /// Match score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Match status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MatchStatus>,
    /// Reason for the action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Actor (SYSTEM, AUTO, or user ID)
    pub actor: String,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl AuditEntry {
    /// Create a new audit entry with auto-generated ID
    pub fn new(event_type: AuditEventType, actor: impl Into<String>) -> Self {
        Self {
            id: generate_audit_id(),
            timestamp: Utc::now(),
            event_type,
            match_id: None,
            offer_id: None,
            request_id: None,
            action: None,
            score: None,
            status: None,
            reason: None,
            actor: actor.into(),
            metadata: None,
        }
    }

    /// Builder: set match ID
    pub fn with_match_id(mut self, id: impl Into<String>) -> Self {
        self.match_id = Some(id.into());
        self
    }

    /// Builder: set offer ID
    pub fn with_offer_id(mut self, id: impl Into<String>) -> Self {
        self.offer_id = Some(id.into());
        self
    }

    /// Builder: set request ID
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// Builder: set action
    pub fn with_action(mut self, action: ActionType) -> Self {
        self.action = Some(action);
        self
    }

    /// Builder: set score
    pub fn with_score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// Builder: set status
    pub fn with_status(mut self, status: MatchStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Builder: set reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Builder: set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Generate a unique audit entry ID
/// Ported from Go: generateAuditID (audit.go:143)
fn generate_audit_id() -> String {
    Utc::now().format("%Y%m%d%H%M%S%.6f").to_string()
}

// =============================================================================
// Audit Filter
// =============================================================================

/// Filter for querying audit logs
/// Ported from Go: AuditFilter (audit.go:56-62)
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// Filter by match ID
    pub match_id: Option<String>,
    /// Filter by event type
    pub event_type: Option<AuditEventType>,
    /// Filter by actor
    pub actor: Option<String>,
    /// Start time (inclusive)
    pub start_time: Option<DateTime<Utc>>,
    /// End time (inclusive)
    pub end_time: Option<DateTime<Utc>>,
    /// Maximum results to return
    pub limit: Option<usize>,
}

impl AuditFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by match ID
    pub fn for_match(mut self, match_id: impl Into<String>) -> Self {
        self.match_id = Some(match_id.into());
        self
    }

    /// Filter by event type
    pub fn of_type(mut self, event_type: AuditEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Filter by actor
    pub fn by_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Filter by time range
    pub fn in_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// Limit results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

// =============================================================================
// Audit Logger Trait
// =============================================================================

/// Trait for audit logging backends
/// Ported from Go: AuditLogger interface (audit.go:49-52)
#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    /// Log an audit entry
    async fn log(&self, entry: &AuditEntry) -> Result<(), AuditError>;

    /// Query audit entries
    async fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, AuditError>;
}

/// Audit error type
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// =============================================================================
// In-Memory Audit Logger
// =============================================================================

/// In-memory audit logger with circular buffer
/// Ported from Go: MemoryAuditLogger (audit.go:68-75)
pub struct MemoryAuditLogger {
    entries: RwLock<VecDeque<AuditEntry>>,
    max_size: usize,
    stats: MemoryAuditStats,
}

#[derive(Debug, Default)]
struct MemoryAuditStats {
    total_logged: AtomicU64,
    total_queries: AtomicU64,
    entries_evicted: AtomicU64,
}

impl MemoryAuditLogger {
    /// Create a new in-memory audit logger
    /// Ported from Go: NewMemoryAuditLogger (audit.go:78-86)
    pub fn new(max_size: usize) -> Self {
        let max_size = if max_size == 0 { 10000 } else { max_size };
        Self {
            entries: RwLock::new(VecDeque::with_capacity(max_size)),
            max_size,
            stats: MemoryAuditStats::default(),
        }
    }

    /// Get all entries (for testing/debugging)
    pub fn get_all(&self) -> Vec<AuditEntry> {
        self.entries.read().unwrap().iter().cloned().collect()
    }

    /// Get entry count
    pub fn count(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Get statistics
    pub fn get_stats(&self) -> MemoryAuditStatsSnapshot {
        MemoryAuditStatsSnapshot {
            total_logged: self.stats.total_logged.load(Ordering::Relaxed),
            total_queries: self.stats.total_queries.load(Ordering::Relaxed),
            entries_evicted: self.stats.entries_evicted.load(Ordering::Relaxed),
            current_size: self.count(),
            max_size: self.max_size,
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
    }
}

/// Statistics snapshot for memory audit logger
#[derive(Debug, Clone, Serialize)]
pub struct MemoryAuditStatsSnapshot {
    pub total_logged: u64,
    pub total_queries: u64,
    pub entries_evicted: u64,
    pub current_size: usize,
    pub max_size: usize,
}

#[async_trait::async_trait]
impl AuditLogger for MemoryAuditLogger {
    /// Log an audit entry
    /// Ported from Go: MemoryAuditLogger.Log (audit.go:89-101)
    async fn log(&self, entry: &AuditEntry) -> Result<(), AuditError> {
        let mut entries = self.entries.write().unwrap();

        // Evict oldest if at capacity
        if entries.len() >= self.max_size {
            entries.pop_front();
            self.stats.entries_evicted.fetch_add(1, Ordering::Relaxed);
        }

        entries.push_back(entry.clone());
        self.stats.total_logged.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Query audit entries
    /// Ported from Go: MemoryAuditLogger.Query (audit.go:104-127)
    async fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, AuditError> {
        self.stats.total_queries.fetch_add(1, Ordering::Relaxed);

        let entries = self.entries.read().unwrap();
        let limit = filter.limit.unwrap_or(100);
        let mut results = Vec::with_capacity(limit.min(entries.len()));

        for entry in entries.iter().rev() {
            // Apply filters
            if let Some(ref match_id) = filter.match_id {
                if entry.match_id.as_ref() != Some(match_id) {
                    continue;
                }
            }
            if let Some(event_type) = filter.event_type {
                if entry.event_type != event_type {
                    continue;
                }
            }
            if let Some(ref actor) = filter.actor {
                if &entry.actor != actor {
                    continue;
                }
            }
            if let Some(start) = filter.start_time {
                if entry.timestamp < start {
                    continue;
                }
            }
            if let Some(end) = filter.end_time {
                if entry.timestamp > end {
                    continue;
                }
            }

            results.push(entry.clone());
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }
}

// =============================================================================
// Audit Trail Configuration
// =============================================================================

/// Configuration for the audit trail
/// Ported from Go: AuditTrailConfig (audit.go:133-138)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrailConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Log to structured logger
    pub log_to_tracing: bool,
    /// Retention period in days
    pub retention_days: u32,
}

impl Default for AuditTrailConfig {
    /// Default configuration
    /// Ported from Go: DefaultAuditTrailConfig (audit.go:141-147)
    fn default() -> Self {
        Self {
            enabled: true,
            log_to_tracing: true,
            retention_days: 90,
        }
    }
}

// =============================================================================
// Audit Trail Manager
// =============================================================================

/// Manages audit logging for match actions
/// Ported from Go: AuditTrail (audit.go:150-157)
pub struct AuditTrail<L: AuditLogger = MemoryAuditLogger> {
    config: RwLock<AuditTrailConfig>,
    logger: L,
}

impl AuditTrail<MemoryAuditLogger> {
    /// Create a new audit trail with in-memory logger
    pub fn new(config: AuditTrailConfig) -> Self {
        Self {
            config: RwLock::new(config),
            logger: MemoryAuditLogger::new(10000),
        }
    }
}

impl<L: AuditLogger> AuditTrail<L> {
    /// Create a new audit trail with custom logger
    /// Ported from Go: NewAuditTrail (audit.go:160-167)
    pub fn with_logger(config: AuditTrailConfig, logger: L) -> Self {
        Self {
            config: RwLock::new(config),
            logger,
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

    // =========================================================================
    // Match Action Logging
    // =========================================================================

    /// Log a match action
    /// Ported from Go: AuditTrail.LogMatchAction (audit.go:170-207)
    pub async fn log_match_action(
        &self,
        match_id: &str,
        offer_id: &str,
        request_id: &str,
        action: ActionType,
        score: f64,
        status: MatchStatus,
        reason: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AuditError> {
        if !self.is_enabled() {
            return Ok(());
        }

        let event_type = action_to_event_type(action);
        let actor = if action == ActionType::AutoConfirm {
            "AUTO"
        } else {
            "SYSTEM"
        };

        // Log to tracing first (before moving status)
        if self.should_trace() {
            tracing::info!(
                event_type = %event_type,
                match_id = %match_id,
                action = ?action,
                score = %score,
                status = ?&status,
                actor = %actor,
                reason = %reason,
                "📝 Audit: Match action logged"
            );
        }

        let entry = AuditEntry::new(event_type, actor)
            .with_match_id(match_id)
            .with_offer_id(offer_id)
            .with_request_id(request_id)
            .with_action(action)
            .with_score(score)
            .with_status(status)
            .with_reason(reason);

        let entry = if let Some(meta) = metadata {
            entry.with_metadata(meta)
        } else {
            entry
        };

        self.logger.log(&entry).await
    }

    /// Log a configuration change
    /// Ported from Go: AuditTrail.LogConfigChange (audit.go:210-233)
    pub async fn log_config_change(
        &self,
        config_type: &str,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        actor: &str,
    ) -> Result<(), AuditError> {
        if !self.is_enabled() {
            return Ok(());
        }

        let entry = AuditEntry::new(AuditEventType::ConfigChanged, actor)
            .with_reason(format!("Configuration changed: {}", config_type))
            .with_metadata(serde_json::json!({
                "config_type": config_type,
                "old_value": old_value,
                "new_value": new_value,
            }));

        if self.should_trace() {
            tracing::info!(
                audit_id = %entry.id,
                config_type = %config_type,
                actor = %actor,
                "📝 Audit: Configuration changed"
            );
        }

        self.logger.log(&entry).await
    }

    /// Log a calibration reset
    /// Ported from Go: AuditTrail.LogCalibrationReset (audit.go:236-254)
    pub async fn log_calibration_reset(&self, actor: &str, reason: &str) -> Result<(), AuditError> {
        if !self.is_enabled() {
            return Ok(());
        }

        let entry = AuditEntry::new(AuditEventType::CalibrationReset, actor).with_reason(reason);

        if self.should_trace() {
            tracing::info!(
                audit_id = %entry.id,
                actor = %actor,
                reason = %reason,
                "📝 Audit: Calibration reset"
            );
        }

        self.logger.log(&entry).await
    }

    /// Log a threshold adjustment
    pub async fn log_threshold_adjustment(
        &self,
        threshold_type: &str,
        old_value: f64,
        new_value: f64,
        reason: &str,
    ) -> Result<(), AuditError> {
        if !self.is_enabled() {
            return Ok(());
        }

        let entry = AuditEntry::new(AuditEventType::ThresholdAdjusted, "SYSTEM")
            .with_reason(reason)
            .with_metadata(serde_json::json!({
                "threshold_type": threshold_type,
                "old_value": old_value,
                "new_value": new_value,
            }));

        if self.should_trace() {
            tracing::info!(
                audit_id = %entry.id,
                threshold_type = %threshold_type,
                old_value = %old_value,
                new_value = %new_value,
                reason = %reason,
                "📝 Audit: Threshold adjusted"
            );
        }

        self.logger.log(&entry).await
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Query audit entries
    /// Ported from Go: AuditTrail.Query (audit.go:257-262)
    pub async fn query(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, AuditError> {
        self.logger.query(filter).await
    }

    /// Get match history
    /// Ported from Go: AuditTrail.GetMatchHistory (audit.go:265-267)
    pub async fn get_match_history(&self, match_id: &str) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(&AuditFilter::new().for_match(match_id).with_limit(100))
            .await
    }

    /// Get recent actions
    /// Ported from Go: AuditTrail.GetRecentActions (audit.go:270-272)
    pub async fn get_recent_actions(&self, limit: usize) -> Result<Vec<AuditEntry>, AuditError> {
        self.query(&AuditFilter::new().with_limit(limit)).await
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    /// Get current configuration
    /// Ported from Go: AuditTrail.GetConfig (audit.go:275-278)
    pub fn get_config(&self) -> AuditTrailConfig {
        self.config.read().unwrap().clone()
    }

    /// Set configuration
    /// Ported from Go: AuditTrail.SetConfig (audit.go:281-289)
    pub fn set_config(&self, config: AuditTrailConfig) {
        tracing::info!(
            enabled = config.enabled,
            retention_days = config.retention_days,
            "Audit trail configuration updated"
        );
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable audit trail
    /// Ported from Go: AuditTrail.Enable (audit.go:292-298)
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
        tracing::info!(enabled = enabled, "Audit trail toggled");
    }
}

/// Convert action type to audit event type
/// Ported from Go: AuditTrail.actionToEventType (audit.go:256-267)
fn action_to_event_type(action: ActionType) -> AuditEventType {
    match action {
        ActionType::AutoConfirm => AuditEventType::MatchAutoConfirmed,
        ActionType::SuggestToOperator => AuditEventType::MatchSuggested,
        ActionType::QueueForReview => AuditEventType::MatchQueuedReview,
        ActionType::Ignore => AuditEventType::MatchIgnored,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // =========================================================================
    // AuditEntry Tests
    // =========================================================================

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM");

        assert!(!entry.id.is_empty());
        assert_eq!(entry.event_type, AuditEventType::MatchCreated);
        assert_eq!(entry.actor, "SYSTEM");
        assert!(entry.match_id.is_none());
    }

    #[test]
    fn test_audit_entry_builder() {
        let entry = AuditEntry::new(AuditEventType::MatchAutoConfirmed, "AUTO")
            .with_match_id("match-123")
            .with_offer_id("offer-456")
            .with_request_id("request-789")
            .with_action(ActionType::AutoConfirm)
            .with_score(0.95)
            .with_status(MatchStatus::Confirmed)
            .with_reason("High confidence match");

        assert_eq!(entry.match_id, Some("match-123".to_string()));
        assert_eq!(entry.offer_id, Some("offer-456".to_string()));
        assert_eq!(entry.request_id, Some("request-789".to_string()));
        assert_eq!(entry.action, Some(ActionType::AutoConfirm));
        assert_eq!(entry.score, Some(0.95));
        assert_eq!(entry.status, Some(MatchStatus::Confirmed));
        assert_eq!(entry.reason, Some("High confidence match".to_string()));
    }

    #[test]
    fn test_audit_entry_to_json() {
        let entry = AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM")
            .with_match_id("match-123")
            .with_score(0.85);

        let json = entry.to_json();
        assert!(json.contains("MATCH_CREATED"));
        assert!(json.contains("match-123"));
        assert!(json.contains("0.85"));
    }

    // =========================================================================
    // AuditFilter Tests
    // =========================================================================

    #[test]
    fn test_audit_filter_builder() {
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now();

        let filter = AuditFilter::new()
            .for_match("match-123")
            .of_type(AuditEventType::MatchAutoConfirmed)
            .by_actor("AUTO")
            .in_range(start, end)
            .with_limit(50);

        assert_eq!(filter.match_id, Some("match-123".to_string()));
        assert_eq!(filter.event_type, Some(AuditEventType::MatchAutoConfirmed));
        assert_eq!(filter.actor, Some("AUTO".to_string()));
        assert_eq!(filter.limit, Some(50));
    }

    // =========================================================================
    // MemoryAuditLogger Tests
    // =========================================================================

    #[tokio::test]
    async fn test_memory_logger_log() {
        let logger = MemoryAuditLogger::new(100);
        let entry = AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM");

        logger.log(&entry).await.unwrap();

        assert_eq!(logger.count(), 1);
        let stats = logger.get_stats();
        assert_eq!(stats.total_logged, 1);
    }

    #[tokio::test]
    async fn test_memory_logger_circular_buffer() {
        let logger = MemoryAuditLogger::new(3);

        for i in 0..5 {
            let entry = AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM")
                .with_match_id(i.to_string());
            logger.log(&entry).await.unwrap();
        }

        assert_eq!(logger.count(), 3);
        let stats = logger.get_stats();
        assert_eq!(stats.entries_evicted, 2);

        // Should have entries 2, 3, 4 (oldest evicted)
        let all = logger.get_all();
        assert_eq!(all[0].match_id, Some("2".to_string()));
        assert_eq!(all[2].match_id, Some("4".to_string()));
    }

    #[tokio::test]
    async fn test_memory_logger_query_by_match_id() {
        let logger = MemoryAuditLogger::new(100);

        for i in 0..3 {
            let entry = AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM")
                .with_match_id(format!("match-{}", i % 2));
            logger.log(&entry).await.unwrap();
        }

        let results = logger
            .query(&AuditFilter::new().for_match("match-0"))
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_logger_query_by_event_type() {
        let logger = MemoryAuditLogger::new(100);

        logger
            .log(&AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM"))
            .await
            .unwrap();
        logger
            .log(&AuditEntry::new(AuditEventType::MatchAutoConfirmed, "AUTO"))
            .await
            .unwrap();
        logger
            .log(&AuditEntry::new(AuditEventType::ConfigChanged, "ADMIN"))
            .await
            .unwrap();

        let results = logger
            .query(&AuditFilter::new().of_type(AuditEventType::MatchAutoConfirmed))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_type, AuditEventType::MatchAutoConfirmed);
    }

    #[tokio::test]
    async fn test_memory_logger_query_limit() {
        let logger = MemoryAuditLogger::new(100);

        for _ in 0..10 {
            logger
                .log(&AuditEntry::new(AuditEventType::MatchCreated, "SYSTEM"))
                .await
                .unwrap();
        }

        let results = logger
            .query(&AuditFilter::new().with_limit(5))
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
    }

    // =========================================================================
    // AuditTrail Tests
    // =========================================================================

    #[tokio::test]
    async fn test_audit_trail_log_match_action() {
        let trail = AuditTrail::new(AuditTrailConfig::default());

        trail
            .log_match_action(
                "match-123",
                "offer-456",
                "request-789",
                ActionType::AutoConfirm,
                0.95,
                MatchStatus::Confirmed,
                "High confidence",
                None,
            )
            .await
            .unwrap();

        let history = trail.get_match_history("match-123").await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, AuditEventType::MatchAutoConfirmed);
        assert_eq!(history[0].actor, "AUTO");
    }

    #[tokio::test]
    async fn test_audit_trail_log_config_change() {
        let trail = AuditTrail::new(AuditTrailConfig::default());

        trail
            .log_config_change(
                "confidence_threshold",
                serde_json::json!(0.7),
                serde_json::json!(0.8),
                "admin-user",
            )
            .await
            .unwrap();

        let recent = trail.get_recent_actions(10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_type, AuditEventType::ConfigChanged);
        assert_eq!(recent[0].actor, "admin-user");
    }

    #[tokio::test]
    async fn test_audit_trail_disabled() {
        let config = AuditTrailConfig {
            enabled: false,
            ..Default::default()
        };
        let trail = AuditTrail::new(config);

        trail
            .log_match_action(
                "match-123",
                "offer-456",
                "request-789",
                ActionType::AutoConfirm,
                0.95,
                MatchStatus::Confirmed,
                "Test",
                None,
            )
            .await
            .unwrap();

        let history = trail.get_match_history("match-123").await.unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_audit_trail_config_update() {
        let trail = AuditTrail::new(AuditTrailConfig::default());

        assert!(trail.get_config().enabled);

        trail.enable(false);
        assert!(!trail.get_config().enabled);

        trail.set_config(AuditTrailConfig {
            enabled: true,
            log_to_tracing: false,
            retention_days: 30,
        });

        let config = trail.get_config();
        assert!(config.enabled);
        assert!(!config.log_to_tracing);
        assert_eq!(config.retention_days, 30);
    }

    // =========================================================================
    // Event Type Tests
    // =========================================================================

    #[rstest]
    #[case(ActionType::AutoConfirm, AuditEventType::MatchAutoConfirmed)]
    #[case(ActionType::SuggestToOperator, AuditEventType::MatchSuggested)]
    #[case(ActionType::QueueForReview, AuditEventType::MatchQueuedReview)]
    #[case(ActionType::Ignore, AuditEventType::MatchIgnored)]
    fn test_action_to_event_type(#[case] action: ActionType, #[case] expected: AuditEventType) {
        assert_eq!(action_to_event_type(action), expected);
    }

    #[rstest]
    #[case(AuditEventType::MatchCreated, "MATCH_CREATED")]
    #[case(AuditEventType::MatchAutoConfirmed, "MATCH_AUTO_CONFIRMED")]
    #[case(AuditEventType::ConfigChanged, "CONFIG_CHANGED")]
    fn test_event_type_display(#[case] event_type: AuditEventType, #[case] expected: &str) {
        assert_eq!(event_type.to_string(), expected);
    }
}
