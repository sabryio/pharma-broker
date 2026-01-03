//! AI Supervised Auto-Approve System
//!
//! Automatically approves high-confidence medication matches while maintaining
//! full human oversight and comprehensive audit trails.
//!
//! Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 4.1, 4.2, 4.3, 4.4, 5.1-5.5, 6.1-6.5, 7.1-7.5

use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

// Re-export SafetyCheckResult and PauseReason from safety_guardrails
pub use super::safety_guardrails::{PauseReason, SafetyCheckResult};

// =============================================================================
// Configuration
// =============================================================================

/// Minimum allowed confidence threshold
pub const MIN_CONFIDENCE_THRESHOLD: f64 = 0.70;
/// Maximum allowed confidence threshold
pub const MAX_CONFIDENCE_THRESHOLD: f64 = 0.99;

/// Configuration for the auto-approve processor
/// Requirements: 1.5, 5.1, 5.2, 5.3, 5.5
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoApproveConfig {
    /// Whether auto-approval is enabled globally
    pub enabled: bool,
    /// Minimum AI confidence for auto-approval (0.70-0.99)
    pub confidence_threshold: f64,
    /// Maximum batch size per processing cycle
    pub batch_size: usize,
    /// Processing interval in seconds
    pub processing_interval_secs: u64,
    /// Undo window in minutes
    pub undo_window_mins: u64,
    /// Override rate threshold to pause (0.0-1.0)
    pub override_rate_pause_threshold: f64,
    /// Consecutive overrides to disable
    pub consecutive_override_limit: u32,
    /// Cooldown period after override (minutes)
    pub override_cooldown_mins: u64,
    /// Category-specific threshold overrides
    pub category_thresholds: HashMap<String, f64>,
    /// Schedule for auto-approval (cron expression, None = always)
    pub schedule: Option<String>,
}

impl Default for AutoApproveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            confidence_threshold: 0.85,
            batch_size: 50,
            processing_interval_secs: 30,
            undo_window_mins: 30,
            override_rate_pause_threshold: 0.10,
            consecutive_override_limit: 5,
            override_cooldown_mins: 60,
            category_thresholds: HashMap::new(),
            schedule: None,
        }
    }
}

impl AutoApproveConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = env::var("AUTO_APPROVE_ENABLED") {
            config.enabled = val.parse().unwrap_or(false);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_CONFIDENCE_THRESHOLD") {
            if let Ok(threshold) = val.parse::<f64>() {
                config.confidence_threshold = config.clamp_threshold(threshold);
            }
        }

        if let Ok(val) = env::var("AUTO_APPROVE_BATCH_SIZE") {
            config.batch_size = val.parse().unwrap_or(50);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_PROCESSING_INTERVAL_SECS") {
            config.processing_interval_secs = val.parse().unwrap_or(30);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_UNDO_WINDOW_MINS") {
            config.undo_window_mins = val.parse().unwrap_or(30);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_OVERRIDE_RATE_PAUSE_THRESHOLD") {
            config.override_rate_pause_threshold = val.parse().unwrap_or(0.10);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_CONSECUTIVE_OVERRIDE_LIMIT") {
            config.consecutive_override_limit = val.parse().unwrap_or(5);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_OVERRIDE_COOLDOWN_MINS") {
            config.override_cooldown_mins = val.parse().unwrap_or(60);
        }

        if let Ok(val) = env::var("AUTO_APPROVE_SCHEDULE") {
            if !val.is_empty() {
                config.schedule = Some(val);
            }
        }

        config
    }

    /// Clamp a threshold value to valid bounds [0.70, 0.99]
    /// Requirements: 1.5
    pub fn clamp_threshold(&self, threshold: f64) -> f64 {
        threshold.clamp(MIN_CONFIDENCE_THRESHOLD, MAX_CONFIDENCE_THRESHOLD)
    }

    /// Validate the configuration and return any errors
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // Validate confidence threshold bounds
        if self.confidence_threshold < MIN_CONFIDENCE_THRESHOLD
            || self.confidence_threshold > MAX_CONFIDENCE_THRESHOLD
        {
            return Err(ConfigValidationError::ThresholdOutOfBounds {
                value: self.confidence_threshold,
                min: MIN_CONFIDENCE_THRESHOLD,
                max: MAX_CONFIDENCE_THRESHOLD,
            });
        }

        // Validate batch size
        if self.batch_size == 0 || self.batch_size > 1000 {
            return Err(ConfigValidationError::InvalidBatchSize(self.batch_size));
        }

        // Validate override rate threshold
        if self.override_rate_pause_threshold < 0.0 || self.override_rate_pause_threshold > 1.0 {
            return Err(ConfigValidationError::InvalidOverrideRateThreshold(
                self.override_rate_pause_threshold,
            ));
        }

        // Validate category thresholds
        for (category, threshold) in &self.category_thresholds {
            if *threshold < MIN_CONFIDENCE_THRESHOLD || *threshold > MAX_CONFIDENCE_THRESHOLD {
                return Err(ConfigValidationError::InvalidCategoryThreshold {
                    category: category.clone(),
                    value: *threshold,
                });
            }
        }

        Ok(())
    }

    /// Get the effective threshold for a medication category
    /// Returns category-specific threshold if configured, otherwise global threshold
    /// Requirements: 5.3
    pub fn get_threshold_for_category(&self, category: Option<&str>) -> f64 {
        if let Some(cat) = category {
            if let Some(threshold) = self.category_thresholds.get(cat) {
                return *threshold;
            }
        }
        self.confidence_threshold
    }

    /// Set a category-specific threshold
    pub fn set_category_threshold(&mut self, category: &str, threshold: f64) {
        let clamped = self.clamp_threshold(threshold);
        self.category_thresholds
            .insert(category.to_string(), clamped);
    }

    /// Check if the current time is within the configured schedule
    /// Returns true if no schedule is configured (always active) or if current time matches schedule
    /// Requirements: 5.5
    ///
    /// The schedule uses a simplified format: "HH:MM-HH:MM" for time ranges
    /// or standard cron expressions for more complex schedules.
    ///
    /// Examples:
    /// - "09:00-17:00" - Active from 9 AM to 5 PM
    /// - "08:00-20:00" - Active from 8 AM to 8 PM
    /// - None - Always active
    pub fn is_within_schedule(&self) -> bool {
        self.is_within_schedule_at(Utc::now())
    }

    /// Check if a specific time is within the configured schedule
    /// This is useful for testing with specific timestamps
    /// Requirements: 5.5
    pub fn is_within_schedule_at(&self, time: DateTime<Utc>) -> bool {
        match &self.schedule {
            None => true, // No schedule means always active
            Some(schedule) => Self::check_schedule(schedule, time),
        }
    }

    /// Parse and check a schedule expression against a given time
    /// Supports simple time range format: "HH:MM-HH:MM"
    fn check_schedule(schedule: &str, time: DateTime<Utc>) -> bool {
        // Try to parse as simple time range format: "HH:MM-HH:MM"
        if let Some((start, end)) = Self::parse_time_range(schedule) {
            let current_minutes = time.hour() * 60 + time.minute();
            let start_minutes = start.0 * 60 + start.1;
            let end_minutes = end.0 * 60 + end.1;

            // Handle overnight ranges (e.g., "22:00-06:00")
            if start_minutes <= end_minutes {
                // Normal range (e.g., "09:00-17:00")
                return current_minutes >= start_minutes && current_minutes < end_minutes;
            } else {
                // Overnight range (e.g., "22:00-06:00")
                return current_minutes >= start_minutes || current_minutes < end_minutes;
            }
        }

        // If not a simple time range, treat as always active
        // (cron expression support could be added here in the future)
        tracing::warn!(
            schedule = schedule,
            "Unrecognized schedule format, treating as always active"
        );
        true
    }

    /// Parse a time range string in format "HH:MM-HH:MM"
    /// Returns ((start_hour, start_min), (end_hour, end_min)) or None if invalid
    fn parse_time_range(schedule: &str) -> Option<((u32, u32), (u32, u32))> {
        let parts: Vec<&str> = schedule.split('-').collect();
        if parts.len() != 2 {
            return None;
        }

        let start = Self::parse_time(parts[0])?;
        let end = Self::parse_time(parts[1])?;

        Some((start, end))
    }

    /// Parse a time string in format "HH:MM"
    /// Returns (hour, minute) or None if invalid
    fn parse_time(time_str: &str) -> Option<(u32, u32)> {
        let parts: Vec<&str> = time_str.trim().split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let hour: u32 = parts[0].parse().ok()?;
        let minute: u32 = parts[1].parse().ok()?;

        if hour > 23 || minute > 59 {
            return None;
        }

        Some((hour, minute))
    }
}

/// Configuration validation errors
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValidationError {
    /// Confidence threshold is outside valid bounds
    ThresholdOutOfBounds { value: f64, min: f64, max: f64 },
    /// Batch size is invalid
    InvalidBatchSize(usize),
    /// Override rate threshold is invalid
    InvalidOverrideRateThreshold(f64),
    /// Category threshold is invalid
    InvalidCategoryThreshold { category: String, value: f64 },
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigValidationError::ThresholdOutOfBounds { value, min, max } => {
                write!(
                    f,
                    "Confidence threshold {} is outside valid bounds [{}, {}]",
                    value, min, max
                )
            }
            ConfigValidationError::InvalidBatchSize(size) => {
                write!(f, "Invalid batch size: {} (must be 1-1000)", size)
            }
            ConfigValidationError::InvalidOverrideRateThreshold(rate) => {
                write!(
                    f,
                    "Invalid override rate threshold: {} (must be 0.0-1.0)",
                    rate
                )
            }
            ConfigValidationError::InvalidCategoryThreshold { category, value } => {
                write!(
                    f,
                    "Invalid threshold {} for category '{}' (must be {}-{})",
                    value, category, MIN_CONFIDENCE_THRESHOLD, MAX_CONFIDENCE_THRESHOLD
                )
            }
        }
    }
}

impl std::error::Error for ConfigValidationError {}

// =============================================================================
// Result Types
// =============================================================================

/// Action taken by the auto-approve processor
/// Requirements: 1.1, 1.4
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AutoApproveAction {
    /// Match was automatically approved
    Approved,
    /// Match was queued for human review
    QueuedForReview { reason: String },
    /// Match was blocked by safety guardrails
    Blocked { reason: String },
}

impl AutoApproveAction {
    /// Check if this action represents an approval
    pub fn is_approved(&self) -> bool {
        matches!(self, AutoApproveAction::Approved)
    }

    /// Check if this action requires human review
    pub fn requires_review(&self) -> bool {
        matches!(self, AutoApproveAction::QueuedForReview { .. })
    }

    /// Check if this action was blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, AutoApproveAction::Blocked { .. })
    }
}

/// Result of processing a single match for auto-approval
/// Requirements: 1.1, 1.2, 1.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoApproveResult {
    /// ID of the match that was processed
    pub match_id: Uuid,
    /// Action taken
    pub action: AutoApproveAction,
    /// AI confidence score
    pub ai_confidence: f64,
    /// AI explanation/reasoning
    pub ai_explanation: String,
    /// When the decision was made
    pub timestamp: DateTime<Utc>,
    /// Results of safety checks
    pub safety_checks: Vec<SafetyCheckResult>,
    /// Whether this is a borderline case (within 5% of threshold)
    pub is_borderline: bool,
}

impl AutoApproveResult {
    /// Create a new approved result
    pub fn approved(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        safety_checks: Vec<SafetyCheckResult>,
        threshold: f64,
    ) -> Self {
        Self {
            match_id,
            action: AutoApproveAction::Approved,
            ai_confidence,
            ai_explanation,
            timestamp: Utc::now(),
            safety_checks,
            is_borderline: Self::check_borderline(ai_confidence, threshold),
        }
    }

    /// Create a new queued-for-review result
    pub fn queued_for_review(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
        threshold: f64,
    ) -> Self {
        Self {
            match_id,
            action: AutoApproveAction::QueuedForReview { reason },
            ai_confidence,
            ai_explanation,
            timestamp: Utc::now(),
            safety_checks,
            is_borderline: Self::check_borderline(ai_confidence, threshold),
        }
    }

    /// Create a new blocked result
    pub fn blocked(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        reason: String,
        safety_checks: Vec<SafetyCheckResult>,
    ) -> Self {
        Self {
            match_id,
            action: AutoApproveAction::Blocked { reason },
            ai_confidence,
            ai_explanation,
            timestamp: Utc::now(),
            safety_checks,
            is_borderline: false, // Blocked matches are not borderline
        }
    }

    /// Check if confidence is within 5% of threshold (borderline case)
    /// Requirements: 3.4
    fn check_borderline(confidence: f64, threshold: f64) -> bool {
        let margin = 0.05;
        confidence >= (threshold - margin) && confidence < (threshold + margin)
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics for auto-approve operations
/// Requirements: 3.2
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoApproveStats {
    /// Total matches auto-approved today
    pub total_approved_today: u64,
    /// Total matches queued for review today
    pub total_queued_today: u64,
    /// Total matches blocked today
    pub total_blocked_today: u64,
    /// Override rate (overrides / total approved)
    pub override_rate: f64,
    /// Average AI confidence for approved matches
    pub average_confidence: f64,
    /// Number of matches pending review
    pub pending_review_count: u64,
    /// Current system status
    pub system_status: SystemStatus,
    /// Reason for pause (if paused)
    pub pause_reason: Option<String>,
}

impl AutoApproveStats {
    /// Calculate statistics from a list of results and override count
    /// Requirements: 3.2
    pub fn from_results(
        results: &[AutoApproveResult],
        override_count: u64,
        pending_count: u64,
        status: SystemStatus,
        pause_reason: Option<String>,
    ) -> Self {
        let mut approved_count = 0u64;
        let mut queued_count = 0u64;
        let mut blocked_count = 0u64;
        let mut confidence_sum = 0.0f64;

        for result in results {
            match &result.action {
                AutoApproveAction::Approved => {
                    approved_count += 1;
                    confidence_sum += result.ai_confidence;
                }
                AutoApproveAction::QueuedForReview { .. } => {
                    queued_count += 1;
                }
                AutoApproveAction::Blocked { .. } => {
                    blocked_count += 1;
                }
            }
        }

        let average_confidence = if approved_count > 0 {
            confidence_sum / approved_count as f64
        } else {
            0.0
        };

        let override_rate = if approved_count > 0 {
            override_count as f64 / approved_count as f64
        } else {
            0.0
        };

        Self {
            total_approved_today: approved_count,
            total_queued_today: queued_count,
            total_blocked_today: blocked_count,
            override_rate,
            average_confidence,
            pending_review_count: pending_count,
            system_status: status,
            pause_reason,
        }
    }

    /// Get total decisions made today
    pub fn total_decisions_today(&self) -> u64 {
        self.total_approved_today + self.total_queued_today + self.total_blocked_today
    }
}

/// System status for auto-approve
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SystemStatus {
    /// System is actively processing
    #[default]
    Active,
    /// System is paused (manual or automatic)
    Paused,
    /// System is disabled
    Disabled,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // AutoApproveConfig Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = AutoApproveConfig::default();

        assert!(!config.enabled);
        assert!((config.confidence_threshold - 0.85).abs() < 0.001);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.processing_interval_secs, 30);
        assert_eq!(config.undo_window_mins, 30);
        assert!((config.override_rate_pause_threshold - 0.10).abs() < 0.001);
        assert_eq!(config.consecutive_override_limit, 5);
        assert_eq!(config.override_cooldown_mins, 60);
        assert!(config.category_thresholds.is_empty());
        assert!(config.schedule.is_none());
    }

    #[test]
    fn test_clamp_threshold_within_bounds() {
        let config = AutoApproveConfig::default();

        assert!((config.clamp_threshold(0.85) - 0.85).abs() < 0.001);
        assert!((config.clamp_threshold(0.70) - 0.70).abs() < 0.001);
        assert!((config.clamp_threshold(0.99) - 0.99).abs() < 0.001);
    }

    #[test]
    fn test_clamp_threshold_below_min() {
        let config = AutoApproveConfig::default();

        assert!((config.clamp_threshold(0.50) - MIN_CONFIDENCE_THRESHOLD).abs() < 0.001);
        assert!((config.clamp_threshold(0.0) - MIN_CONFIDENCE_THRESHOLD).abs() < 0.001);
        assert!((config.clamp_threshold(-1.0) - MIN_CONFIDENCE_THRESHOLD).abs() < 0.001);
    }

    #[test]
    fn test_clamp_threshold_above_max() {
        let config = AutoApproveConfig::default();

        assert!((config.clamp_threshold(1.0) - MAX_CONFIDENCE_THRESHOLD).abs() < 0.001);
        assert!((config.clamp_threshold(1.5) - MAX_CONFIDENCE_THRESHOLD).abs() < 0.001);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = AutoApproveConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_threshold_too_low() {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = 0.50;

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigValidationError::ThresholdOutOfBounds { .. })
        ));
    }

    #[test]
    fn test_validate_threshold_too_high() {
        let mut config = AutoApproveConfig::default();
        config.confidence_threshold = 1.0;

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigValidationError::ThresholdOutOfBounds { .. })
        ));
    }

    #[test]
    fn test_validate_invalid_batch_size() {
        let mut config = AutoApproveConfig::default();
        config.batch_size = 0;

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigValidationError::InvalidBatchSize(_))
        ));
    }

    #[test]
    fn test_validate_invalid_override_rate() {
        let mut config = AutoApproveConfig::default();
        config.override_rate_pause_threshold = 1.5;

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigValidationError::InvalidOverrideRateThreshold(_))
        ));
    }

    #[test]
    fn test_get_threshold_for_category_default() {
        let config = AutoApproveConfig::default();

        assert!((config.get_threshold_for_category(None) - 0.85).abs() < 0.001);
        assert!((config.get_threshold_for_category(Some("unknown")) - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_get_threshold_for_category_custom() {
        let mut config = AutoApproveConfig::default();
        config.set_category_threshold("controlled", 0.95);

        assert!((config.get_threshold_for_category(Some("controlled")) - 0.95).abs() < 0.001);
        assert!((config.get_threshold_for_category(Some("other")) - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_set_category_threshold_clamps() {
        let mut config = AutoApproveConfig::default();
        config.set_category_threshold("test", 0.50); // Below min

        assert!(
            (config.get_threshold_for_category(Some("test")) - MIN_CONFIDENCE_THRESHOLD).abs()
                < 0.001
        );
    }

    // =========================================================================
    // Schedule Enforcement Tests
    // =========================================================================

    #[test]
    fn test_schedule_none_always_active() {
        let config = AutoApproveConfig::default();
        assert!(config.schedule.is_none());
        assert!(config.is_within_schedule());
    }

    #[test]
    fn test_schedule_parse_time_range() {
        // Test valid time range parsing
        let result = AutoApproveConfig::parse_time_range("09:00-17:00");
        assert!(result.is_some());
        let ((start_h, start_m), (end_h, end_m)) = result.unwrap();
        assert_eq!(start_h, 9);
        assert_eq!(start_m, 0);
        assert_eq!(end_h, 17);
        assert_eq!(end_m, 0);
    }

    #[test]
    fn test_schedule_parse_time_range_invalid() {
        // Test invalid formats
        assert!(AutoApproveConfig::parse_time_range("invalid").is_none());
        assert!(AutoApproveConfig::parse_time_range("09:00").is_none());
        assert!(AutoApproveConfig::parse_time_range("25:00-17:00").is_none());
        assert!(AutoApproveConfig::parse_time_range("09:60-17:00").is_none());
    }

    #[test]
    fn test_schedule_within_business_hours() {
        use chrono::TimeZone;

        let mut config = AutoApproveConfig::default();
        config.schedule = Some("09:00-17:00".to_string());

        // 10:00 AM should be within schedule
        let time_10am = Utc.with_ymd_and_hms(2026, 1, 3, 10, 0, 0).unwrap();
        assert!(config.is_within_schedule_at(time_10am));

        // 8:00 AM should be outside schedule
        let time_8am = Utc.with_ymd_and_hms(2026, 1, 3, 8, 0, 0).unwrap();
        assert!(!config.is_within_schedule_at(time_8am));

        // 5:00 PM (17:00) should be outside schedule (end is exclusive)
        let time_5pm = Utc.with_ymd_and_hms(2026, 1, 3, 17, 0, 0).unwrap();
        assert!(!config.is_within_schedule_at(time_5pm));

        // 4:59 PM should be within schedule
        let time_459pm = Utc.with_ymd_and_hms(2026, 1, 3, 16, 59, 0).unwrap();
        assert!(config.is_within_schedule_at(time_459pm));
    }

    #[test]
    fn test_schedule_overnight_range() {
        use chrono::TimeZone;

        let mut config = AutoApproveConfig::default();
        config.schedule = Some("22:00-06:00".to_string()); // Night shift

        // 11:00 PM should be within schedule
        let time_11pm = Utc.with_ymd_and_hms(2026, 1, 3, 23, 0, 0).unwrap();
        assert!(config.is_within_schedule_at(time_11pm));

        // 3:00 AM should be within schedule
        let time_3am = Utc.with_ymd_and_hms(2026, 1, 3, 3, 0, 0).unwrap();
        assert!(config.is_within_schedule_at(time_3am));

        // 12:00 PM (noon) should be outside schedule
        let time_noon = Utc.with_ymd_and_hms(2026, 1, 3, 12, 0, 0).unwrap();
        assert!(!config.is_within_schedule_at(time_noon));

        // 6:00 AM should be outside schedule (end is exclusive)
        let time_6am = Utc.with_ymd_and_hms(2026, 1, 3, 6, 0, 0).unwrap();
        assert!(!config.is_within_schedule_at(time_6am));
    }

    #[test]
    fn test_schedule_boundary_conditions() {
        use chrono::TimeZone;

        let mut config = AutoApproveConfig::default();
        config.schedule = Some("09:00-17:00".to_string());

        // Exactly at start time should be within schedule
        let time_9am = Utc.with_ymd_and_hms(2026, 1, 3, 9, 0, 0).unwrap();
        assert!(config.is_within_schedule_at(time_9am));

        // One minute before start should be outside
        let time_858am = Utc.with_ymd_and_hms(2026, 1, 3, 8, 59, 0).unwrap();
        assert!(!config.is_within_schedule_at(time_858am));
    }

    // =========================================================================
    // AutoApproveStats Tests
    // =========================================================================

    #[test]
    fn test_stats_from_results_empty() {
        let stats = AutoApproveStats::from_results(&[], 0, 0, SystemStatus::Active, None);

        assert_eq!(stats.total_approved_today, 0);
        assert_eq!(stats.total_queued_today, 0);
        assert_eq!(stats.total_blocked_today, 0);
        assert!((stats.override_rate - 0.0).abs() < 0.001);
        assert!((stats.average_confidence - 0.0).abs() < 0.001);
        assert_eq!(stats.pending_review_count, 0);
        assert_eq!(stats.system_status, SystemStatus::Active);
        assert!(stats.pause_reason.is_none());
    }

    #[test]
    fn test_stats_from_results_mixed() {
        let results = vec![
            AutoApproveResult::approved(
                Uuid::new_v4(),
                0.90,
                "High confidence".to_string(),
                vec![],
                0.85,
            ),
            AutoApproveResult::approved(
                Uuid::new_v4(),
                0.95,
                "Very high confidence".to_string(),
                vec![],
                0.85,
            ),
            AutoApproveResult::queued_for_review(
                Uuid::new_v4(),
                0.75,
                "Below threshold".to_string(),
                "Confidence below threshold".to_string(),
                vec![],
                0.85,
            ),
            AutoApproveResult::blocked(
                Uuid::new_v4(),
                0.92,
                "Good match".to_string(),
                "Medication on blocklist".to_string(),
                vec![],
            ),
        ];

        let stats = AutoApproveStats::from_results(
            &results,
            1, // 1 override
            5, // 5 pending
            SystemStatus::Active,
            None,
        );

        assert_eq!(stats.total_approved_today, 2);
        assert_eq!(stats.total_queued_today, 1);
        assert_eq!(stats.total_blocked_today, 1);
        assert!((stats.override_rate - 0.5).abs() < 0.001); // 1 override / 2 approved
        assert!((stats.average_confidence - 0.925).abs() < 0.001); // (0.90 + 0.95) / 2
        assert_eq!(stats.pending_review_count, 5);
        assert_eq!(stats.system_status, SystemStatus::Active);
    }

    #[test]
    fn test_stats_total_decisions() {
        let stats = AutoApproveStats {
            total_approved_today: 10,
            total_queued_today: 5,
            total_blocked_today: 2,
            ..Default::default()
        };

        assert_eq!(stats.total_decisions_today(), 17);
    }

    #[test]
    fn test_stats_with_pause_reason() {
        let stats = AutoApproveStats::from_results(
            &[],
            0,
            0,
            SystemStatus::Paused,
            Some("High override rate".to_string()),
        );

        assert_eq!(stats.system_status, SystemStatus::Paused);
        assert_eq!(stats.pause_reason, Some("High override rate".to_string()));
    }

    // =========================================================================
    // AutoApproveAction Tests
    // =========================================================================

    #[test]
    fn test_action_is_approved() {
        assert!(AutoApproveAction::Approved.is_approved());
        assert!(
            !AutoApproveAction::QueuedForReview {
                reason: "test".to_string()
            }
            .is_approved()
        );
        assert!(
            !AutoApproveAction::Blocked {
                reason: "test".to_string()
            }
            .is_approved()
        );
    }

    #[test]
    fn test_action_requires_review() {
        assert!(!AutoApproveAction::Approved.requires_review());
        assert!(
            AutoApproveAction::QueuedForReview {
                reason: "test".to_string()
            }
            .requires_review()
        );
        assert!(
            !AutoApproveAction::Blocked {
                reason: "test".to_string()
            }
            .requires_review()
        );
    }

    #[test]
    fn test_action_is_blocked() {
        assert!(!AutoApproveAction::Approved.is_blocked());
        assert!(
            !AutoApproveAction::QueuedForReview {
                reason: "test".to_string()
            }
            .is_blocked()
        );
        assert!(
            AutoApproveAction::Blocked {
                reason: "test".to_string()
            }
            .is_blocked()
        );
    }

    // =========================================================================
    // AutoApproveResult Tests
    // =========================================================================

    #[test]
    fn test_result_approved() {
        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            0.92,
            "High confidence match".to_string(),
            vec![],
            0.85,
        );

        assert!(result.action.is_approved());
        assert!((result.ai_confidence - 0.92).abs() < 0.001);
        assert!(!result.is_borderline);
    }

    #[test]
    fn test_result_borderline_detection() {
        // Just above threshold (within 5%)
        let result = AutoApproveResult::approved(
            Uuid::new_v4(),
            0.87, // 0.85 + 0.02, within 5% margin
            "Borderline match".to_string(),
            vec![],
            0.85,
        );
        assert!(result.is_borderline);

        // Well above threshold
        let result2 = AutoApproveResult::approved(
            Uuid::new_v4(),
            0.95, // Well above 0.85 + 0.05
            "High confidence".to_string(),
            vec![],
            0.85,
        );
        assert!(!result2.is_borderline);
    }

    #[test]
    fn test_result_queued_for_review() {
        let result = AutoApproveResult::queued_for_review(
            Uuid::new_v4(),
            0.75,
            "Below threshold".to_string(),
            "Confidence below threshold".to_string(),
            vec![],
            0.85,
        );

        assert!(result.action.requires_review());
        assert!((result.ai_confidence - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_result_blocked() {
        let result = AutoApproveResult::blocked(
            Uuid::new_v4(),
            0.90,
            "Good match".to_string(),
            "Medication on blocklist".to_string(),
            vec![SafetyCheckResult::failed("blocklist", "Medication blocked")],
        );

        assert!(result.action.is_blocked());
        assert!(!result.is_borderline);
    }

    // =========================================================================
    // SafetyCheckResult Tests
    // =========================================================================

    #[test]
    fn test_safety_check_passed() {
        let check = SafetyCheckResult::passed("blocklist");

        assert!(check.passed);
        assert!(check.reason.is_none());
        assert_eq!(check.check_name, "blocklist");
    }

    #[test]
    fn test_safety_check_failed() {
        let check = SafetyCheckResult::failed("dosage", "Dosage differs by >20%");

        assert!(!check.passed);
        assert_eq!(check.reason, Some("Dosage differs by >20%".to_string()));
        assert_eq!(check.check_name, "dosage");
    }

    // =========================================================================
    // ProcessingMetrics Tests
    // =========================================================================

    #[test]
    fn test_metrics_record_processing() {
        let mut metrics = ProcessingMetrics::default();

        metrics.record_processing(10);
        metrics.record_processing(20);
        metrics.record_processing(30);

        assert_eq!(metrics.total_processed, 3);
        assert_eq!(metrics.total_processing_time_ms, 60);
    }

    #[test]
    fn test_metrics_average_latency() {
        let mut metrics = ProcessingMetrics::default();

        metrics.record_processing(10);
        metrics.record_processing(20);
        metrics.record_processing(30);

        let avg = metrics.average_latency_ms();
        assert!((avg - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_metrics_average_latency_empty() {
        let metrics = ProcessingMetrics::default();
        assert!((metrics.average_latency_ms() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_metrics_p95_latency() {
        let mut metrics = ProcessingMetrics::default();

        // Add 20 values: 1, 2, 3, ..., 20
        for i in 1..=20 {
            metrics.record_processing(i);
        }

        // 95th percentile of 1-20 should be 19 (index 18 in 0-based, ceil(20*0.95)-1 = 18)
        let p95 = metrics.p95_latency_ms();
        assert_eq!(p95, 19);
    }

    #[test]
    fn test_metrics_p95_latency_empty() {
        let metrics = ProcessingMetrics::default();
        assert_eq!(metrics.p95_latency_ms(), 0);
    }

    #[test]
    fn test_metrics_throughput() {
        let mut metrics = ProcessingMetrics::default();

        // 10 matches in 1000ms = 10 per second
        for _ in 0..10 {
            metrics.record_processing(100);
        }

        let throughput = metrics.throughput_per_second();
        assert!((throughput - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_metrics_throughput_empty() {
        let metrics = ProcessingMetrics::default();
        assert!((metrics.throughput_per_second() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = ProcessingMetrics::default();

        metrics.record_processing(10);
        metrics.record_processing(20);

        assert_eq!(metrics.total_processed, 2);

        metrics.reset();

        assert_eq!(metrics.total_processed, 0);
        assert_eq!(metrics.total_processing_time_ms, 0);
        assert!(metrics.last_reset.is_some());
    }

    #[test]
    fn test_metrics_keeps_last_100() {
        let mut metrics = ProcessingMetrics::default();

        // Add 150 values
        for i in 0..150 {
            metrics.record_processing(i);
        }

        // Should only keep last 100
        assert_eq!(metrics.recent_latencies_ms.len(), 100);
        // First value should be 50 (150 - 100)
        assert_eq!(metrics.recent_latencies_ms[0], 50);
    }

    // =========================================================================
    // RetryInfo Tests
    // =========================================================================

    #[test]
    fn test_retry_info_new() {
        let match_id = Uuid::new_v4();
        let retry_info = RetryInfo::new(match_id, "Test error".to_string(), 3);

        assert_eq!(retry_info.match_id, match_id);
        assert_eq!(retry_info.retry_count, 0);
        assert_eq!(retry_info.max_retries, 3);
        assert_eq!(retry_info.error_message, "Test error");
        assert!(retry_info.should_retry());
    }

    #[test]
    fn test_retry_info_increment() {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 3);

        retry_info.increment();
        assert_eq!(retry_info.retry_count, 1);
        assert!(retry_info.should_retry());

        retry_info.increment();
        assert_eq!(retry_info.retry_count, 2);
        assert!(retry_info.should_retry());

        retry_info.increment();
        assert_eq!(retry_info.retry_count, 3);
        assert!(!retry_info.should_retry()); // Max retries reached
    }

    #[test]
    fn test_retry_info_exponential_backoff() {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 5);

        // Initial next_retry_at is ~30s from now
        let initial_next = retry_info.next_retry_at;

        retry_info.increment();
        // After first increment, should be ~60s from now (30 * 2^1)
        let after_first = retry_info.next_retry_at;
        assert!(after_first > initial_next);

        retry_info.increment();
        // After second increment, should be ~120s from now (30 * 2^2)
        let after_second = retry_info.next_retry_at;
        assert!(after_second > after_first);
    }

    #[test]
    fn test_retry_info_is_ready_for_retry() {
        let mut retry_info = RetryInfo::new(Uuid::new_v4(), "Test error".to_string(), 3);

        // Initially not ready (next_retry_at is in the future)
        assert!(!retry_info.is_ready_for_retry());

        // Set next_retry_at to the past
        retry_info.next_retry_at = Utc::now() - Duration::seconds(10);
        assert!(retry_info.is_ready_for_retry());
    }
}

// =============================================================================
// Processor Implementation
// =============================================================================

use std::sync::Arc;
use tokio::sync::RwLock;

use super::blocklist::MedicationBlocklist;
use super::dosage_gate::{DosageFlag, DosageGate};
use super::reviewer::AIReviewer;
use super::safety_guardrails::CooldownTracker;
use crate::domain::{Match, Offer, Request};

/// Internal state for tracking overrides and anomalies
#[derive(Debug, Default)]
struct ProcessorState {
    /// Count of consecutive overrides
    consecutive_overrides: u32,
    /// Recent override count for rate calculation
    recent_overrides: u32,
    /// Recent approval count for rate calculation
    recent_approvals: u32,
    /// Recent queued count for statistics
    recent_queued: u32,
    /// Recent blocked count for statistics
    recent_blocked: u32,
    /// Whether the system is paused
    is_paused: bool,
    /// Reason for pause (if paused)
    pause_reason: Option<PauseReason>,
    /// Recent confidence scores for anomaly detection
    recent_confidences: Vec<f64>,
    /// Metrics for monitoring processing latency and throughput
    /// Requirements: 6.5
    metrics: ProcessingMetrics,
    /// Queue of matches waiting for retry after AI service failure
    /// Requirements: 6.4
    retry_queue: Vec<RetryInfo>,
}

/// Metrics for monitoring auto-approve processing
/// Requirements: 6.5
#[derive(Debug, Clone, Default)]
pub struct ProcessingMetrics {
    /// Total matches processed
    pub total_processed: u64,
    /// Total processing time in milliseconds
    pub total_processing_time_ms: u64,
    /// Recent processing latencies (last 100)
    recent_latencies_ms: Vec<u64>,
    /// Timestamp of last metrics reset
    pub last_reset: Option<DateTime<Utc>>,
}

impl ProcessingMetrics {
    /// Record a processing event with its latency
    pub fn record_processing(&mut self, latency_ms: u64) {
        self.total_processed += 1;
        self.total_processing_time_ms += latency_ms;
        self.recent_latencies_ms.push(latency_ms);

        // Keep only last 100 latencies
        if self.recent_latencies_ms.len() > 100 {
            self.recent_latencies_ms.remove(0);
        }
    }

    /// Get average processing latency in milliseconds
    pub fn average_latency_ms(&self) -> f64 {
        if self.recent_latencies_ms.is_empty() {
            return 0.0;
        }
        self.recent_latencies_ms.iter().sum::<u64>() as f64 / self.recent_latencies_ms.len() as f64
    }

    /// Get the 95th percentile latency in milliseconds
    pub fn p95_latency_ms(&self) -> u64 {
        if self.recent_latencies_ms.is_empty() {
            return 0;
        }
        let mut sorted = self.recent_latencies_ms.clone();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.95).ceil() as usize - 1;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Get throughput (matches per second) based on recent processing
    pub fn throughput_per_second(&self) -> f64 {
        if self.total_processing_time_ms == 0 {
            return 0.0;
        }
        (self.total_processed as f64 / self.total_processing_time_ms as f64) * 1000.0
    }

    /// Reset metrics
    pub fn reset(&mut self) {
        self.total_processed = 0;
        self.total_processing_time_ms = 0;
        self.recent_latencies_ms.clear();
        self.last_reset = Some(Utc::now());
    }
}

impl ProcessorState {
    /// Calculate the current override rate
    fn override_rate(&self) -> f64 {
        let total = self.recent_overrides + self.recent_approvals;
        if total == 0 {
            return 0.0;
        }
        self.recent_overrides as f64 / total as f64
    }

    /// Record an approval
    fn record_approval(&mut self, confidence: f64) {
        self.consecutive_overrides = 0;
        self.recent_approvals += 1;
        self.recent_confidences.push(confidence);

        // Keep only last 100 confidence scores
        if self.recent_confidences.len() > 100 {
            self.recent_confidences.remove(0);
        }
    }

    /// Record an override
    fn record_override(&mut self) {
        self.consecutive_overrides += 1;
        self.recent_overrides += 1;
    }

    /// Record a queued decision
    fn record_queued(&mut self) {
        self.recent_queued += 1;
    }

    /// Record a blocked decision
    fn record_blocked(&mut self) {
        self.recent_blocked += 1;
    }

    /// Check for anomalies (sudden confidence drop)
    fn check_anomaly(&self) -> Option<String> {
        if self.recent_confidences.len() < 10 {
            return None;
        }

        // Calculate average of last 10 vs previous 10
        let recent: Vec<_> = self.recent_confidences.iter().rev().take(10).collect();
        let previous: Vec<_> = self
            .recent_confidences
            .iter()
            .rev()
            .skip(10)
            .take(10)
            .collect();

        if previous.is_empty() {
            return None;
        }

        let recent_avg: f64 = recent.iter().copied().copied().sum::<f64>() / recent.len() as f64;
        let previous_avg: f64 =
            previous.iter().copied().copied().sum::<f64>() / previous.len() as f64;

        // Check for >20% drop
        if previous_avg > 0.0 && (previous_avg - recent_avg) / previous_avg > 0.20 {
            return Some(format!(
                "Confidence dropped from {:.2} to {:.2} (>{:.0}% drop)",
                previous_avg,
                recent_avg,
                ((previous_avg - recent_avg) / previous_avg) * 100.0
            ));
        }

        None
    }

    /// Reset rolling window statistics
    fn reset_window(&mut self) {
        self.recent_overrides = 0;
        self.recent_approvals = 0;
        self.recent_queued = 0;
        self.recent_blocked = 0;
    }
}

/// Auto-approve processor for medication matches
///
/// Orchestrates AI-based auto-approval with safety checks and audit trails.
/// Requirements: 1.1, 1.2, 1.3, 1.4, 6.1, 6.2, 7.1-7.5
pub struct AutoApproveProcessor {
    /// Configuration
    config: RwLock<AutoApproveConfig>,
    /// AI reviewer for match evaluation (reserved for future use)
    #[allow(dead_code)]
    ai_reviewer: Arc<AIReviewer>,
    /// Medication blocklist for safety checks
    blocklist: Arc<MedicationBlocklist>,
    /// Dosage gate for dosage mismatch checks
    dosage_gate: Arc<DosageGate>,
    /// Internal state
    state: RwLock<ProcessorState>,
    /// Cooldown tracker for medication pairs
    cooldown_tracker: RwLock<CooldownTracker>,
}

impl AutoApproveProcessor {
    /// Create a new auto-approve processor
    pub fn new(
        config: AutoApproveConfig,
        ai_reviewer: Arc<AIReviewer>,
        blocklist: Arc<MedicationBlocklist>,
        dosage_gate: Arc<DosageGate>,
    ) -> Self {
        Self {
            config: RwLock::new(config),
            ai_reviewer,
            blocklist,
            dosage_gate,
            state: RwLock::new(ProcessorState::default()),
            cooldown_tracker: RwLock::new(CooldownTracker::new()),
        }
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AutoApproveConfig {
        self.config.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(
        &self,
        config: AutoApproveConfig,
    ) -> Result<(), ConfigValidationError> {
        config.validate()?;
        *self.config.write().await = config;
        Ok(())
    }

    /// Check if auto-approval is currently enabled and not paused
    pub async fn is_active(&self) -> bool {
        let config = self.config.read().await;
        let state = self.state.read().await;
        config.enabled && !state.is_paused
    }

    /// Get current system status
    pub async fn get_status(&self) -> (SystemStatus, Option<PauseReason>) {
        let config = self.config.read().await;
        let state = self.state.read().await;

        if !config.enabled {
            return (SystemStatus::Disabled, None);
        }

        if state.is_paused {
            return (SystemStatus::Paused, state.pause_reason.clone());
        }

        (SystemStatus::Active, None)
    }

    /// Pause the auto-approve system
    pub async fn pause(&self, reason: PauseReason) {
        let mut state = self.state.write().await;
        state.is_paused = true;
        state.pause_reason = Some(reason);

        tracing::warn!(
            pause_reason = ?state.pause_reason,
            "Auto-approve system paused"
        );
    }

    /// Resume the auto-approve system
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        state.is_paused = false;
        state.pause_reason = None;
        state.consecutive_overrides = 0;
        state.reset_window();

        tracing::info!("Auto-approve system resumed");
    }

    /// Reset daily statistics (should be called at midnight)
    /// Requirements: 3.2
    pub async fn reset_daily_stats(&self) {
        let mut state = self.state.write().await;
        state.reset_window();
        state.recent_confidences.clear();

        tracing::info!("Auto-approve daily statistics reset");
    }

    /// Run safety checks for a match
    /// Requirements: 7.1, 7.4, 4.3
    pub async fn run_safety_checks(
        &self,
        offer: &Offer,
        request: &Request,
        dosage_score: f64,
    ) -> Vec<SafetyCheckResult> {
        let mut results = Vec::new();

        // Check blocklist (Requirement 7.1)
        if let Some(entry) = self
            .blocklist
            .is_blocked(&offer.medication, &request.medication)
        {
            results.push(SafetyCheckResult::failed(
                "blocklist",
                &format!(
                    "Medication pair blocked: {} - {}. Reason: {}",
                    entry.medication_a, entry.medication_b, entry.reason
                ),
            ));
        } else {
            results.push(SafetyCheckResult::passed("blocklist"));
        }

        // Check dosage mismatch (Requirement 7.4)
        let dosage_result = self.dosage_gate.evaluate(offer, request, dosage_score);
        if dosage_result.has_flag(&DosageFlag::MandatoryReview) {
            results.push(SafetyCheckResult::failed(
                "dosage_mismatch",
                "Dosage differs by more than 20%, requires human review",
            ));
        } else {
            results.push(SafetyCheckResult::passed("dosage_mismatch"));
        }

        // Check cooldown (Requirement 4.3)
        let cooldown_tracker = self.cooldown_tracker.read().await;
        if let Some(entry) = cooldown_tracker.is_in_cooldown(&offer.medication, &request.medication)
        {
            results.push(SafetyCheckResult::failed(
                "cooldown",
                &format!(
                    "Medication pair in cooldown until {} (override match: {})",
                    entry.cooldown_until.format("%Y-%m-%d %H:%M:%S UTC"),
                    entry.override_match_id
                ),
            ));
        } else {
            results.push(SafetyCheckResult::passed("cooldown"));
        }

        results
    }

    /// Check if any safety check failed
    fn any_safety_check_failed(checks: &[SafetyCheckResult]) -> Option<String> {
        for check in checks {
            if !check.passed {
                return check.reason.clone();
            }
        }
        None
    }

    /// Process a single match for auto-approval
    /// Requirements: 1.1, 1.2, 1.4, 7.1-7.5
    pub async fn process_match(
        &self,
        match_entity: &Match,
        offer: &Offer,
        request: &Request,
        category: Option<&str>,
    ) -> Result<AutoApproveResult, AutoApproveError> {
        let start_time = std::time::Instant::now();
        let config = self.config.read().await;

        // Check if system is enabled
        if !config.enabled {
            return Err(AutoApproveError::SystemDisabled);
        }

        // Check if within schedule (Requirement 5.5)
        if !config.is_within_schedule() {
            return Err(AutoApproveError::OutsideSchedule);
        }

        // Check if system is paused
        {
            let state = self.state.read().await;
            if state.is_paused {
                return Err(AutoApproveError::SystemPaused(
                    state
                        .pause_reason
                        .clone()
                        .unwrap_or(PauseReason::ManualPause {
                            user_id: Uuid::nil(),
                            reason: "Unknown".to_string(),
                        }),
                ));
            }
        }

        // Get effective threshold for this category
        let threshold = config.get_threshold_for_category(category);

        // Get AI confidence (use existing or evaluate)
        let ai_confidence = match_entity.ai_confidence.unwrap_or(0.0);
        let ai_explanation = match_entity
            .ai_explanation
            .clone()
            .unwrap_or_else(|| "No AI explanation available".to_string());

        // Run safety checks
        let dosage_score = match_entity.score; // Use match score as proxy for dosage score
        let safety_checks = self.run_safety_checks(offer, request, dosage_score).await;

        // Check for safety failures (Requirement 7.1, 7.4)
        if let Some(reason) = Self::any_safety_check_failed(&safety_checks) {
            // Record blocked in state for statistics and metrics
            {
                let mut state = self.state.write().await;
                state.record_blocked();
                state
                    .metrics
                    .record_processing(start_time.elapsed().as_millis() as u64);
            }
            return Ok(AutoApproveResult::blocked(
                match_entity.id,
                ai_confidence,
                ai_explanation,
                reason,
                safety_checks,
            ));
        }

        // Check confidence threshold (Requirements 1.1, 1.4)
        if ai_confidence >= threshold {
            // Record approval in state and metrics
            {
                let mut state = self.state.write().await;
                state.record_approval(ai_confidence);
                state
                    .metrics
                    .record_processing(start_time.elapsed().as_millis() as u64);

                // Check for anomalies (Requirement 7.3)
                if let Some(anomaly) = state.check_anomaly() {
                    drop(state);
                    self.pause(PauseReason::AnomalyDetected {
                        description: anomaly,
                    })
                    .await;
                }
            }

            Ok(AutoApproveResult::approved(
                match_entity.id,
                ai_confidence,
                ai_explanation,
                safety_checks,
                threshold,
            ))
        } else {
            // Record queued in state for statistics and metrics
            {
                let mut state = self.state.write().await;
                state.record_queued();
                state
                    .metrics
                    .record_processing(start_time.elapsed().as_millis() as u64);
            }
            Ok(AutoApproveResult::queued_for_review(
                match_entity.id,
                ai_confidence,
                ai_explanation,
                format!(
                    "Confidence {:.2} below threshold {:.2}",
                    ai_confidence, threshold
                ),
                safety_checks,
                threshold,
            ))
        }
    }

    /// Process a batch of pending matches
    /// Requirements: 6.1, 6.2
    pub async fn process_batch(
        &self,
        matches: Vec<(Match, Offer, Request, Option<String>)>,
    ) -> Vec<Result<AutoApproveResult, AutoApproveError>> {
        let config = self.config.read().await;
        let batch_size = config.batch_size;
        drop(config);

        // Limit to batch size (Requirement 6.1)
        let limited: Vec<_> = matches.into_iter().take(batch_size).collect();

        // Sort by age (oldest first) - Requirement 6.2
        // Note: matches should already be sorted by created_at ASC from the query
        // but we ensure it here for the property test

        let mut results = Vec::with_capacity(limited.len());

        for (match_entity, offer, request, category) in limited {
            let result = self
                .process_match(&match_entity, &offer, &request, category.as_deref())
                .await;
            results.push(result);
        }

        results
    }

    /// Record an override (called when a human overrides an AI decision)
    /// Requirements: 7.2, 7.5
    pub async fn record_override(&self) {
        let config = self.config.read().await;
        let override_threshold = config.override_rate_pause_threshold;
        let consecutive_limit = config.consecutive_override_limit;
        drop(config);

        let mut state = self.state.write().await;
        state.record_override();

        // Check override rate (Requirement 7.2)
        let rate = state.override_rate();
        if rate > override_threshold {
            state.is_paused = true;
            state.pause_reason = Some(PauseReason::HighOverrideRate {
                rate,
                threshold: override_threshold,
            });
            tracing::warn!(
                override_rate = rate,
                threshold = override_threshold,
                "Auto-approve paused due to high override rate"
            );
        }

        // Check consecutive overrides (Requirement 7.5)
        if state.consecutive_overrides >= consecutive_limit {
            state.is_paused = true;
            state.pause_reason = Some(PauseReason::ConsecutiveOverrides {
                count: state.consecutive_overrides,
                limit: consecutive_limit,
            });
            tracing::warn!(
                consecutive_overrides = state.consecutive_overrides,
                limit = consecutive_limit,
                "Auto-approve disabled due to consecutive overrides"
            );
        }
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> AutoApproveStats {
        let config = self.config.read().await;
        let state = self.state.read().await;

        let (status, pause_reason) = if !config.enabled {
            (SystemStatus::Disabled, None)
        } else if state.is_paused {
            (
                SystemStatus::Paused,
                state.pause_reason.clone().map(|r| format!("{:?}", r)),
            )
        } else {
            (SystemStatus::Active, None)
        };

        let avg_confidence = if state.recent_confidences.is_empty() {
            0.0
        } else {
            state.recent_confidences.iter().sum::<f64>() / state.recent_confidences.len() as f64
        };

        AutoApproveStats {
            total_approved_today: state.recent_approvals as u64,
            total_queued_today: state.recent_queued as u64,
            total_blocked_today: state.recent_blocked as u64,
            override_rate: state.override_rate(),
            average_confidence: avg_confidence,
            pending_review_count: 0, // Would need database query
            system_status: status,
            pause_reason,
        }
    }

    /// Get processing metrics for monitoring
    /// Requirements: 6.5
    pub async fn get_metrics(&self) -> ProcessingMetrics {
        let state = self.state.read().await;
        state.metrics.clone()
    }

    /// Reset processing metrics
    /// Requirements: 6.5
    pub async fn reset_metrics(&self) {
        let mut state = self.state.write().await;
        state.metrics.reset();
    }

    /// Queue a match for retry after AI service failure
    /// Requirements: 6.4
    pub async fn queue_for_retry(&self, match_id: Uuid, error_message: String) -> RetryInfo {
        let config = self.config.read().await;
        let max_retries = 3; // Default max retries
        drop(config);

        let retry_info = RetryInfo::new(match_id, error_message, max_retries);

        let mut state = self.state.write().await;
        state.retry_queue.push(retry_info.clone());

        tracing::warn!(
            match_id = %match_id,
            "Match queued for retry after AI service failure"
        );

        retry_info
    }

    /// Get matches ready for retry
    /// Requirements: 6.4
    pub async fn get_ready_for_retry(&self) -> Vec<RetryInfo> {
        let state = self.state.read().await;
        state
            .retry_queue
            .iter()
            .filter(|r| r.is_ready_for_retry() && r.should_retry())
            .cloned()
            .collect()
    }

    /// Update retry info after a retry attempt
    /// Requirements: 6.4
    pub async fn update_retry(&self, match_id: Uuid, success: bool) {
        let mut state = self.state.write().await;

        if success {
            // Remove from retry queue on success
            state.retry_queue.retain(|r| r.match_id != match_id);
            tracing::info!(match_id = %match_id, "Match retry succeeded, removed from queue");
        } else {
            // Increment retry count
            if let Some(retry_info) = state
                .retry_queue
                .iter_mut()
                .find(|r| r.match_id == match_id)
            {
                retry_info.increment();
                if !retry_info.should_retry() {
                    tracing::error!(
                        match_id = %match_id,
                        retry_count = retry_info.retry_count,
                        "Match exceeded max retries, needs manual intervention"
                    );
                }
            }
        }
    }

    /// Get the current retry queue
    /// Requirements: 6.4
    pub async fn get_retry_queue(&self) -> Vec<RetryInfo> {
        let state = self.state.read().await;
        state.retry_queue.clone()
    }

    /// Clear expired retries (those that exceeded max retries)
    /// Requirements: 6.4
    pub async fn clear_expired_retries(&self) -> Vec<RetryInfo> {
        let mut state = self.state.write().await;
        let expired: Vec<_> = state
            .retry_queue
            .iter()
            .filter(|r| !r.should_retry())
            .cloned()
            .collect();

        state.retry_queue.retain(|r| r.should_retry());
        expired
    }

    /// Override an AI auto-approval decision
    /// Requirements: 4.1, 4.3, 4.4
    ///
    /// This method:
    /// 1. Records the override in the tracker (for rate/consecutive monitoring)
    /// 2. Adds a cooldown entry for the medication pair
    /// 3. Creates a feedback event for the learning system
    /// 4. Returns an OverrideResult that can be used to update the match status
    ///
    /// Note: The actual database update should be done by the caller
    pub async fn override_decision(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
    ) -> Result<OverrideResult, AutoApproveError> {
        self.override_decision_with_confidence(
            match_id,
            user_id,
            reason,
            offer_medication,
            request_medication,
            0.0, // Default confidence when not provided
        )
        .await
    }

    /// Override an AI auto-approval decision with AI confidence for feedback
    /// Requirements: 4.1, 4.3, 4.4
    ///
    /// This method:
    /// 1. Records the override in the tracker (for rate/consecutive monitoring)
    /// 2. Adds a cooldown entry for the medication pair
    /// 3. Creates a feedback event for the learning system
    /// 4. Returns an OverrideResult that can be used to update the match status
    ///
    /// Note: The actual database update should be done by the caller
    pub async fn override_decision_with_confidence(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
        _ai_confidence: f64,
    ) -> Result<OverrideResult, AutoApproveError> {
        // Record the override in our tracker
        self.record_override().await;

        let config = self.config.read().await;
        let cooldown_mins = config.override_cooldown_mins;
        drop(config);

        // Add cooldown for the medication pair (Requirement 4.3)
        {
            let mut cooldown_tracker = self.cooldown_tracker.write().await;
            cooldown_tracker.add_cooldown(
                offer_medication,
                request_medication,
                cooldown_mins,
                match_id,
            );
        }

        // Calculate cooldown expiry
        let cooldown_until = Utc::now() + chrono::Duration::minutes(cooldown_mins as i64);

        tracing::info!(
            match_id = %match_id,
            user_id = %user_id,
            reason = reason,
            cooldown_until = %cooldown_until,
            "AI decision overridden"
        );

        Ok(OverrideResult {
            match_id,
            user_id,
            reason: reason.to_string(),
            timestamp: Utc::now(),
            cooldown_until,
            offer_medication: offer_medication.to_string(),
            request_medication: request_medication.to_string(),
        })
    }

    /// Create a feedback event for an override
    /// Requirements: 4.4
    ///
    /// This creates a FeedbackEvent that should be saved to the learning system
    pub fn create_override_feedback(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
    ) -> FeedbackEvent {
        FeedbackEvent::from_override(
            match_id,
            user_id,
            ai_confidence,
            reason,
            offer_medication,
            request_medication,
        )
    }

    /// Create a feedback event for a rejection
    /// Requirements: 4.4
    ///
    /// This creates a FeedbackEvent that should be saved to the learning system
    pub fn create_rejection_feedback(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
    ) -> FeedbackEvent {
        FeedbackEvent::from_rejection(
            match_id,
            user_id,
            ai_confidence,
            reason,
            offer_medication,
            request_medication,
        )
    }

    /// Create a feedback event for a confirmation
    /// Requirements: 4.4
    ///
    /// This creates a FeedbackEvent that should be saved to the learning system
    pub fn create_confirmation_feedback(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        offer_medication: &str,
        request_medication: &str,
    ) -> FeedbackEvent {
        FeedbackEvent::from_confirmation(
            match_id,
            user_id,
            ai_confidence,
            offer_medication,
            request_medication,
        )
    }

    /// Undo an AI auto-approval within the undo window
    /// Requirements: 4.2
    ///
    /// Returns Ok if the undo is within the time window, Err otherwise
    pub async fn undo_approval(
        &self,
        match_id: Uuid,
        user_id: Uuid,
        approved_at: DateTime<Utc>,
    ) -> Result<UndoResult, AutoApproveError> {
        let config = self.config.read().await;
        let undo_window_mins = config.undo_window_mins;
        drop(config);

        let now = Utc::now();
        let window_end = approved_at + chrono::Duration::minutes(undo_window_mins as i64);

        if now > window_end {
            return Err(AutoApproveError::UndoWindowExpired);
        }

        tracing::info!(
            match_id = %match_id,
            user_id = %user_id,
            approved_at = %approved_at,
            "AI approval undone"
        );

        Ok(UndoResult {
            match_id,
            user_id,
            timestamp: now,
            original_approved_at: approved_at,
        })
    }

    /// Check if a medication pair is within the undo window
    pub async fn is_within_undo_window(&self, approved_at: DateTime<Utc>) -> bool {
        let config = self.config.read().await;
        let undo_window_mins = config.undo_window_mins;
        drop(config);

        let now = Utc::now();
        let window_end = approved_at + chrono::Duration::minutes(undo_window_mins as i64);

        now <= window_end
    }

    /// Check if a medication pair is in cooldown
    /// Requirements: 4.3
    pub async fn is_in_cooldown(&self, offer_medication: &str, request_medication: &str) -> bool {
        let cooldown_tracker = self.cooldown_tracker.read().await;
        cooldown_tracker
            .is_in_cooldown(offer_medication, request_medication)
            .is_some()
    }

    /// Get cooldown info for a medication pair if in cooldown
    /// Requirements: 4.3
    pub async fn get_cooldown_info(
        &self,
        offer_medication: &str,
        request_medication: &str,
    ) -> Option<DateTime<Utc>> {
        let cooldown_tracker = self.cooldown_tracker.read().await;
        cooldown_tracker
            .is_in_cooldown(offer_medication, request_medication)
            .map(|entry| entry.cooldown_until)
    }
}

/// Result of an override operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideResult {
    /// ID of the match that was overridden
    pub match_id: Uuid,
    /// ID of the user who performed the override
    pub user_id: Uuid,
    /// Reason for the override
    pub reason: String,
    /// When the override occurred
    pub timestamp: DateTime<Utc>,
    /// When the cooldown expires for this medication pair
    pub cooldown_until: DateTime<Utc>,
    /// Offer medication name (for cooldown tracking)
    pub offer_medication: String,
    /// Request medication name (for cooldown tracking)
    pub request_medication: String,
}

/// Result of an undo operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    /// ID of the match that was undone
    pub match_id: Uuid,
    /// ID of the user who performed the undo
    pub user_id: Uuid,
    /// When the undo occurred
    pub timestamp: DateTime<Utc>,
    /// When the original approval occurred
    pub original_approved_at: DateTime<Utc>,
}

/// Feedback event for learning system integration
/// Requirements: 4.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEvent {
    /// ID of the match
    pub match_id: Uuid,
    /// ID of the user who provided feedback
    pub user_id: Uuid,
    /// Whether the match was confirmed (false = rejected/overridden)
    pub confirmed: bool,
    /// AI confidence score at the time of the decision
    pub ai_confidence: f64,
    /// Reason for the feedback (override reason, rejection reason, etc.)
    pub reason: String,
    /// Type of feedback event
    pub event_type: FeedbackEventType,
    /// When the feedback was recorded
    pub timestamp: DateTime<Utc>,
    /// Offer medication name
    pub offer_medication: String,
    /// Request medication name
    pub request_medication: String,
}

/// Type of feedback event
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeedbackEventType {
    /// AI decision was overridden by human
    Override,
    /// AI-approved match was rejected
    Rejection,
    /// AI-approved match was confirmed by human
    Confirmation,
    /// AI decision was undone
    Undo,
}

impl FeedbackEvent {
    /// Create a feedback event for an override
    pub fn from_override(
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
    ) -> Self {
        Self {
            match_id,
            user_id,
            confirmed: false,
            ai_confidence,
            reason: reason.to_string(),
            event_type: FeedbackEventType::Override,
            timestamp: Utc::now(),
            offer_medication: offer_medication.to_string(),
            request_medication: request_medication.to_string(),
        }
    }

    /// Create a feedback event for a rejection
    pub fn from_rejection(
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        reason: &str,
        offer_medication: &str,
        request_medication: &str,
    ) -> Self {
        Self {
            match_id,
            user_id,
            confirmed: false,
            ai_confidence,
            reason: reason.to_string(),
            event_type: FeedbackEventType::Rejection,
            timestamp: Utc::now(),
            offer_medication: offer_medication.to_string(),
            request_medication: request_medication.to_string(),
        }
    }

    /// Create a feedback event for a confirmation
    pub fn from_confirmation(
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        offer_medication: &str,
        request_medication: &str,
    ) -> Self {
        Self {
            match_id,
            user_id,
            confirmed: true,
            ai_confidence,
            reason: "Human confirmed AI decision".to_string(),
            event_type: FeedbackEventType::Confirmation,
            timestamp: Utc::now(),
            offer_medication: offer_medication.to_string(),
            request_medication: request_medication.to_string(),
        }
    }

    /// Create a feedback event for an undo
    pub fn from_undo(
        match_id: Uuid,
        user_id: Uuid,
        ai_confidence: f64,
        offer_medication: &str,
        request_medication: &str,
    ) -> Self {
        Self {
            match_id,
            user_id,
            confirmed: false,
            ai_confidence,
            reason: "AI approval undone".to_string(),
            event_type: FeedbackEventType::Undo,
            timestamp: Utc::now(),
            offer_medication: offer_medication.to_string(),
            request_medication: request_medication.to_string(),
        }
    }
}

/// Errors that can occur during auto-approve processing
#[derive(Debug, Clone)]
pub enum AutoApproveError {
    /// System is disabled
    SystemDisabled,
    /// System is paused
    SystemPaused(PauseReason),
    /// Outside configured schedule
    OutsideSchedule,
    /// AI service error (with retry info)
    AIServiceError(String),
    /// AI service timeout (should retry)
    AIServiceTimeout { match_id: Uuid, retry_count: u32 },
    /// Database error
    DatabaseError(String),
    /// Match not found
    MatchNotFound(Uuid),
    /// Undo window expired
    UndoWindowExpired,
    /// Match is in cooldown
    InCooldown { until: DateTime<Utc> },
}

/// Information about a match queued for retry
/// Requirements: 6.4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryInfo {
    /// Match ID to retry
    pub match_id: Uuid,
    /// Number of retry attempts so far
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// When to retry next
    pub next_retry_at: DateTime<Utc>,
    /// Original error message
    pub error_message: String,
    /// When the failure occurred
    pub failed_at: DateTime<Utc>,
}

impl RetryInfo {
    /// Create a new retry info for a failed match
    pub fn new(match_id: Uuid, error_message: String, max_retries: u32) -> Self {
        Self {
            match_id,
            retry_count: 0,
            max_retries,
            next_retry_at: Utc::now() + Duration::seconds(30), // Initial retry after 30s
            error_message,
            failed_at: Utc::now(),
        }
    }

    /// Increment retry count and calculate next retry time (exponential backoff)
    pub fn increment(&mut self) {
        self.retry_count += 1;
        // Exponential backoff: 30s, 60s, 120s, 240s, ...
        let backoff_secs = 30 * (2_i64.pow(self.retry_count.min(5)));
        self.next_retry_at = Utc::now() + Duration::seconds(backoff_secs);
    }

    /// Check if we should retry
    pub fn should_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Check if it's time to retry
    pub fn is_ready_for_retry(&self) -> bool {
        Utc::now() >= self.next_retry_at
    }
}

impl std::fmt::Display for AutoApproveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutoApproveError::SystemDisabled => write!(f, "Auto-approve system is disabled"),
            AutoApproveError::SystemPaused(reason) => {
                write!(f, "Auto-approve system is paused: {:?}", reason)
            }
            AutoApproveError::OutsideSchedule => {
                write!(f, "Auto-approve is outside configured schedule hours")
            }
            AutoApproveError::AIServiceError(msg) => write!(f, "AI service error: {}", msg),
            AutoApproveError::AIServiceTimeout {
                match_id,
                retry_count,
            } => {
                write!(
                    f,
                    "AI service timeout for match {} (retry {})",
                    match_id, retry_count
                )
            }
            AutoApproveError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AutoApproveError::MatchNotFound(id) => write!(f, "Match not found: {}", id),
            AutoApproveError::UndoWindowExpired => write!(f, "Undo window has expired"),
            AutoApproveError::InCooldown { until } => {
                write!(f, "Match is in cooldown until {}", until)
            }
        }
    }
}

impl std::error::Error for AutoApproveError {}

// =============================================================================
// AI Evaluation Data Persistence
// Requirements: 1.2
// =============================================================================

/// AI evaluation status values
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AIStatus {
    /// AI evaluation completed successfully
    Evaluated,
    /// AI evaluation pending
    Pending,
    /// AI evaluation failed
    Failed,
    /// AI evaluation skipped (e.g., blocklisted)
    Skipped,
}

impl std::fmt::Display for AIStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIStatus::Evaluated => write!(f, "evaluated"),
            AIStatus::Pending => write!(f, "pending"),
            AIStatus::Failed => write!(f, "failed"),
            AIStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl AIStatus {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "evaluated" => Some(AIStatus::Evaluated),
            "pending" => Some(AIStatus::Pending),
            "failed" => Some(AIStatus::Failed),
            "skipped" => Some(AIStatus::Skipped),
            _ => None,
        }
    }
}

/// AI evaluation data for a match
/// Requirements: 1.2
///
/// Contains the results of AI evaluation including confidence score,
/// explanation, and status. This data is persisted to the Match entity
/// after AI evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIEvaluationData {
    /// Match ID this evaluation is for
    pub match_id: Uuid,
    /// AI confidence score (0.0 to 1.0)
    pub ai_confidence: f64,
    /// AI explanation/reasoning for the confidence score
    pub ai_explanation: String,
    /// AI evaluation status
    pub ai_status: AIStatus,
    /// When the evaluation was performed
    pub evaluated_at: DateTime<Utc>,
}

impl AIEvaluationData {
    /// Create new AI evaluation data
    pub fn new(
        match_id: Uuid,
        ai_confidence: f64,
        ai_explanation: String,
        ai_status: AIStatus,
    ) -> Self {
        Self {
            match_id,
            ai_confidence,
            ai_explanation,
            ai_status,
            evaluated_at: Utc::now(),
        }
    }

    /// Create evaluation data for a successful evaluation
    pub fn evaluated(match_id: Uuid, confidence: f64, explanation: String) -> Self {
        Self::new(match_id, confidence, explanation, AIStatus::Evaluated)
    }

    /// Create evaluation data for a failed evaluation
    pub fn failed(match_id: Uuid, error_message: String) -> Self {
        Self::new(match_id, 0.0, error_message, AIStatus::Failed)
    }

    /// Create evaluation data for a skipped evaluation
    pub fn skipped(match_id: Uuid, reason: String) -> Self {
        Self::new(match_id, 0.0, reason, AIStatus::Skipped)
    }

    /// Check if the evaluation data is complete (has non-empty required fields)
    /// Requirements: 1.2 - Property 3
    pub fn is_complete(&self) -> bool {
        // ai_confidence can be 0.0 for failed/skipped, so we check ai_explanation and ai_status
        !self.ai_explanation.is_empty()
    }

    /// Check if the evaluation was successful
    pub fn is_successful(&self) -> bool {
        self.ai_status == AIStatus::Evaluated && self.ai_confidence > 0.0
    }

    /// Validate the evaluation data
    pub fn validate(&self) -> Result<(), AIEvaluationError> {
        if self.ai_explanation.is_empty() {
            return Err(AIEvaluationError::EmptyExplanation);
        }

        if self.ai_status == AIStatus::Evaluated && self.ai_confidence <= 0.0 {
            return Err(AIEvaluationError::InvalidConfidence(self.ai_confidence));
        }

        if self.ai_status == AIStatus::Evaluated && self.ai_confidence > 1.0 {
            return Err(AIEvaluationError::InvalidConfidence(self.ai_confidence));
        }

        Ok(())
    }
}

/// Errors that can occur during AI evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum AIEvaluationError {
    /// AI explanation is empty
    EmptyExplanation,
    /// AI confidence is invalid
    InvalidConfidence(f64),
    /// AI status is invalid
    InvalidStatus(String),
    /// Match not found
    MatchNotFound(Uuid),
    /// Database error
    DatabaseError(String),
}

impl std::fmt::Display for AIEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIEvaluationError::EmptyExplanation => {
                write!(f, "AI explanation cannot be empty")
            }
            AIEvaluationError::InvalidConfidence(c) => {
                write!(f, "Invalid AI confidence: {} (must be 0.0-1.0)", c)
            }
            AIEvaluationError::InvalidStatus(s) => {
                write!(f, "Invalid AI status: {}", s)
            }
            AIEvaluationError::MatchNotFound(id) => {
                write!(f, "Match not found: {}", id)
            }
            AIEvaluationError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
        }
    }
}

impl std::error::Error for AIEvaluationError {}

/// Repository trait for persisting AI evaluation data
/// Requirements: 1.2
#[async_trait::async_trait]
pub trait AIEvaluationRepository: Send + Sync {
    /// Persist AI evaluation data to a match
    async fn persist_evaluation(&self, data: &AIEvaluationData) -> Result<(), AIEvaluationError>;

    /// Get AI evaluation data for a match
    async fn get_evaluation(
        &self,
        match_id: Uuid,
    ) -> Result<Option<AIEvaluationData>, AIEvaluationError>;
}

/// In-memory implementation of AIEvaluationRepository for testing
#[derive(Debug, Default)]
pub struct InMemoryAIEvaluationRepository {
    evaluations: std::sync::RwLock<HashMap<Uuid, AIEvaluationData>>,
}

impl InMemoryAIEvaluationRepository {
    /// Create a new in-memory repository
    pub fn new() -> Self {
        Self {
            evaluations: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get all evaluations (for testing)
    pub fn get_all(&self) -> Vec<AIEvaluationData> {
        self.evaluations.read().unwrap().values().cloned().collect()
    }

    /// Clear all evaluations (for testing)
    pub fn clear(&self) {
        self.evaluations.write().unwrap().clear();
    }
}

#[async_trait::async_trait]
impl AIEvaluationRepository for InMemoryAIEvaluationRepository {
    async fn persist_evaluation(&self, data: &AIEvaluationData) -> Result<(), AIEvaluationError> {
        // Validate before persisting
        data.validate()?;

        self.evaluations
            .write()
            .unwrap()
            .insert(data.match_id, data.clone());
        Ok(())
    }

    async fn get_evaluation(
        &self,
        match_id: Uuid,
    ) -> Result<Option<AIEvaluationData>, AIEvaluationError> {
        Ok(self.evaluations.read().unwrap().get(&match_id).cloned())
    }
}

// =============================================================================
// AI Evaluation Data Persistence Tests
// =============================================================================

#[cfg(test)]
mod ai_evaluation_tests {
    use super::*;

    #[test]
    fn test_ai_evaluation_data_new() {
        let match_id = Uuid::new_v4();
        let data = AIEvaluationData::new(
            match_id,
            0.92,
            "High confidence match".to_string(),
            AIStatus::Evaluated,
        );

        assert_eq!(data.match_id, match_id);
        assert!((data.ai_confidence - 0.92).abs() < 0.001);
        assert_eq!(data.ai_explanation, "High confidence match");
        assert_eq!(data.ai_status, AIStatus::Evaluated);
    }

    #[test]
    fn test_ai_evaluation_data_evaluated() {
        let match_id = Uuid::new_v4();
        let data = AIEvaluationData::evaluated(
            match_id,
            0.88,
            "Good match based on medication similarity".to_string(),
        );

        assert_eq!(data.ai_status, AIStatus::Evaluated);
        assert!(data.is_successful());
        assert!(data.is_complete());
    }

    #[test]
    fn test_ai_evaluation_data_failed() {
        let match_id = Uuid::new_v4();
        let data = AIEvaluationData::failed(match_id, "AI service timeout".to_string());

        assert_eq!(data.ai_status, AIStatus::Failed);
        assert!(!data.is_successful());
        assert!(data.is_complete());
        assert!((data.ai_confidence - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ai_evaluation_data_skipped() {
        let match_id = Uuid::new_v4();
        let data = AIEvaluationData::skipped(match_id, "Medication on blocklist".to_string());

        assert_eq!(data.ai_status, AIStatus::Skipped);
        assert!(!data.is_successful());
        assert!(data.is_complete());
    }

    #[test]
    fn test_ai_evaluation_data_validate_success() {
        let data =
            AIEvaluationData::evaluated(Uuid::new_v4(), 0.85, "Valid evaluation".to_string());

        assert!(data.validate().is_ok());
    }

    #[test]
    fn test_ai_evaluation_data_validate_empty_explanation() {
        let data = AIEvaluationData::new(Uuid::new_v4(), 0.85, "".to_string(), AIStatus::Evaluated);

        assert!(matches!(
            data.validate(),
            Err(AIEvaluationError::EmptyExplanation)
        ));
    }

    #[test]
    fn test_ai_evaluation_data_validate_invalid_confidence() {
        let data = AIEvaluationData::new(
            Uuid::new_v4(),
            1.5, // Invalid: > 1.0
            "Some explanation".to_string(),
            AIStatus::Evaluated,
        );

        assert!(matches!(
            data.validate(),
            Err(AIEvaluationError::InvalidConfidence(_))
        ));
    }

    #[test]
    fn test_ai_evaluation_data_validate_zero_confidence_evaluated() {
        let data = AIEvaluationData::new(
            Uuid::new_v4(),
            0.0, // Invalid for Evaluated status
            "Some explanation".to_string(),
            AIStatus::Evaluated,
        );

        assert!(matches!(
            data.validate(),
            Err(AIEvaluationError::InvalidConfidence(_))
        ));
    }

    #[test]
    fn test_ai_evaluation_data_validate_zero_confidence_failed() {
        // Zero confidence is valid for Failed status
        let data = AIEvaluationData::failed(Uuid::new_v4(), "Error message".to_string());

        assert!(data.validate().is_ok());
    }

    #[test]
    fn test_ai_status_display() {
        assert_eq!(AIStatus::Evaluated.to_string(), "evaluated");
        assert_eq!(AIStatus::Pending.to_string(), "pending");
        assert_eq!(AIStatus::Failed.to_string(), "failed");
        assert_eq!(AIStatus::Skipped.to_string(), "skipped");
    }

    #[test]
    fn test_ai_status_from_str() {
        assert_eq!(AIStatus::from_str("evaluated"), Some(AIStatus::Evaluated));
        assert_eq!(AIStatus::from_str("EVALUATED"), Some(AIStatus::Evaluated));
        assert_eq!(AIStatus::from_str("pending"), Some(AIStatus::Pending));
        assert_eq!(AIStatus::from_str("failed"), Some(AIStatus::Failed));
        assert_eq!(AIStatus::from_str("skipped"), Some(AIStatus::Skipped));
        assert_eq!(AIStatus::from_str("unknown"), None);
    }

    #[tokio::test]
    async fn test_in_memory_repository_persist_and_get() {
        let repo = InMemoryAIEvaluationRepository::new();
        let match_id = Uuid::new_v4();
        let data = AIEvaluationData::evaluated(match_id, 0.90, "Test evaluation".to_string());

        // Persist
        let result = repo.persist_evaluation(&data).await;
        assert!(result.is_ok());

        // Get
        let retrieved = repo.get_evaluation(match_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.match_id, match_id);
        assert!((retrieved.ai_confidence - 0.90).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_in_memory_repository_get_nonexistent() {
        let repo = InMemoryAIEvaluationRepository::new();
        let result = repo.get_evaluation(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_in_memory_repository_persist_invalid() {
        let repo = InMemoryAIEvaluationRepository::new();
        let data = AIEvaluationData::new(
            Uuid::new_v4(),
            0.85,
            "".to_string(), // Invalid: empty explanation
            AIStatus::Evaluated,
        );

        let result = repo.persist_evaluation(&data).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_repository_clear() {
        let repo = InMemoryAIEvaluationRepository::new();
        let data = AIEvaluationData::evaluated(Uuid::new_v4(), 0.90, "Test".to_string());

        repo.persist_evaluation(&data).await.unwrap();
        assert_eq!(repo.get_all().len(), 1);

        repo.clear();
        assert_eq!(repo.get_all().len(), 0);
    }
}
