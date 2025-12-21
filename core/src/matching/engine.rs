//! Unified Matching Engine
//!
//! Integrates all matching components:
//! - Scorer (multi-field scoring)
//! - WeightLearner (adaptive learning)
//! - LearningScheduler (automated jobs)
//! - WarmStartManager (cold start handling)
//! - ABTestManager (A/B testing)
//! - OutlierDetector (anomaly filtering)
//! - MatchFilter (stale/same-sender filtering)
//! - EmbeddingCache (medication embedding cache)
//! - AuditTrail (comprehensive audit logging)

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};

use super::{
    ABTestConfig, ABTestManager, ABTestResult, AuditTrail, AuditTrailConfig, AutoActionHandler,
    CalibrationConfig, CalibrationReport, ConfidenceCalibrator, ConfidenceConfig,
    ConfidenceManager, ConfidenceManagerStats, EmbeddingCache, EmbeddingCacheStatsSnapshot,
    LearnerError, MatchAction, MatchFilter, MatchFilterConfig, MatchFilterStatsSnapshot,
    MatchScore, OutlierDetector, OutlierDetectorConfig, PerformanceMetrics, SchedulerConfig,
    SchedulerStatus, Scorer, WarmStartConfig, WarmStartManager, WeightLearner, Weights,
};
use crate::domain::{AuditAction, AuditLog, EntityType, FeedbackStats, MedicationMapping};
use crate::domain::{Match as MatchEntity, Offer, Request};
use crate::notify::MatchNotifier;
use crate::repository::{AuditLogRepository, FeedbackRecordRepository};

/// Matching engine configuration
#[derive(Debug, Default, Clone)]
pub struct MatchingEngineConfig {
    /// Initial weights
    pub weights: Weights,
    /// Scheduler settings
    pub scheduler: SchedulerConfig,
    /// Warm start settings
    pub warm_start: WarmStartConfig,
    /// Outlier detection settings
    pub outlier_detector: OutlierDetectorConfig,
    /// Confidence manager settings
    pub confidence: ConfidenceConfig,
    /// Calibration settings
    pub calibration: CalibrationConfig,
    /// Match filter settings
    pub match_filter: MatchFilterConfig,
    /// Audit trail settings
    pub audit_trail: AuditTrailConfig,
}

/// Unified matching engine orchestrating all components
pub struct MatchingEngine {
    /// Core scorer
    scorer: Scorer,
    /// Adaptive weight learner
    learner: WeightLearner,
    /// Cold start handler
    warm_start: WarmStartManager,
    /// A/B test manager
    ab_test: ABTestManager,
    /// Outlier detector
    outlier_detector: OutlierDetector,
    /// Dynamic confidence manager
    confidence_manager: ConfidenceManager,
    /// Confidence calibrator
    calibrator: ConfidenceCalibrator,
    /// Match filter (stale/same-sender)
    match_filter: MatchFilter,
    /// Embedding cache for medications
    embedding_cache: EmbeddingCache,
    /// Audit trail for match actions
    audit_trail: AuditTrail,
    /// Configuration
    config: RwLock<MatchingEngineConfig>,
    /// Current sample count for warm start
    sample_count: RwLock<usize>,
    /// Cron scheduler handle
    scheduler_handle: RwLock<Option<JobScheduler>>,
    /// Auto action handler
    pub auto_action: AutoActionHandler,
    /// Notification sender
    pub notifier: Arc<dyn MatchNotifier>,
    /// Repository for fetching feedback
    feedback_repo: Option<Arc<dyn FeedbackRecordRepository>>,
    /// Repository for audit logging
    audit_log_repo: Option<Arc<dyn AuditLogRepository>>,
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new(MatchingEngineConfig::default())
    }
}

impl MatchingEngine {
    /// Create a new matching engine
    pub fn new(config: MatchingEngineConfig) -> Self {
        let scorer = Scorer::new(Some(config.weights.clone()), None);
        let learner = WeightLearner::with_config(config.scheduler.algorithm.clone());
        let warm_start = WarmStartManager::new(config.warm_start.clone());
        let ab_test = ABTestManager::new(config.weights.clone());
        let outlier_detector = OutlierDetector::new(config.outlier_detector.clone());
        let confidence_manager = ConfidenceManager::new(config.confidence.clone());
        let calibrator = ConfidenceCalibrator::new(config.calibration.clone());
        let match_filter = MatchFilter::new(config.match_filter.clone());
        let embedding_cache = EmbeddingCache::new();
        let audit_trail = AuditTrail::new(config.audit_trail.clone());

        Self {
            scorer,
            learner,
            warm_start,
            ab_test,
            outlier_detector,
            confidence_manager,
            calibrator,
            match_filter,
            embedding_cache,
            audit_trail,
            config: RwLock::new(config),
            sample_count: RwLock::new(0),
            scheduler_handle: RwLock::new(None),
            auto_action: AutoActionHandler::from_env(),
            notifier: Arc::new(crate::notify::NullNotifier), // Default to null, can be replaced
            feedback_repo: None,
            audit_log_repo: None,
        }
    }

    /// Set repositories for the learning job
    pub fn set_repositories(
        &mut self,
        feedback_repo: Arc<dyn FeedbackRecordRepository>,
        audit_log_repo: Arc<dyn AuditLogRepository>,
    ) {
        self.feedback_repo = Some(feedback_repo);
        self.audit_log_repo = Some(audit_log_repo);
    }

    /// Update the notifier
    pub fn set_notifier(&mut self, notifier: Arc<dyn MatchNotifier>) {
        self.notifier = notifier;
    }

    // =========================================================================
    // Scoring
    // =========================================================================

    /// Score a match between offer and request
    /// Uses: Scorer + WarmStart + ABTest + AutoAction + ConfidenceManager
    pub async fn score_match(
        &self,
        offer: &Offer,
        request: &Request,
        medication_score: f64,
        user_id: Option<&str>,
    ) -> (MatchScore, MatchAction) {
        // Get weights (consider A/B test if user_id provided)
        let weights = self.get_weights_for_scoring(user_id).await;

        // Apply warm start blending
        let sample_count = *self.sample_count.read().await;
        let effective_weights = self
            .warm_start
            .get_effective_weights(&weights, sample_count);

        // Update scorer with effective weights
        self.scorer.update_weights(effective_weights);

        // Score the match
        let score = self.scorer.score_match(offer, request, medication_score);

        // Evaluate confidence (tracks statistics and may adjust thresholds)
        let _meets_strict = self.confidence_manager.evaluate(score.total);

        // Determine action based on score
        let action = self.auto_action.determine_action(score.total).await;

        (score, action)
    }

    /// Process a MatchAction (notify, auto-confirm, etc.)
    /// Now includes audit trail logging
    pub async fn process_match_action(
        &self,
        match_entity: &MatchEntity,
        action: MatchAction,
    ) -> crate::Result<()> {
        // Log to audit trail first
        let reason = match action {
            MatchAction::AutoConfirm => "High confidence auto-confirmation",
            MatchAction::SuggestToOperator => "Medium confidence - suggested to operator",
            MatchAction::QueueForReview => "Low confidence - queued for review",
            MatchAction::Ignore => "Below threshold - ignored",
        };

        if let Err(e) = self.log_match_action(match_entity, action, reason).await {
            tracing::warn!(error = %e, "Failed to log match action to audit trail");
        }

        // Process notifications
        match action {
            MatchAction::AutoConfirm => {
                // Notifying about auto-confirmation
                self.notifier
                    .notify_auto_confirmed(&match_entity.id, match_entity.score)
                    .await?;
            }
            MatchAction::SuggestToOperator => {
                self.notifier.notify_suggested(match_entity).await?;
            }
            MatchAction::QueueForReview => {
                self.notifier
                    .notify_queued_for_review(&match_entity.id, "low_confidence_match")
                    .await?;
            }
            MatchAction::Ignore => {
                // Nothing to do
            }
        }

        // Also notify about the new match in general if it's not ignored
        if action != MatchAction::Ignore {
            self.notifier.notify_new_match(match_entity, action).await?;
        }

        Ok(())
    }

    /// Get weights for scoring (considering A/B tests)
    async fn get_weights_for_scoring(&self, user_id: Option<&str>) -> Weights {
        if let Some(uid) = user_id {
            let (weights, _group) = self.ab_test.get_weights_for_user(uid);
            weights
        } else {
            self.scorer.get_weights()
        }
    }

    // =========================================================================
    // Feedback & Learning
    // =========================================================================

    /// Record operator feedback (confirm/reject)
    /// Uses: OutlierDetector + ABTest + SampleCount + Calibrator
    pub async fn record_feedback(
        &self,
        user_id: &str,
        confirmed: bool,
        total_score: f64,
    ) -> std::result::Result<(), String> {
        // Check for outliers
        if self.outlier_detector.is_outlier(total_score) {
            tracing::warn!(
                user_id = user_id,
                score = total_score,
                "Feedback rejected as outlier"
            );
            return Ok(()); // Silently ignore outliers
        }

        // Add to outlier detector window
        self.outlier_detector.add_score(total_score);

        // Record for A/B testing
        self.ab_test
            .record_feedback(user_id, confirmed, total_score);

        // Record for calibration (prediction-outcome pair)
        self.calibrator.record_outcome(total_score, confirmed);

        // Increment sample count
        {
            let mut count = self.sample_count.write().await;
            *count += 1;
        }

        tracing::debug!(
            user_id = user_id,
            confirmed = confirmed,
            score = total_score,
            "Feedback recorded"
        );

        Ok(())
    }

    /// Trigger learning calculation
    /// Uses: WeightLearner + WarmStart
    pub async fn calculate_new_weights(
        &self,
        stats: &FeedbackStats,
    ) -> std::result::Result<(Weights, PerformanceMetrics), LearnerError> {
        let sample_count = *self.sample_count.read().await;

        // Get current weights with warm start blending
        let current = self.scorer.get_weights();
        let effective = self
            .warm_start
            .get_effective_weights(&current, sample_count);

        // Calculate optimal weights
        let (new_weights, metrics) = self.learner.calculate_optimal_weights(stats, &effective)?;

        tracing::info!(
            sample_size = stats.total_feedbacks,
            confirmation_rate = format!("{:.1}%", metrics.confirmation_rate * 100.0),
            "Calculated new weights"
        );

        Ok((new_weights, metrics))
    }

    /// Apply new weights
    pub async fn apply_weights(&self, weights: Weights, reason: &str) {
        self.scorer.update_weights(weights.clone());
        self.ab_test.set_base_weights(weights.clone());

        tracing::info!(
            medication = format!("{:.2}", weights.medication),
            dosage = format!("{:.2}", weights.dosage),
            quantity = format!("{:.2}", weights.quantity),
            price = format!("{:.2}", weights.price),
            recency = format!("{:.2}", weights.recency),
            reason = reason,
            "Applied new weights"
        );
    }

    // =========================================================================
    // Scheduler
    // =========================================================================

    /// Start the learning scheduler
    pub async fn start_scheduler(self: Arc<Self>) -> std::result::Result<(), String> {
        let config = self.config.read().await;

        if !config.scheduler.enabled {
            tracing::info!("Learning scheduler disabled");
            return Ok(());
        }

        let schedule = config.scheduler.schedule.clone();
        drop(config);

        // Create scheduler
        let scheduler = JobScheduler::new().await.map_err(|e| e.to_string())?;

        // Add the learning job
        let engine = self.clone();
        let job = Job::new_async(schedule.as_str(), move |_uuid, _lock| {
            let engine = engine.clone();
            Box::pin(async move {
                tracing::info!("Learning job triggered by scheduler");
                if let Err(e) = engine.run_learning_job().await {
                    tracing::error!(error = %e, "Learning job failed");
                }
            })
        })
        .map_err(|e| e.to_string())?;

        scheduler.add(job).await.map_err(|e| e.to_string())?;
        scheduler.start().await.map_err(|e| e.to_string())?;

        *self.scheduler_handle.write().await = Some(scheduler);

        tracing::info!(schedule = schedule, "Learning scheduler started");
        Ok(())
    }

    /// Primary learning job: updates weights and thresholds
    pub async fn run_learning_job(&self) -> crate::Result<()> {
        let feedback_repo = self.feedback_repo.as_ref().ok_or_else(|| {
            crate::Error::Internal("Feedback repository not set in matching engine".to_owned())
        })?;

        // 1. Get stats for the last 30 days
        let end = chrono::Utc::now();
        let start = end - chrono::Duration::days(30);
        let stats = feedback_repo.get_stats(start, end).await?;

        if stats.total_feedbacks < 50 {
            tracing::info!(
                count = stats.total_feedbacks,
                "Not enough feedback for learning (minimum 50)"
            );
            return Ok(());
        }

        // 2. Calculate new weights
        let (new_weights, metrics) = self
            .calculate_new_weights(&stats)
            .await
            .map_err(|e| crate::Error::Internal(format!("Weight calculation failed: {}", e)))?;

        // 3. Adjust thresholds based on confirmation rate
        // If confirmation rate is high, we can be more aggressive with auto-actions
        let calculator = self.auto_action.calculator();
        let mut threshold_config = calculator.config().read().await.clone();

        // Simple heuristic: adjust auto threshold based on metrics
        if metrics.confirmation_rate > 0.95 {
            threshold_config.auto_threshold = (threshold_config.auto_threshold - 0.01).max(0.85);
        } else if metrics.confirmation_rate < 0.85 {
            threshold_config.auto_threshold = (threshold_config.auto_threshold + 0.01).min(0.95);
        }

        // 4. Update the engine
        self.apply_weights(new_weights.clone(), "Automated periodic learning")
            .await;
        calculator.update_config(threshold_config).await;

        // 5. Audit log the change
        if let Some(audit_repo) = &self.audit_log_repo {
            let audit_log = AuditLog::system(
                AuditAction::WeightsUpdated,
                EntityType::Weights,
                "current".to_string(),
            )
            .with_details(serde_json::json!({
                "weights": new_weights,
                "confirmation_rate": metrics.confirmation_rate,
                "sample_size": stats.total_feedbacks
            }));
            let _ = audit_repo.save(&audit_log).await;
        }

        tracing::info!("✅ Learning job completed successfully");
        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop_scheduler(&self) {
        if let Some(mut scheduler) = self.scheduler_handle.write().await.take() {
            scheduler.shutdown().await.ok();
            tracing::info!("Learning scheduler stopped");
        }
    }

    /// Get scheduler status
    pub async fn scheduler_status(&self) -> SchedulerStatus {
        let config = self.config.read().await;
        SchedulerStatus {
            enabled: config.scheduler.enabled,
            schedule: config.scheduler.schedule.clone(),
            last_run: None, // Would be tracked in production
            last_status: super::JobStatus::Pending,
            last_error: None,
            last_metrics: None,
            pending_apply: None,
            pending_reason: None,
        }
    }

    // =========================================================================
    // A/B Testing
    // =========================================================================

    /// Create a new A/B test
    pub fn create_ab_test(&self, config: ABTestConfig) -> std::result::Result<(), String> {
        self.ab_test.create_test(config)
    }

    /// Get A/B test results
    pub fn get_ab_test_result(&self, test_id: &str) -> Option<ABTestResult> {
        self.ab_test.get_test_result(test_id)
    }

    /// End an A/B test
    pub fn end_ab_test(&self, test_id: &str) -> Option<ABTestResult> {
        self.ab_test.end_test(test_id)
    }

    /// Get all active A/B tests
    pub fn get_active_ab_tests(&self) -> Vec<ABTestConfig> {
        self.ab_test.get_active_tests()
    }

    // =========================================================================
    // Warm Start
    // =========================================================================

    /// Get current prior influence percentage
    pub async fn get_prior_influence(&self) -> f64 {
        let sample_count = *self.sample_count.read().await;
        self.warm_start.get_prior_influence(sample_count)
    }

    /// Reset warm start timer
    pub fn reset_warm_start(&self) {
        self.warm_start.reset();
    }

    // =========================================================================
    // Outlier Detection
    // =========================================================================

    /// Get outlier detector statistics
    pub fn get_outlier_stats(&self) -> (f64, f64, usize) {
        self.outlier_detector.get_stats()
    }

    /// Reset outlier detector
    pub fn reset_outlier_detector(&self) {
        self.outlier_detector.reset();
    }

    // =========================================================================
    // Confidence Management
    // =========================================================================

    /// Get confidence manager statistics
    pub fn get_confidence_stats(&self) -> ConfidenceManagerStats {
        self.confidence_manager.get_stats()
    }

    /// Get current strict confidence threshold
    pub fn get_strict_threshold(&self) -> f64 {
        self.confidence_manager.strict_threshold()
    }

    /// Get current relaxed confidence threshold
    pub fn get_relaxed_threshold(&self) -> f64 {
        self.confidence_manager.relaxed_threshold()
    }

    /// Set strict confidence threshold manually
    pub fn set_strict_threshold(&self, threshold: f64) {
        self.confidence_manager.set_strict_threshold(threshold);
    }

    /// Set relaxed confidence threshold manually
    pub fn set_relaxed_threshold(&self, threshold: f64) {
        self.confidence_manager.set_relaxed_threshold(threshold);
    }

    /// Enable or disable adaptive confidence adjustment
    pub fn enable_adaptive_confidence(&self, enabled: bool) {
        self.confidence_manager.enable_adaptive(enabled);
    }

    /// Reset confidence thresholds to base values
    pub fn reset_confidence_thresholds(&self) {
        self.confidence_manager.reset_to_base();
    }

    /// Reset confidence statistics
    pub fn reset_confidence_stats(&self) {
        self.confidence_manager.reset_stats();
    }

    /// Get confidence configuration
    pub fn get_confidence_config(&self) -> ConfidenceConfig {
        self.confidence_manager.get_config()
    }

    /// Update confidence configuration
    pub fn set_confidence_config(&self, config: ConfidenceConfig) {
        self.confidence_manager.set_config(config);
    }

    // =========================================================================
    // Calibration
    // =========================================================================

    /// Calibrate a raw confidence score based on historical outcomes
    pub fn calibrate_score(&self, raw_score: f64) -> f64 {
        self.calibrator.calibrate(raw_score)
    }

    /// Get calibration report with ECE, MCE, and bin statistics
    pub fn get_calibration_report(&self) -> CalibrationReport {
        self.calibrator.get_report()
    }

    /// Get calibration configuration
    pub fn get_calibration_config(&self) -> CalibrationConfig {
        self.calibrator.get_config()
    }

    /// Update calibration configuration
    pub fn set_calibration_config(&self, config: CalibrationConfig) {
        self.calibrator.set_config(config);
    }

    /// Enable or disable calibration
    pub fn enable_calibration(&self, enabled: bool) {
        self.calibrator.enable(enabled);
    }

    /// Set calibration smoothing factor (0.0 = raw, 1.0 = full calibration)
    pub fn set_calibration_smoothing(&self, factor: f64) {
        self.calibrator.set_smoothing_factor(factor);
    }

    /// Reset all calibration data
    pub fn reset_calibration(&self) {
        self.calibrator.reset();
    }

    /// Check if calibration is enabled
    pub fn is_calibration_enabled(&self) -> bool {
        self.calibrator.is_enabled()
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    /// Get current weights
    pub fn get_weights(&self) -> Weights {
        self.scorer.get_weights()
    }

    /// Update configuration
    pub async fn update_config(&self, config: MatchingEngineConfig) {
        // Update scorer
        self.scorer.update_weights(config.weights.clone());

        // Update learner
        self.learner.set_config(config.scheduler.algorithm.clone());

        // Update warm start
        self.warm_start.set_config(config.warm_start.clone());

        // Update outlier detector
        self.outlier_detector
            .set_config(config.outlier_detector.clone());

        // Update confidence manager
        self.confidence_manager
            .set_config(config.confidence.clone());

        // Update calibrator
        self.calibrator.set_config(config.calibration.clone());

        // Update match filter
        self.match_filter.set_config(config.match_filter.clone());

        // Update audit trail
        self.audit_trail.set_config(config.audit_trail.clone());

        // Update A/B test base weights
        self.ab_test.set_base_weights(config.weights.clone());

        // Store config
        *self.config.write().await = config;

        tracing::info!("Matching engine configuration updated");
    }

    /// Get current sample count
    pub async fn get_sample_count(&self) -> usize {
        *self.sample_count.read().await
    }

    /// Access scorer directly
    pub fn scorer(&self) -> &Scorer {
        &self.scorer
    }

    /// Access learner directly
    pub fn learner(&self) -> &WeightLearner {
        &self.learner
    }

    // =========================================================================
    // Match Filter (Stale/Same-Sender Filtering)
    // =========================================================================

    /// Filter offers for a request (removes stale and same-sender)
    pub fn filter_offers_for_request<'a>(
        &self,
        offers: &'a [Offer],
        request: &Request,
    ) -> Vec<&'a Offer> {
        self.match_filter.filter_offers(offers, request)
    }

    /// Filter requests for an offer (removes stale and same-sender)
    pub fn filter_requests_for_offer<'a>(
        &self,
        requests: &'a [Request],
        offer: &Offer,
    ) -> Vec<&'a Request> {
        self.match_filter.filter_requests(requests, offer)
    }

    /// Get match filter statistics
    pub fn get_match_filter_stats(&self) -> MatchFilterStatsSnapshot {
        self.match_filter.get_stats()
    }

    /// Get match filter configuration
    pub fn get_match_filter_config(&self) -> MatchFilterConfig {
        self.match_filter.get_config()
    }

    /// Update match filter configuration
    pub fn set_match_filter_config(&self, config: MatchFilterConfig) {
        self.match_filter.set_config(config);
    }

    /// Enable or disable stale offer filtering
    pub fn enable_stale_filter(&self, enabled: bool) {
        self.match_filter.enable_stale_filter(enabled);
    }

    /// Enable or disable same-sender exclusion
    pub fn enable_same_sender_exclusion(&self, enabled: bool) {
        self.match_filter.enable_same_sender_exclusion(enabled);
    }

    /// Set maximum offer age for stale filtering
    pub fn set_max_offer_age_days(&self, days: i64) {
        self.match_filter.set_max_offer_age_days(days);
    }

    /// Reset match filter statistics
    pub fn reset_match_filter_stats(&self) {
        self.match_filter.reset_stats();
    }

    // =========================================================================
    // Embedding Cache (Medication Embeddings)
    // =========================================================================

    /// Refresh the embedding cache from medication mappings
    pub fn refresh_embedding_cache(&self, mappings: &[MedicationMapping]) {
        self.embedding_cache.refresh(mappings);
        tracing::info!(
            count = mappings.len(),
            "Refreshed embedding cache from medication mappings"
        );
    }

    /// Get embedding for a medication term
    pub fn get_medication_embedding(&self, term: &str) -> Option<Vec<f32>> {
        self.embedding_cache.get_embedding(term)
    }

    /// Check if two medication terms are synonyms
    pub fn are_medications_synonyms(&self, term1: &str, term2: &str) -> bool {
        self.embedding_cache.are_synonyms(term1, term2)
    }

    /// Get canonical name for a medication term
    pub fn get_canonical_medication(&self, term: &str) -> Option<String> {
        self.embedding_cache.get_canonical(term)
    }

    /// Get all synonyms for a medication term
    pub fn get_medication_synonyms(&self, term: &str) -> Vec<String> {
        self.embedding_cache.get_all_synonyms(term)
    }

    /// Get embedding cache statistics
    pub fn get_embedding_cache_stats(&self) -> EmbeddingCacheStatsSnapshot {
        self.embedding_cache.get_stats()
    }

    /// Check if embedding cache is empty
    pub fn is_embedding_cache_empty(&self) -> bool {
        self.embedding_cache.is_empty()
    }

    /// Clear the embedding cache
    pub fn clear_embedding_cache(&self) {
        self.embedding_cache.clear();
    }

    // =========================================================================
    // Audit Trail (Match Action Logging)
    // =========================================================================

    /// Log a match action to the audit trail
    pub async fn log_match_action(
        &self,
        match_entity: &MatchEntity,
        action: MatchAction,
        reason: &str,
    ) -> Result<(), super::AuditError> {
        // Convert MatchAction to ActionType
        let action_type = match action {
            MatchAction::AutoConfirm => super::ActionType::AutoConfirm,
            MatchAction::SuggestToOperator => super::ActionType::SuggestToOperator,
            MatchAction::QueueForReview => super::ActionType::QueueForReview,
            MatchAction::Ignore => super::ActionType::Ignore,
        };

        self.audit_trail
            .log_match_action(
                &match_entity.id,
                &match_entity.offer_id,
                &match_entity.request_id,
                action_type,
                match_entity.score,
                match_entity.status.clone(),
                reason,
                Some(serde_json::json!({
                    "reasoning": &match_entity.reasoning,
                })),
            )
            .await
    }

    /// Log a configuration change to the audit trail
    pub async fn log_config_change(
        &self,
        config_type: &str,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        actor: &str,
    ) -> Result<(), super::AuditError> {
        self.audit_trail
            .log_config_change(config_type, old_value, new_value, actor)
            .await
    }

    /// Log a calibration reset to the audit trail
    pub async fn log_calibration_reset(
        &self,
        actor: &str,
        reason: &str,
    ) -> Result<(), super::AuditError> {
        self.audit_trail.log_calibration_reset(actor, reason).await
    }

    /// Get match history from audit trail
    pub async fn get_match_audit_history(
        &self,
        match_id: &str,
    ) -> Result<Vec<super::AuditEntry>, super::AuditError> {
        self.audit_trail.get_match_history(match_id).await
    }

    /// Get recent audit actions
    pub async fn get_recent_audit_actions(
        &self,
        limit: usize,
    ) -> Result<Vec<super::AuditEntry>, super::AuditError> {
        self.audit_trail.get_recent_actions(limit).await
    }

    /// Get audit trail configuration
    pub fn get_audit_trail_config(&self) -> AuditTrailConfig {
        self.audit_trail.get_config()
    }

    /// Update audit trail configuration
    pub fn set_audit_trail_config(&self, config: AuditTrailConfig) {
        self.audit_trail.set_config(config);
    }

    /// Enable or disable audit trail
    pub fn enable_audit_trail(&self, enabled: bool) {
        self.audit_trail.enable(enabled);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;
    use crate::domain::MatchStatus;

    #[tokio::test]
    async fn test_create_matching_engine() {
        let engine = MatchingEngine::default();

        let weights = engine.get_weights();
        assert!(weights.medication > 0.0);
    }

    #[tokio::test]
    async fn test_score_match_basic() {
        let engine = MatchingEngine::default();

        let offer = Offer {
            medication: "Aspirin 100mg".to_string(),
            quantity: 100.0,
            price: 50.0,
            created_at: Utc::now(),
            ..Default::default()
        };

        let request = Request {
            medication: "Aspirin 100mg".to_string(),
            quantity: 100.0,
            max_price: 60.0,
            ..Default::default()
        };

        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        assert!(score.total > 0.0);
        assert!(score.medication_score == 0.95);
    }

    #[tokio::test]
    async fn test_record_feedback() {
        let engine = MatchingEngine::default();

        let result = engine.record_feedback("user-1", true, 0.85).await;
        assert!(result.is_ok());

        let count = engine.get_sample_count().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_ab_test_integration() {
        let engine = MatchingEngine::default();

        let test_config = ABTestConfig {
            test_id: "test-1".to_string(),
            name: "Weight Test".to_string(),
            description: "Testing new weights".to_string(),
            control_pct: 0.5,
            test_weights: Weights {
                medication: 0.50,
                dosage: 0.20,
                quantity: 0.10,
                price: 0.10,
                recency: 0.10,
            },
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now() + Duration::hours(1),
            min_samples: 10,
            active: true,
        };

        let result = engine.create_ab_test(test_config);
        assert!(result.is_ok());

        let active = engine.get_active_ab_tests();
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn test_warm_start_integration() {
        let engine = MatchingEngine::default();

        // With 0 samples, prior influence should be 100%
        let influence = engine.get_prior_influence().await;
        assert!((influence - 100.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_outlier_detection_integration() {
        let engine = MatchingEngine::default();

        // Add some normal scores
        for _ in 0..30 {
            engine.record_feedback("user", true, 0.75).await.ok();
        }

        let (mean, _std_dev, count) = engine.get_outlier_stats();
        assert_eq!(count, 30);
        assert!(mean > 0.7);
    }

    #[tokio::test]
    async fn test_config_update() {
        let engine = MatchingEngine::default();

        let new_config = MatchingEngineConfig {
            weights: Weights {
                medication: 0.50,
                dosage: 0.20,
                quantity: 0.10,
                price: 0.10,
                recency: 0.10,
            },
            ..Default::default()
        };

        engine.update_config(new_config).await;

        let weights = engine.get_weights();
        assert!((weights.medication - 0.50).abs() < 0.001);
    }

    // =========================================================================
    // Match Filter Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_match_filter_integration() {
        let engine = MatchingEngine::default();

        let request = Request {
            id: "req-1".to_string(),
            source_phone: "buyer-phone".to_string(),
            ..Default::default()
        };

        let offers = vec![
            Offer {
                id: "offer-1".to_string(),
                source_phone: "seller-1".to_string(),
                created_at: Utc::now(),
                ..Default::default()
            },
            Offer {
                id: "offer-2".to_string(),
                source_phone: "buyer-phone".to_string(), // Same as buyer
                created_at: Utc::now(),
                ..Default::default()
            },
            Offer {
                id: "offer-3".to_string(),
                source_phone: "seller-3".to_string(),
                created_at: Utc::now() - Duration::days(10), // Stale
                ..Default::default()
            },
        ];

        let filtered = engine.filter_offers_for_request(&offers, &request);

        // Only offer-1 should pass (offer-2 same sender, offer-3 stale)
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "offer-1");
    }

    #[tokio::test]
    async fn test_match_filter_stats() {
        let engine = MatchingEngine::default();

        let request = Request {
            id: "req-1".to_string(),
            source_phone: "buyer".to_string(),
            ..Default::default()
        };

        let offers = vec![Offer {
            id: "offer-1".to_string(),
            source_phone: "seller".to_string(),
            created_at: Utc::now(),
            ..Default::default()
        }];

        engine.filter_offers_for_request(&offers, &request);

        let stats = engine.get_match_filter_stats();
        assert_eq!(stats.total_candidates, 1);
        assert_eq!(stats.passed_filters, 1);
    }

    // =========================================================================
    // Embedding Cache Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_embedding_cache_integration() {
        let engine = MatchingEngine::default();

        assert!(engine.is_embedding_cache_empty());

        let mappings = vec![MedicationMapping {
            id: "1".to_string(),
            arabic_name: "بروفين".to_string(),
            english_name: "Brufen".to_string(),
            synonyms: Some(vec!["Ibuprofen".to_string()]),
            embedding: Some(pgvector::Vector::from(vec![0.1, 0.2, 0.3])),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        engine.refresh_embedding_cache(&mappings);

        assert!(!engine.is_embedding_cache_empty());

        // Test embedding retrieval
        let emb = engine.get_medication_embedding("Brufen");
        assert!(emb.is_some());

        // Test synonym check
        assert!(engine.are_medications_synonyms("Brufen", "Ibuprofen"));
        assert!(engine.are_medications_synonyms("بروفين", "Brufen"));

        // Test canonical name
        let canonical = engine.get_canonical_medication("Ibuprofen");
        assert_eq!(canonical, Some("brufen".to_string()));
    }

    // =========================================================================
    // Audit Trail Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_audit_trail_integration() {
        let engine = MatchingEngine::default();

        let match_entity = MatchEntity {
            id: "match-123".to_string(),
            offer_id: "offer-456".to_string(),
            request_id: "request-789".to_string(),
            score: 0.95,
            status: MatchStatus::Confirmed,
            reasoning: "High confidence match".to_string(),
            ..Default::default()
        };

        // Log a match action
        let result = engine
            .log_match_action(&match_entity, MatchAction::AutoConfirm, "Test action")
            .await;
        assert!(result.is_ok());

        // Get match history
        let history = engine.get_match_audit_history("match-123").await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].match_id, Some("match-123".to_string()));
    }

    #[tokio::test]
    async fn test_audit_trail_config_change() {
        let engine = MatchingEngine::default();

        let result = engine
            .log_config_change(
                "confidence_threshold",
                serde_json::json!(0.7),
                serde_json::json!(0.8),
                "admin",
            )
            .await;
        assert!(result.is_ok());

        let recent = engine.get_recent_audit_actions(10).await.unwrap();
        assert_eq!(recent.len(), 1);
    }
}
