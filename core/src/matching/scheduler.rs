//! Learning scheduler module
//!
//! Ported from legacy/matching/scheduler.go

use chrono::{DateTime, Duration, Utc};
use std::sync::RwLock;

use super::{LearningConfig, PerformanceMetrics, WeightLearner, Weights};

/// Job execution status
/// Ported from Go: JobStatus (scheduler.go:34-43)
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Initial state
    #[default]
    Pending,
    /// Currently executing
    Running,
    /// Completed successfully
    Success,
    /// Execution failed
    Failed,
    /// Skipped (e.g., insufficient data)
    Skipped,
    /// Weights calculated but not applied (manual review)
    Recommended,
}

/// Auto-apply configuration
/// Ported from Go: config.AutoApplyConfig
#[derive(Debug, Clone)]
pub struct AutoApplyConfig {
    /// Enable auto-apply
    pub enabled: bool,
    /// Require improvement to apply
    pub require_improvement: bool,
    /// Minimum separation gain required
    pub min_separation_gain: f64,
    /// Maximum confirmation rate drop allowed
    pub max_confirmation_rate_drop: f64,
}

impl Default for AutoApplyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            require_improvement: true,
            min_separation_gain: 0.01,
            max_confirmation_rate_drop: 0.05,
        }
    }
}

/// Notification configuration
#[derive(Debug, Clone, Default)]
pub struct NotificationConfig {
    pub on_success: bool,
    pub on_failure: bool,
    pub on_recommendation: bool,
}

/// Scheduler configuration
/// Ported from Go: config.AdaptiveLearningConfig
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Enable scheduler
    pub enabled: bool,
    /// Cron schedule (e.g., "0 3 * * *")
    pub schedule: String,
    /// Learning algorithm config
    pub algorithm: LearningConfig,
    /// Auto-apply settings
    pub auto_apply: AutoApplyConfig,
    /// Notification settings
    pub notifications: NotificationConfig,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: "0 3 * * *".to_string(), // Daily at 3 AM
            algorithm: LearningConfig::default(),
            auto_apply: AutoApplyConfig::default(),
            notifications: NotificationConfig::default(),
        }
    }
}

/// Current scheduler state
/// Ported from Go: SchedulerStatus (scheduler.go:315-324)
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    pub enabled: bool,
    pub schedule: String,
    pub last_run: Option<DateTime<Utc>>,
    pub last_status: JobStatus,
    pub last_error: Option<String>,
    pub last_metrics: Option<PerformanceMetrics>,
    pub pending_apply: Option<Weights>,
    pub pending_reason: Option<String>,
}

/// Learning scheduler for automated weight optimization
/// Ported from Go: LearningScheduler (scheduler.go:17-31)
pub struct LearningScheduler {
    config: RwLock<SchedulerConfig>,
    learner: Option<WeightLearner>,
    // State tracking
    last_run: RwLock<Option<DateTime<Utc>>>,
    last_status: RwLock<JobStatus>,
    last_error: RwLock<Option<String>>,
    last_metrics: RwLock<Option<PerformanceMetrics>>,
    pending_apply: RwLock<Option<Weights>>,
    pending_reason: RwLock<Option<String>>,
}

impl Default for LearningScheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

impl LearningScheduler {
    /// Create a new learning scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            learner: Some(WeightLearner::new()),
            last_run: RwLock::new(None),
            last_status: RwLock::new(JobStatus::Pending),
            last_error: RwLock::new(None),
            last_metrics: RwLock::new(None),
            pending_apply: RwLock::new(None),
            pending_reason: RwLock::new(None),
        }
    }

    /// Create scheduler with custom learner
    pub fn with_learner(config: SchedulerConfig, learner: WeightLearner) -> Self {
        Self {
            config: RwLock::new(config),
            learner: Some(learner),
            last_run: RwLock::new(None),
            last_status: RwLock::new(JobStatus::Pending),
            last_error: RwLock::new(None),
            last_metrics: RwLock::new(None),
            pending_apply: RwLock::new(None),
            pending_reason: RwLock::new(None),
        }
    }

    /// Check if scheduler is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Get current status
    /// Ported from Go: LearningScheduler.Status (scheduler.go:297-312)
    pub fn status(&self) -> SchedulerStatus {
        let config = self.config.read().unwrap();
        SchedulerStatus {
            enabled: config.enabled,
            schedule: config.schedule.clone(),
            last_run: *self.last_run.read().unwrap(),
            last_status: *self.last_status.read().unwrap(),
            last_error: self.last_error.read().unwrap().clone(),
            last_metrics: self.last_metrics.read().unwrap().clone(),
            pending_apply: self.pending_apply.read().unwrap().clone(),
            pending_reason: self.pending_reason.read().unwrap().clone(),
        }
    }

    /// Determine if new weights should be auto-applied
    /// Ported from Go: LearningScheduler.shouldApply (scheduler.go:195-214)
    pub fn should_apply(&self, current: &PerformanceMetrics, new: &PerformanceMetrics) -> bool {
        let config = self.config.read().unwrap();

        if !config.auto_apply.require_improvement {
            return true;
        }

        // Calculate separation (confirmed - rejected avg scores)
        let current_sep = current.avg_score_confirmed - current.avg_score_rejected;
        let new_sep = new.avg_score_confirmed - new.avg_score_rejected;

        // Check separation gain
        let sep_gain = new_sep - current_sep;
        if sep_gain < config.auto_apply.min_separation_gain {
            return false;
        }

        // Check confirmation rate drop
        let rate_drop = current.confirmation_rate - new.confirmation_rate;
        rate_drop <= config.auto_apply.max_confirmation_rate_drop
    }

    /// Get skip reason
    /// Ported from Go: LearningScheduler.getSkipReason (scheduler.go:216-225)
    pub fn get_skip_reason(&self, should_apply: bool) -> String {
        let config = self.config.read().unwrap();
        if !config.auto_apply.enabled {
            "auto-apply disabled, manual review required".to_string()
        } else if !should_apply {
            "performance improvement threshold not met".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Build note for weight change
    /// Ported from Go: LearningScheduler.buildNote (scheduler.go:260-273)
    pub fn build_note(&self, old: &Weights, new: &Weights, metrics: &PerformanceMetrics) -> String {
        format!(
            "Auto-learned from {} samples. Separation: {:.3}. \
             Weights changed: med {:.2}→{:.2}, dosage {:.2}→{:.2}, \
             qty {:.2}→{:.2}, price {:.2}→{:.2}, recency {:.2}→{:.2}",
            metrics.sample_size,
            metrics.avg_score_confirmed - metrics.avg_score_rejected,
            old.medication,
            new.medication,
            old.dosage,
            new.dosage,
            old.quantity,
            new.quantity,
            old.price,
            new.price,
            old.recency,
            new.recency,
        )
    }

    /// Get current metrics (for comparison)
    /// Ported from Go: LearningScheduler.getCurrentMetrics (scheduler.go:241-258)
    pub fn get_current_metrics(&self) -> PerformanceMetrics {
        if let Some(metrics) = self.last_metrics.read().unwrap().as_ref() {
            return metrics.clone();
        }

        // Default neutral metrics
        PerformanceMetrics {
            confirmation_rate: 0.5,
            avg_score_confirmed: 0.7,
            avg_score_rejected: 0.3,
            ..Default::default()
        }
    }

    /// Record job start
    fn record_job_start(&self) {
        *self.last_run.write().unwrap() = Some(Utc::now());
        *self.last_status.write().unwrap() = JobStatus::Running;
        *self.last_error.write().unwrap() = None;
    }

    /// Record job success
    fn record_success(&self, metrics: PerformanceMetrics) {
        *self.last_status.write().unwrap() = JobStatus::Success;
        *self.last_metrics.write().unwrap() = Some(metrics);
        *self.pending_apply.write().unwrap() = None;
        *self.pending_reason.write().unwrap() = None;
    }

    /// Record job as recommended (needs manual review)
    fn record_recommended(&self, metrics: PerformanceMetrics, weights: Weights, reason: String) {
        *self.last_status.write().unwrap() = JobStatus::Recommended;
        *self.last_metrics.write().unwrap() = Some(metrics);
        *self.pending_apply.write().unwrap() = Some(weights);
        *self.pending_reason.write().unwrap() = Some(reason);
    }

    /// Record job failure
    fn record_failure(&self, error: String) {
        *self.last_status.write().unwrap() = JobStatus::Failed;
        *self.last_error.write().unwrap() = Some(error);
    }

    /// Get analysis date range
    pub fn get_analysis_range(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let config = self.config.read().unwrap();
        let end_date = Utc::now();
        let start_date = end_date - Duration::days(config.algorithm.analysis_window);
        (start_date, end_date)
    }

    /// Reject pending weights without applying
    /// Ported from Go: LearningScheduler.RejectPending (scheduler.go:360-370)
    pub fn reject_pending(&self) {
        *self.pending_apply.write().unwrap() = None;
        *self.pending_reason.write().unwrap() = Some("rejected by user".to_string());
        *self.last_status.write().unwrap() = JobStatus::Skipped;

        tracing::info!("Pending weights rejected");
    }

    /// Update scheduler configuration
    /// Ported from Go: LearningScheduler.UpdateConfig (scheduler.go:372-392)
    pub fn update_config(&self, config: SchedulerConfig) {
        // Update learner config if present
        if let Some(ref learner) = self.learner {
            learner.set_config(config.algorithm.clone());
        }

        *self.config.write().unwrap() = config;

        tracing::info!("Scheduler config updated");
    }

    /// Get learner reference
    pub fn learner(&self) -> Option<&WeightLearner> {
        self.learner.as_ref()
    }

    /// Check if there are pending weights
    pub fn has_pending(&self) -> bool {
        self.pending_apply.read().unwrap().is_some()
    }

    /// Get pending weights if any
    pub fn pending_weights(&self) -> Option<Weights> {
        self.pending_apply.read().unwrap().clone()
    }
}

// ============================================================================
// Tests - Ported from scheduler_test.go
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_learning_scheduler() {
        let config = SchedulerConfig {
            enabled: false,
            schedule: "0 3 * * *".to_string(),
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        assert!(!scheduler.is_enabled());
        assert_eq!(*scheduler.last_status.read().unwrap(), JobStatus::Pending);
    }

    #[test]
    fn test_scheduler_status() {
        let config = SchedulerConfig {
            enabled: true,
            schedule: "0 3 * * *".to_string(),
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);
        let status = scheduler.status();

        assert!(status.enabled);
        assert_eq!(status.schedule, "0 3 * * *");
        assert_eq!(status.last_status, JobStatus::Pending);
    }

    #[test]
    fn test_should_apply_require_improvement_disabled() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: true,
                require_improvement: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let old = PerformanceMetrics::default();
        let new = PerformanceMetrics::default();

        assert!(scheduler.should_apply(&old, &new));
    }

    #[test]
    fn test_should_apply_separation_improved() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: true,
                require_improvement: true,
                min_separation_gain: 0.01,
                max_confirmation_rate_drop: 0.05,
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let old = PerformanceMetrics {
            avg_score_confirmed: 0.85,
            avg_score_rejected: 0.65,
            confirmation_rate: 0.75,
            ..Default::default()
        };

        let new = PerformanceMetrics {
            avg_score_confirmed: 0.88,
            avg_score_rejected: 0.62,
            confirmation_rate: 0.76,
            ..Default::default()
        };

        assert!(scheduler.should_apply(&old, &new));
    }

    #[test]
    fn test_should_apply_insufficient_gain() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: true,
                require_improvement: true,
                min_separation_gain: 0.10, // Need 10% gain
                max_confirmation_rate_drop: 0.05,
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let old = PerformanceMetrics {
            avg_score_confirmed: 0.85,
            avg_score_rejected: 0.65,
            confirmation_rate: 0.75,
            ..Default::default()
        };

        let new = PerformanceMetrics {
            avg_score_confirmed: 0.86,
            avg_score_rejected: 0.64, // Only 0.02 gain
            confirmation_rate: 0.75,
            ..Default::default()
        };

        assert!(!scheduler.should_apply(&old, &new));
    }

    #[test]
    fn test_should_apply_rate_dropped() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: true,
                require_improvement: true,
                min_separation_gain: 0.01,
                max_confirmation_rate_drop: 0.05,
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let old = PerformanceMetrics {
            avg_score_confirmed: 0.80,
            avg_score_rejected: 0.60,
            confirmation_rate: 0.80,
            ..Default::default()
        };

        let new = PerformanceMetrics {
            avg_score_confirmed: 0.90,
            avg_score_rejected: 0.55,
            confirmation_rate: 0.70, // Dropped by 0.10
            ..Default::default()
        };

        assert!(!scheduler.should_apply(&old, &new));
    }

    #[test]
    fn test_reject_pending() {
        let scheduler = LearningScheduler::default();

        // Set pending weights
        *scheduler.pending_apply.write().unwrap() = Some(Weights {
            medication: 0.5,
            ..Default::default()
        });
        *scheduler.pending_reason.write().unwrap() = Some("test".to_string());

        scheduler.reject_pending();

        assert!(scheduler.pending_apply.read().unwrap().is_none());
        assert_eq!(*scheduler.last_status.read().unwrap(), JobStatus::Skipped);
    }

    #[test]
    fn test_build_note() {
        let scheduler = LearningScheduler::default();

        let old = Weights {
            medication: 0.45,
            dosage: 0.10,
            quantity: 0.20,
            price: 0.15,
            recency: 0.10,
        };

        let new = Weights {
            medication: 0.48,
            dosage: 0.10,
            quantity: 0.18,
            price: 0.14,
            recency: 0.10,
        };

        let metrics = PerformanceMetrics {
            sample_size: 200,
            avg_score_confirmed: 0.88,
            avg_score_rejected: 0.65,
            ..Default::default()
        };

        let note = scheduler.build_note(&old, &new, &metrics);

        assert!(!note.is_empty());
        assert!(note.contains("200"));
    }

    #[test]
    fn test_get_skip_reason_auto_apply_disabled() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let reason = scheduler.get_skip_reason(true);
        assert!(reason.contains("manual review"));
    }

    #[test]
    fn test_get_skip_reason_threshold_not_met() {
        let config = SchedulerConfig {
            auto_apply: AutoApplyConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let scheduler = LearningScheduler::new(config);

        let reason = scheduler.get_skip_reason(false);
        assert!(reason.contains("threshold"));
    }

    #[test]
    fn test_update_config() {
        let scheduler = LearningScheduler::default();

        let new_config = SchedulerConfig {
            enabled: true,
            schedule: "0 6 * * *".to_string(),
            algorithm: LearningConfig {
                learning_rate: 0.05,
                min_samples: 200,
                ..Default::default()
            },
            ..Default::default()
        };

        scheduler.update_config(new_config);

        let config = scheduler.config.read().unwrap();
        assert!(config.enabled);
        assert_eq!(config.schedule, "0 6 * * *");
        assert!((config.algorithm.learning_rate - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_get_current_metrics_default() {
        let scheduler = LearningScheduler::default();

        let metrics = scheduler.get_current_metrics();

        // Should return neutral defaults
        assert!((metrics.confirmation_rate - 0.5).abs() < 0.001);
        assert!((metrics.avg_score_confirmed - 0.7).abs() < 0.001);
        assert!((metrics.avg_score_rejected - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_record_job_states() {
        let scheduler = LearningScheduler::default();

        // Test record start
        scheduler.record_job_start();
        assert_eq!(*scheduler.last_status.read().unwrap(), JobStatus::Running);
        assert!(scheduler.last_run.read().unwrap().is_some());

        // Test record success
        let metrics = PerformanceMetrics {
            sample_size: 100,
            ..Default::default()
        };
        scheduler.record_success(metrics.clone());
        assert_eq!(*scheduler.last_status.read().unwrap(), JobStatus::Success);

        // Test record failure
        scheduler.record_failure("test error".to_string());
        assert_eq!(*scheduler.last_status.read().unwrap(), JobStatus::Failed);
        assert!(scheduler.last_error.read().unwrap().is_some());

        // Test record recommended
        let weights = Weights::default();
        scheduler.record_recommended(metrics, weights, "needs review".to_string());
        assert_eq!(
            *scheduler.last_status.read().unwrap(),
            JobStatus::Recommended
        );
        assert!(scheduler.pending_apply.read().unwrap().is_some());
    }
}
