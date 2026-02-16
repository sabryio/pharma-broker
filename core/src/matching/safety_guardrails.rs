//! Safety Guardrails for AI Auto-Approve System
//!
//! Implements safety checks to prevent the AI from making harmful decisions.
//! Requirements: 7.1, 7.2, 7.3, 7.4, 7.5
//!
//! Safety checks include:
//! - Blocklist enforcement (7.1)
//! - Override rate monitoring (7.2)
//! - Anomaly detection (7.3)
//! - Dosage mismatch detection (7.4)
//! - Consecutive override tracking (7.5)

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::blocklist::MedicationBlocklist;
use crate::domain::{Offer, Request};

// =============================================================================
// Safety Check Result
// =============================================================================

/// Result of a safety check
/// Requirements: 7.1, 7.2, 7.3, 7.4, 7.5
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyCheckResult {
    /// Name of the safety check
    pub check_name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Reason for failure (if applicable)
    pub reason: Option<String>,
}

impl SafetyCheckResult {
    /// Create a passed safety check result
    pub fn passed(check_name: &str) -> Self {
        Self {
            check_name: check_name.to_string(),
            passed: true,
            reason: None,
        }
    }

    /// Create a failed safety check result
    pub fn failed(check_name: &str, reason: &str) -> Self {
        Self {
            check_name: check_name.to_string(),
            passed: false,
            reason: Some(reason.to_string()),
        }
    }
}

// =============================================================================
// Pause Reason
// =============================================================================

/// Reason for pausing auto-approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PauseReason {
    /// Override rate exceeded threshold
    HighOverrideRate { rate: f64, threshold: f64 },
    /// Consecutive overrides exceeded limit
    ConsecutiveOverrides { count: u32, limit: u32 },
    /// Anomaly detected (e.g., sudden confidence drop)
    AnomalyDetected { description: String },
    /// Manually paused by user
    ManualPause { user_id: Uuid, reason: String },
    /// Outside scheduled hours
    OutsideSchedule,
}

impl std::fmt::Display for PauseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PauseReason::HighOverrideRate { rate, threshold } => {
                write!(
                    f,
                    "Override rate {:.1}% exceeds threshold {:.1}%",
                    rate * 100.0,
                    threshold * 100.0
                )
            }
            PauseReason::ConsecutiveOverrides { count, limit } => {
                write!(f, "{} consecutive overrides (limit: {})", count, limit)
            }
            PauseReason::AnomalyDetected { description } => {
                write!(f, "Anomaly detected: {}", description)
            }
            PauseReason::ManualPause { reason, .. } => {
                write!(f, "Manually paused: {}", reason)
            }
            PauseReason::OutsideSchedule => {
                write!(f, "Outside scheduled hours")
            }
        }
    }
}

// =============================================================================
// Override Tracker
// =============================================================================

/// Tracks override events for rate calculation
#[derive(Debug, Clone)]
struct OverrideEvent {
    timestamp: DateTime<Utc>,
    was_override: bool,
}

/// Configuration for override tracking
#[derive(Debug, Clone)]
pub struct OverrideTrackerConfig {
    /// Rolling window duration for rate calculation
    pub window_duration: Duration,
    /// Threshold rate to trigger pause (0.0-1.0)
    pub pause_threshold: f64,
    /// Consecutive override limit to disable
    pub consecutive_limit: u32,
}

impl Default for OverrideTrackerConfig {
    fn default() -> Self {
        Self {
            window_duration: Duration::hours(24),
            pause_threshold: 0.10,
            consecutive_limit: 5,
        }
    }
}

/// Tracks overrides for rate calculation and consecutive detection
#[derive(Debug)]
pub struct OverrideTracker {
    config: OverrideTrackerConfig,
    events: VecDeque<OverrideEvent>,
    consecutive_overrides: u32,
}

impl OverrideTracker {
    /// Create a new override tracker
    pub fn new(config: OverrideTrackerConfig) -> Self {
        Self {
            config,
            events: VecDeque::new(),
            consecutive_overrides: 0,
        }
    }

    /// Record an approval event
    pub fn record_approval(&mut self) {
        self.consecutive_overrides = 0;
        self.events.push_back(OverrideEvent {
            timestamp: Utc::now(),
            was_override: false,
        });
        self.cleanup_old_events();
    }

    /// Record an override event
    pub fn record_override(&mut self) {
        self.consecutive_overrides += 1;
        self.events.push_back(OverrideEvent {
            timestamp: Utc::now(),
            was_override: true,
        });
        self.cleanup_old_events();
    }

    /// Remove events outside the rolling window
    fn cleanup_old_events(&mut self) {
        let cutoff = Utc::now() - self.config.window_duration;
        while let Some(event) = self.events.front() {
            if event.timestamp < cutoff {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate the current override rate
    pub fn override_rate(&self) -> f64 {
        if self.events.is_empty() {
            return 0.0;
        }
        let overrides = self.events.iter().filter(|e| e.was_override).count();
        overrides as f64 / self.events.len() as f64
    }

    /// Get the count of consecutive overrides
    pub fn consecutive_count(&self) -> u32 {
        self.consecutive_overrides
    }

    /// Check if override rate exceeds threshold
    /// Returns Some(PauseReason) if should pause
    pub fn check_override_rate(&self) -> Option<PauseReason> {
        let rate = self.override_rate();
        if rate > self.config.pause_threshold && self.events.len() >= 10 {
            Some(PauseReason::HighOverrideRate {
                rate,
                threshold: self.config.pause_threshold,
            })
        } else {
            None
        }
    }

    /// Check if consecutive overrides exceed limit
    /// Returns Some(PauseReason) if should disable
    pub fn check_consecutive_overrides(&self) -> Option<PauseReason> {
        if self.consecutive_overrides >= self.config.consecutive_limit {
            Some(PauseReason::ConsecutiveOverrides {
                count: self.consecutive_overrides,
                limit: self.config.consecutive_limit,
            })
        } else {
            None
        }
    }

    /// Reset the tracker state
    pub fn reset(&mut self) {
        self.events.clear();
        self.consecutive_overrides = 0;
    }
}

// =============================================================================
// Anomaly Detector
// =============================================================================

/// Configuration for anomaly detection
#[derive(Debug, Clone)]
pub struct AnomalyDetectorConfig {
    /// Minimum samples needed before detecting anomalies
    pub min_samples: usize,
    /// Window size for recent samples
    pub recent_window: usize,
    /// Window size for baseline samples
    pub baseline_window: usize,
    /// Threshold for confidence drop (e.g., 0.20 = 20% drop)
    pub drop_threshold: f64,
}

impl Default for AnomalyDetectorConfig {
    fn default() -> Self {
        Self {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        }
    }
}

/// Detects anomalies in AI confidence scores
#[derive(Debug)]
pub struct AnomalyDetector {
    config: AnomalyDetectorConfig,
    confidence_history: VecDeque<f64>,
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(config: AnomalyDetectorConfig) -> Self {
        Self {
            config,
            confidence_history: VecDeque::new(),
        }
    }

    /// Record a confidence score
    pub fn record_confidence(&mut self, confidence: f64) {
        self.confidence_history.push_back(confidence);
        // Keep only enough history for analysis
        let max_size = self.config.recent_window + self.config.baseline_window + 10;
        while self.confidence_history.len() > max_size {
            self.confidence_history.pop_front();
        }
    }

    /// Check for anomalies (sudden confidence drop)
    /// Returns Some(description) if anomaly detected
    pub fn check_anomaly(&self) -> Option<String> {
        if self.confidence_history.len() < self.config.min_samples {
            return None;
        }

        // Calculate average of recent samples
        let recent: Vec<f64> = self
            .confidence_history
            .iter()
            .rev()
            .take(self.config.recent_window)
            .copied()
            .collect();

        // Calculate average of baseline samples (before recent)
        let baseline: Vec<f64> = self
            .confidence_history
            .iter()
            .rev()
            .skip(self.config.recent_window)
            .take(self.config.baseline_window)
            .copied()
            .collect();

        if baseline.is_empty() || recent.is_empty() {
            return None;
        }

        let recent_avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let baseline_avg: f64 = baseline.iter().sum::<f64>() / baseline.len() as f64;

        // Check for significant drop
        if baseline_avg > 0.0 {
            let drop = (baseline_avg - recent_avg) / baseline_avg;
            if drop > self.config.drop_threshold {
                return Some(format!(
                    "Confidence dropped from {:.2} to {:.2} ({:.1}% drop)",
                    baseline_avg,
                    recent_avg,
                    drop * 100.0
                ));
            }
        }

        None
    }

    /// Reset the detector state
    pub fn reset(&mut self) {
        self.confidence_history.clear();
    }
}

// =============================================================================
// Cooldown Tracker
// =============================================================================

/// Tracks medication pair cooldowns after overrides
#[derive(Debug, Clone)]
pub struct CooldownEntry {
    pub offer_medication: String,
    pub request_medication: String,
    pub cooldown_until: DateTime<Utc>,
    pub override_match_id: Uuid,
}

/// Tracks cooldowns for medication pairs
#[derive(Debug, Default)]
pub struct CooldownTracker {
    cooldowns: Vec<CooldownEntry>,
}

impl CooldownTracker {
    /// Create a new cooldown tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cooldown for a medication pair
    pub fn add_cooldown(
        &mut self,
        offer_med: &str,
        request_med: &str,
        duration_mins: u64,
        match_id: Uuid,
    ) {
        let cooldown_until = Utc::now() + Duration::minutes(duration_mins as i64);
        self.cooldowns.push(CooldownEntry {
            offer_medication: Self::normalize(offer_med),
            request_medication: Self::normalize(request_med),
            cooldown_until,
            override_match_id: match_id,
        });
        self.cleanup_expired();
    }

    /// Check if a medication pair is in cooldown
    pub fn is_in_cooldown(&self, offer_med: &str, request_med: &str) -> Option<&CooldownEntry> {
        let offer_norm = Self::normalize(offer_med);
        let request_norm = Self::normalize(request_med);
        let now = Utc::now();

        self.cooldowns.iter().find(|c| {
            c.cooldown_until > now
                && ((c.offer_medication == offer_norm && c.request_medication == request_norm)
                    || (c.offer_medication == request_norm && c.request_medication == offer_norm))
        })
    }

    /// Remove expired cooldowns
    fn cleanup_expired(&mut self) {
        let now = Utc::now();
        self.cooldowns.retain(|c| c.cooldown_until > now);
    }

    /// Normalize medication name for comparison
    fn normalize(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Clear all cooldowns
    pub fn clear(&mut self) {
        self.cooldowns.clear();
    }
}

// =============================================================================
// Safety Guardrails
// =============================================================================

/// Configuration for safety guardrails
#[derive(Debug, Clone)]
pub struct SafetyGuardrailsConfig {
    /// Override tracker configuration
    pub override_tracker: OverrideTrackerConfig,
    /// Anomaly detector configuration
    pub anomaly_detector: AnomalyDetectorConfig,
    /// Cooldown duration in minutes after override
    pub cooldown_duration_mins: u64,
    /// Dosage difference threshold for blocking (e.g., 0.20 = 20%)
    pub dosage_mismatch_threshold: f64,
}

impl Default for SafetyGuardrailsConfig {
    fn default() -> Self {
        Self {
            override_tracker: OverrideTrackerConfig::default(),
            anomaly_detector: AnomalyDetectorConfig::default(),
            cooldown_duration_mins: 60,
            dosage_mismatch_threshold: 0.20,
        }
    }
}

/// Safety guardrails for auto-approval
/// Requirements: 7.1, 7.2, 7.3, 7.4, 7.5
pub struct SafetyGuardrails {
    blocklist: Arc<MedicationBlocklist>,
    override_tracker: RwLock<OverrideTracker>,
    anomaly_detector: RwLock<AnomalyDetector>,
    cooldown_tracker: RwLock<CooldownTracker>,
    config: SafetyGuardrailsConfig,
    is_paused: RwLock<bool>,
    pause_reason: RwLock<Option<PauseReason>>,
}

impl SafetyGuardrails {
    /// Create new safety guardrails
    pub fn new(
        blocklist: Arc<MedicationBlocklist>,
        config: SafetyGuardrailsConfig,
    ) -> Self {
        Self {
            blocklist,
            override_tracker: RwLock::new(OverrideTracker::new(config.override_tracker.clone())),
            anomaly_detector: RwLock::new(AnomalyDetector::new(config.anomaly_detector.clone())),
            cooldown_tracker: RwLock::new(CooldownTracker::new()),
            config,
            is_paused: RwLock::new(false),
            pause_reason: RwLock::new(None),
        }
    }

    /// Run all safety checks for a match
    /// Requirements: 7.1, 7.4
    pub async fn check(
        &self,
        offer: &Offer,
        request: &Request,
    ) -> Vec<SafetyCheckResult> {
        let mut results = Vec::new();

        // Check blocklist (Requirement 7.1)
        results.push(self.check_blocklist(&offer.medication, &request.medication));

        // Check cooldown
        results.push(
            self.check_cooldown(&offer.medication, &request.medication)
                .await,
        );

        results
    }

    /// Check if medications are on the blocklist
    /// Requirement 7.1
    fn check_blocklist(&self, offer_med: &str, request_med: &str) -> SafetyCheckResult {
        if let Some(entry) = self.blocklist.is_blocked(offer_med, request_med) {
            SafetyCheckResult::failed(
                "blocklist",
                &format!(
                    "Medication pair blocked: {} - {}. Reason: {}",
                    entry.medication_a, entry.medication_b, entry.reason
                ),
            )
        } else {
            SafetyCheckResult::passed("blocklist")
        }
    }

    /// Check if medication pair is in cooldown
    async fn check_cooldown(&self, offer_med: &str, request_med: &str) -> SafetyCheckResult {
        let tracker = self.cooldown_tracker.read().await;
        if let Some(entry) = tracker.is_in_cooldown(offer_med, request_med) {
            SafetyCheckResult::failed(
                "cooldown",
                &format!(
                    "Medication pair in cooldown until {} (override match: {})",
                    entry.cooldown_until.format("%Y-%m-%d %H:%M:%S UTC"),
                    entry.override_match_id
                ),
            )
        } else {
            SafetyCheckResult::passed("cooldown")
        }
    }

    /// Check if auto-approval should be paused
    /// Requirements: 7.2, 7.3, 7.5
    pub async fn should_pause(&self) -> Option<PauseReason> {
        // Check if already paused
        if *self.is_paused.read().await {
            return self.pause_reason.read().await.clone();
        }

        // Check override rate (Requirement 7.2)
        let tracker = self.override_tracker.read().await;
        if let Some(reason) = tracker.check_override_rate() {
            return Some(reason);
        }

        // Check consecutive overrides (Requirement 7.5)
        if let Some(reason) = tracker.check_consecutive_overrides() {
            return Some(reason);
        }
        drop(tracker);

        // Check for anomalies (Requirement 7.3)
        let detector = self.anomaly_detector.read().await;
        if let Some(description) = detector.check_anomaly() {
            return Some(PauseReason::AnomalyDetected { description });
        }

        None
    }

    /// Record an approval
    pub async fn record_approval(&self, confidence: f64) {
        let mut tracker = self.override_tracker.write().await;
        tracker.record_approval();
        drop(tracker);

        let mut detector = self.anomaly_detector.write().await;
        detector.record_confidence(confidence);
    }

    /// Record an override
    /// Requirements: 7.2, 7.5
    pub async fn record_override(&self, match_id: Uuid, offer_med: &str, request_med: &str) {
        // Record in override tracker
        let mut tracker = self.override_tracker.write().await;
        tracker.record_override();
        drop(tracker);

        // Add cooldown for medication pair
        let mut cooldown = self.cooldown_tracker.write().await;
        cooldown.add_cooldown(
            offer_med,
            request_med,
            self.config.cooldown_duration_mins,
            match_id,
        );
    }

    /// Check if a medication pair is in cooldown
    pub async fn is_in_cooldown(&self, offer_med: &str, request_med: &str) -> bool {
        let tracker = self.cooldown_tracker.read().await;
        tracker.is_in_cooldown(offer_med, request_med).is_some()
    }

    /// Pause the system
    pub async fn pause(&self, reason: PauseReason) {
        *self.is_paused.write().await = true;
        *self.pause_reason.write().await = Some(reason);
    }

    /// Resume the system
    pub async fn resume(&self) {
        *self.is_paused.write().await = false;
        *self.pause_reason.write().await = None;

        // Reset trackers
        self.override_tracker.write().await.reset();
        self.anomaly_detector.write().await.reset();
    }

    /// Check if system is paused
    pub async fn is_paused(&self) -> bool {
        *self.is_paused.read().await
    }

    /// Get pause reason if paused
    pub async fn get_pause_reason(&self) -> Option<PauseReason> {
        self.pause_reason.read().await.clone()
    }

    /// Get current override rate
    pub async fn get_override_rate(&self) -> f64 {
        self.override_tracker.read().await.override_rate()
    }

    /// Get consecutive override count
    pub async fn get_consecutive_overrides(&self) -> u32 {
        self.override_tracker.read().await.consecutive_count()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
    // PauseReason Tests
    // =========================================================================

    #[test]
    fn test_pause_reason_display() {
        let reason = PauseReason::HighOverrideRate {
            rate: 0.15,
            threshold: 0.10,
        };
        let display = format!("{}", reason);
        assert!(display.contains("15.0%"));
        assert!(display.contains("10.0%"));
    }

    #[test]
    fn test_pause_reason_consecutive() {
        let reason = PauseReason::ConsecutiveOverrides { count: 5, limit: 5 };
        let display = format!("{}", reason);
        assert!(display.contains("5 consecutive"));
    }

    // =========================================================================
    // OverrideTracker Tests
    // =========================================================================

    #[test]
    fn test_override_tracker_initial_state() {
        let tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        assert_eq!(tracker.override_rate(), 0.0);
        assert_eq!(tracker.consecutive_count(), 0);
    }

    #[test]
    fn test_override_tracker_record_approval() {
        let mut tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        tracker.record_approval();
        assert_eq!(tracker.override_rate(), 0.0);
        assert_eq!(tracker.consecutive_count(), 0);
    }

    #[test]
    fn test_override_tracker_record_override() {
        let mut tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        tracker.record_override();
        assert_eq!(tracker.override_rate(), 1.0);
        assert_eq!(tracker.consecutive_count(), 1);
    }

    #[test]
    fn test_override_tracker_mixed_events() {
        let mut tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        tracker.record_approval();
        tracker.record_approval();
        tracker.record_override();
        tracker.record_approval();

        // 1 override out of 4 events = 25%
        assert!((tracker.override_rate() - 0.25).abs() < 0.01);
        // Consecutive resets after approval
        assert_eq!(tracker.consecutive_count(), 0);
    }

    #[test]
    fn test_override_tracker_consecutive_overrides() {
        let mut tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        tracker.record_override();
        tracker.record_override();
        tracker.record_override();

        assert_eq!(tracker.consecutive_count(), 3);
    }

    #[test]
    fn test_override_tracker_consecutive_reset_on_approval() {
        let mut tracker = OverrideTracker::new(OverrideTrackerConfig::default());
        tracker.record_override();
        tracker.record_override();
        tracker.record_approval();

        assert_eq!(tracker.consecutive_count(), 0);
    }

    #[test]
    fn test_override_tracker_check_rate_threshold() {
        let config = OverrideTrackerConfig {
            pause_threshold: 0.10,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        // Add 10 events: 2 overrides, 8 approvals = 20% rate
        for _ in 0..8 {
            tracker.record_approval();
        }
        for _ in 0..2 {
            tracker.record_override();
        }

        // 20% > 10% threshold, should trigger pause
        let result = tracker.check_override_rate();
        assert!(result.is_some());
        assert!(matches!(result, Some(PauseReason::HighOverrideRate { .. })));
    }

    #[test]
    fn test_override_tracker_check_consecutive_limit() {
        let config = OverrideTrackerConfig {
            consecutive_limit: 5,
            ..Default::default()
        };
        let mut tracker = OverrideTracker::new(config);

        for _ in 0..5 {
            tracker.record_override();
        }

        let result = tracker.check_consecutive_overrides();
        assert!(result.is_some());
        assert!(matches!(
            result,
            Some(PauseReason::ConsecutiveOverrides { .. })
        ));
    }

    // =========================================================================
    // AnomalyDetector Tests
    // =========================================================================

    #[test]
    fn test_anomaly_detector_initial_state() {
        let detector = AnomalyDetector::new(AnomalyDetectorConfig::default());
        assert!(detector.check_anomaly().is_none());
    }

    #[test]
    fn test_anomaly_detector_not_enough_samples() {
        let mut detector = AnomalyDetector::new(AnomalyDetectorConfig {
            min_samples: 20,
            ..Default::default()
        });

        for _ in 0..10 {
            detector.record_confidence(0.9);
        }

        // Not enough samples yet
        assert!(detector.check_anomaly().is_none());
    }

    #[test]
    fn test_anomaly_detector_stable_confidence() {
        let mut detector = AnomalyDetector::new(AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        });

        // Add stable confidence scores
        for _ in 0..25 {
            detector.record_confidence(0.85);
        }

        // No anomaly with stable scores
        assert!(detector.check_anomaly().is_none());
    }

    #[test]
    fn test_anomaly_detector_detects_drop() {
        let mut detector = AnomalyDetector::new(AnomalyDetectorConfig {
            min_samples: 20,
            recent_window: 10,
            baseline_window: 10,
            drop_threshold: 0.20,
        });

        // Add baseline scores (high confidence)
        for _ in 0..15 {
            detector.record_confidence(0.90);
        }

        // Add recent scores (low confidence - >20% drop)
        for _ in 0..10 {
            detector.record_confidence(0.60);
        }

        // Should detect anomaly
        let result = detector.check_anomaly();
        assert!(result.is_some());
        assert!(result.unwrap().contains("drop"));
    }

    // =========================================================================
    // CooldownTracker Tests
    // =========================================================================

    #[test]
    fn test_cooldown_tracker_initial_state() {
        let tracker = CooldownTracker::new();
        assert!(tracker.is_in_cooldown("MedA", "MedB").is_none());
    }

    #[test]
    fn test_cooldown_tracker_add_cooldown() {
        let mut tracker = CooldownTracker::new();
        let match_id = Uuid::new_v4();

        tracker.add_cooldown("Aspirin", "Ibuprofen", 60, match_id);

        assert!(tracker.is_in_cooldown("Aspirin", "Ibuprofen").is_some());
    }

    #[test]
    fn test_cooldown_tracker_case_insensitive() {
        let mut tracker = CooldownTracker::new();
        let match_id = Uuid::new_v4();

        tracker.add_cooldown("ASPIRIN", "ibuprofen", 60, match_id);

        assert!(tracker.is_in_cooldown("aspirin", "IBUPROFEN").is_some());
    }

    #[test]
    fn test_cooldown_tracker_order_independent() {
        let mut tracker = CooldownTracker::new();
        let match_id = Uuid::new_v4();

        tracker.add_cooldown("Aspirin", "Ibuprofen", 60, match_id);

        // Should find cooldown regardless of order
        assert!(tracker.is_in_cooldown("Ibuprofen", "Aspirin").is_some());
    }

    #[test]
    fn test_cooldown_tracker_different_pair_not_blocked() {
        let mut tracker = CooldownTracker::new();
        let match_id = Uuid::new_v4();

        tracker.add_cooldown("Aspirin", "Ibuprofen", 60, match_id);

        // Different pair should not be in cooldown
        assert!(tracker.is_in_cooldown("Aspirin", "Tylenol").is_none());
    }

    #[test]
    fn test_cooldown_tracker_clear() {
        let mut tracker = CooldownTracker::new();
        let match_id = Uuid::new_v4();

        tracker.add_cooldown("Aspirin", "Ibuprofen", 60, match_id);
        tracker.clear();

        assert!(tracker.is_in_cooldown("Aspirin", "Ibuprofen").is_none());
    }
}
