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
//! - HistoricalLearner (medication pair affinity learning)

use crate::repository::{AuditLogRepository, FeedbackRepository};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};

use super::{
    ABTestConfig,
    ABTestManager,
    ABTestResult,
    AIClient,
    AIReviewer,
    ArabicPhoneticMatcher,
    AuditRecorder,
    AuditTrail,
    AuditTrailConfig,
    AutoActionHandler,
    AutoApproveConfig,
    AutoApproveProcessor,
    CalibrationConfig,
    CalibrationReport,
    ConfidenceCalibrator,
    ConfidenceConfig,
    ConfidenceManager,
    ConfidenceManagerStats,
    EmbeddingCache,
    EmbeddingCacheStatsSnapshot,
    ExpiryConfig,
    ExpiryResult,
    ExpiryScorer,
    HardNegativeIndex,
    HistoricalLearner,
    HistoricalLearnerStats,
    HistoricalLearningConfig,
    JobStatus,
    LearnerError,
    MatchAction,
    MatchFilter,
    MatchFilterConfig,
    MatchFilterStatsSnapshot,
    MatchScore,
    MedicationAffinity,
    MedicationBlocklist,
    // Supervision audit trail for AI decision logging (Feature: ai-supervision-persistence)
    MemorySupervisionAuditRepository,
    OutlierDetector,
    OutlierDetectorConfig,
    PauseReason,
    PerformanceMetrics,
    PersistentAuditRecorder,
    ReviewResult,
    ReviewStatus,
    SchedulerConfig,
    SchedulerStatus,
    Scorer,
    SharedEventEmitter,
    SupervisionAuditConfig,
    SupervisionAuditTrail,
    UncertaintyConfig,
    UncertaintyEstimator,
    WarmStartConfig,
    WarmStartManager,
    WeightLearner,
    Weights,
    contains_arabic,
};

/// Runtime state for the learning scheduler job
#[derive(Debug, Default)]
struct SchedulerState {
    last_run: Option<DateTime<Utc>>,
    last_status: JobStatus,
    last_error: Option<String>,
    last_metrics: Option<PerformanceMetrics>,
    run_count: u64,
    success_count: u64,
    failure_count: u64,
}

/// Public scheduler statistics for monitoring
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerStats {
    pub run_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_run: Option<DateTime<Utc>>,
    pub last_status: JobStatus,
}
use crate::domain::{AuditAction, AuditLog, EntityType, FeedbackStats, MedicationMaster};
use crate::domain::{Match as MatchEntity, Offer, Request};
use crate::notify::MatchNotifier;

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
    /// Historical learning settings
    pub historical: HistoricalLearningConfig,
    /// Expiry scorer settings (Requirements 5.1, 5.2, 5.4, 5.5)
    pub expiry: ExpiryConfig,
}

/// Result of class mismatch detection (Requirement 3.2)
///
/// When embedding similarity is high (>0.8) but therapeutic classes differ,
/// this indicates a potentially suspicious match that should be flagged for review.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ClassMismatchResult {
    /// Whether a class mismatch was detected
    pub is_mismatch: bool,
    /// The therapeutic class of the offer medication (if known)
    pub offer_class: Option<String>,
    /// The therapeutic class of the request medication (if known)
    pub request_class: Option<String>,
    /// Whether this match should be flagged as suspicious
    pub suspicious: bool,
    /// Reason for the suspicious flag (if any)
    pub reason: Option<String>,
}

impl ClassMismatchResult {
    /// Create a result indicating no mismatch
    pub fn no_mismatch() -> Self {
        Self::default()
    }

    /// Create a result indicating a class mismatch
    pub fn mismatch(offer_class: Option<String>, request_class: Option<String>) -> Self {
        let reason = match (&offer_class, &request_class) {
            (Some(oc), Some(rc)) => Some(format!(
                "High embedding similarity but different therapeutic classes: '{}' vs '{}'",
                oc, rc
            )),
            (Some(oc), None) => Some(format!(
                "High embedding similarity but request medication has unknown class (offer: '{}')",
                oc
            )),
            (None, Some(rc)) => Some(format!(
                "High embedding similarity but offer medication has unknown class (request: '{}')",
                rc
            )),
            (None, None) => None,
        };

        Self {
            is_mismatch: true,
            offer_class,
            request_class,
            suspicious: true,
            reason,
        }
    }
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
    /// Historical pattern learner
    historical_learner: HistoricalLearner,
    /// Audit recorder for debugging/replay
    audit_recorder: AuditRecorder,
    /// Persistent audit recorder for database-backed audit storage
    /// Feature: debug-recordings-persistence (Requirements 2.1)
    persistent_audit_recorder:
        Option<Arc<PersistentAuditRecorder<pharma_db::repo::SeaOrmMatchAuditRecordRepo>>>,
    /// Uncertainty estimator
    uncertainty_estimator: UncertaintyEstimator,
    /// Medication blocklist for dangerous pairs (Requirements 3.1, 3.5)
    blocklist: RwLock<MedicationBlocklist>,
    /// Expiry scorer for expiry date validation (Requirements 5.1, 5.2, 5.4, 5.5)
    expiry_scorer: RwLock<ExpiryScorer>,
    /// Arabic phonetic matcher for dual-language support (Requirements 2.5)
    arabic_matcher: ArabicPhoneticMatcher,
    /// Therapeutic class index for class mismatch detection (Requirement 3.2)
    class_index: RwLock<HardNegativeIndex>,
    /// Configuration
    config: RwLock<MatchingEngineConfig>,
    /// Current sample count for warm start
    sample_count: RwLock<usize>,
    /// Cron scheduler handle
    scheduler_handle: RwLock<Option<JobScheduler>>,
    /// Scheduler runtime state (job tracking)
    scheduler_state: Arc<RwLock<SchedulerState>>,
    /// Auto action handler
    pub auto_action: AutoActionHandler,
    /// Notification sender
    pub notifier: Arc<dyn MatchNotifier>,
    /// Repository for fetching feedback
    feedback_repo: Option<Arc<dyn FeedbackRepository>>,
    /// Repository for audit logging
    audit_log_repo: Option<Arc<dyn AuditLogRepository>>,
    /// AI client for semantic operations
    pub ai_client: Arc<AIClient>,
    /// AI expert reviewer
    pub ai_reviewer: Arc<AIReviewer>,
    /// Auto-approve processor for AI-supervised auto-approval
    /// Requirements: 1.1, 1.2, 1.3, 1.4, 6.1, 6.2, 7.1-7.5
    auto_approve_processor: Arc<AutoApproveProcessor>,
    /// Pipeline event emitter for real-time WebSocket updates
    /// Feature: debug-recording-enhancement (Requirements 2.1, 2.2, 2.3, 2.4)
    event_emitter: Option<SharedEventEmitter>,
    /// Supervision audit trail for AI decision logging
    /// Feature: ai-supervision-persistence (Requirements 1.1, 2.1, 2.2, 2.3)
    supervision_audit: Arc<SupervisionAuditTrail<MemorySupervisionAuditRepository>>,
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
        let historical_learner = HistoricalLearner::new(config.historical.clone());
        let audit_recorder = AuditRecorder::from_env();
        let uncertainty_estimator =
            UncertaintyEstimator::new(UncertaintyConfig::from_env(), config.weights.clone());

        // Initialize new safety components (Requirements 1.1, 3.1, 5.1)
        let blocklist = RwLock::new(MedicationBlocklist::with_defaults());
        let expiry_scorer = RwLock::new(ExpiryScorer::new(config.expiry.clone()));

        // Initialize Arabic phonetic matcher for dual-language support (Requirement 2.5)
        let arabic_matcher = ArabicPhoneticMatcher::new();

        // Initialize therapeutic class index for class mismatch detection (Requirement 3.2)
        let class_index = RwLock::new(HardNegativeIndex::new());

        let ai_client = Arc::new(AIClient::from_env());
        let ai_reviewer = Arc::new(AIReviewer::new(ai_client.clone()));

        // Initialize auto-approve processor for AI-supervised auto-approval
        // Requirements: 1.1, 1.2, 1.3, 1.4, 6.1, 6.2, 7.1-7.5
        let blocklist_for_processor = Arc::new(MedicationBlocklist::with_defaults());
        let auto_approve_processor = Arc::new(AutoApproveProcessor::new(
            AutoApproveConfig::default(),
            ai_reviewer.clone(),
            blocklist_for_processor,
        ));

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
            historical_learner,
            audit_recorder,
            persistent_audit_recorder: None, // Default to None, can be set via set_persistent_audit_recorder()
            uncertainty_estimator,
            blocklist,
            expiry_scorer,
            arabic_matcher,
            class_index,
            config: RwLock::new(config),
            sample_count: RwLock::new(0),
            scheduler_handle: RwLock::new(None),
            scheduler_state: Arc::new(RwLock::new(SchedulerState::default())),
            auto_action: AutoActionHandler::from_env(),
            notifier: Arc::new(crate::notify::NullNotifier), // Default to null, can be replaced
            feedback_repo: None,
            audit_log_repo: None,
            ai_client,
            ai_reviewer: ai_reviewer.clone(),
            auto_approve_processor,
            event_emitter: None, // Default to None, can be set via set_event_emitter
            // Initialize supervision audit trail with default config
            // Feature: ai-supervision-persistence (Requirements 1.1)
            supervision_audit: Arc::new(SupervisionAuditTrail::new(
                SupervisionAuditConfig::default(),
            )),
        }
    }

    /// Set the persistent audit recorder for database-backed audit storage
    /// Feature: debug-recordings-persistence (Requirements 2.1)
    pub fn set_persistent_audit_recorder(
        &mut self,
        recorder: Arc<PersistentAuditRecorder<pharma_db::repo::SeaOrmMatchAuditRecordRepo>>,
    ) {
        self.persistent_audit_recorder = Some(recorder);
    }

    /// Get the persistent audit recorder (if configured)
    /// Feature: debug-recordings-persistence
    pub fn get_persistent_audit_recorder(
        &self,
    ) -> Option<&Arc<PersistentAuditRecorder<pharma_db::repo::SeaOrmMatchAuditRecordRepo>>> {
        self.persistent_audit_recorder.as_ref()
    }

    /// Set repositories for the learning job
    pub fn set_repositories(
        &mut self,
        feedback_repo: Arc<dyn FeedbackRepository>,
        audit_log_repo: Arc<dyn AuditLogRepository>,
    ) {
        self.feedback_repo = Some(feedback_repo);
        self.audit_log_repo = Some(audit_log_repo);
    }

    /// Update the notifier
    pub fn set_notifier(&mut self, notifier: Arc<dyn MatchNotifier>) {
        self.notifier = notifier;
    }

    /// Set the pipeline event emitter for real-time WebSocket updates
    /// Feature: debug-recording-enhancement (Requirements 2.1, 2.2, 2.3, 2.4)
    pub fn set_event_emitter(&mut self, emitter: SharedEventEmitter) {
        self.event_emitter = Some(emitter);
    }

    /// Get the pipeline event emitter (if configured)
    pub fn get_event_emitter(&self) -> Option<&SharedEventEmitter> {
        self.event_emitter.as_ref()
    }

    /// Set the supervision audit trail for AI decision logging
    /// Feature: ai-supervision-persistence (Requirements 1.1)
    pub fn set_supervision_audit(
        &mut self,
        audit: Arc<SupervisionAuditTrail<MemorySupervisionAuditRepository>>,
    ) {
        self.supervision_audit = audit;
    }

    /// Get the supervision audit trail reference
    /// Feature: ai-supervision-persistence
    pub fn supervision_audit(
        &self,
    ) -> &Arc<SupervisionAuditTrail<MemorySupervisionAuditRepository>> {
        &self.supervision_audit
    }

    // =========================================================================
    // Audit Record Building
    // =========================================================================

    /// Build a MatchAuditRecord from scoring results
    /// Feature: debug-recordings-persistence (Requirements 1.2, 4.1, 4.4)
    ///
    /// Creates a complete audit record containing:
    /// - Unique identifier for deduplication
    /// - Session identifier (if provided)
    /// - Timestamps
    /// - Input parameters (offer/request snapshots)
    /// - Weights snapshot
    /// - Scoring results (breakdown and final score)
    pub fn build_audit_record(
        &self,
        offer: &Offer,
        request: &Request,
        score: &MatchScore,
        action: &MatchAction,
        session_id: Option<&str>,
    ) -> super::MatchAuditRecord {
        use super::MatchAuditRecord;

        let weights = self.scorer.get_weights();

        // Get pharmaceutical validation details
        let pharmaceutical_result = self
            .scorer
            .pharmaceutical_validator()
            .validate(offer, request);

        // Build score breakdown as JSON
        let score_breakdown = serde_json::json!({
            "medication_score": score.medication_score,
            "pharmaceutical_score": score.pharmaceutical_score,
            "dosage_score": score.dosage_score,
            "recency_score": score.recency_score,
            "ai_logic_score": score.ai_logic_score,
            "total": score.total,
            "confidence": format!("{:?}", score.confidence),
            "breakdown": score.breakdown,
            "action": format!("{:?}", action),
            "pharmaceutical_validation": {
                "passed": pharmaceutical_result.passed,
                "score": pharmaceutical_result.score,
                "concentration_check": pharmaceutical_result.concentration_check.as_ref().map(|c| serde_json::json!({
                    "offer_value": c.offer_value.as_ref().map(|v| serde_json::json!({
                        "numeric": v.numeric,
                        "unit": v.unit,
                        "original": v.original,
                    })),
                    "request_value": c.request_value.as_ref().map(|v| serde_json::json!({
                        "numeric": v.numeric,
                        "unit": v.unit,
                        "original": v.original,
                    })),
                    "difference_percent": c.difference_percent,
                    "penalty": c.penalty,
                    "compatible": c.compatible,
                })),
                "form_check": pharmaceutical_result.form_check.as_ref().map(|f| serde_json::json!({
                    "offer_form": f.offer_form,
                    "request_form": f.request_form,
                    "compatible": f.compatible,
                    "penalty": f.penalty,
                })),
                "rejection_reason": pharmaceutical_result.rejection_reason,
            },
        });

        // Build weights snapshot as JSON
        let weights_snapshot = serde_json::json!({
            "medication": weights.medication,
            "pharmaceutical": weights.pharmaceutical,
            "recency": weights.recency,
            "expiry": weights.expiry,
            "supplier": weights.supplier,
            "ai_logic": weights.ai_logic,
        });

        MatchAuditRecord {
            id: uuid::Uuid::new_v4(),
            match_id: uuid::Uuid::new_v4(), // Will be updated when match is created
            offer_id: offer.id,
            request_id: request.id,
            pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
            offer_snapshot: serde_json::to_value(offer).unwrap_or_default(),
            request_snapshot: serde_json::to_value(request).unwrap_or_default(),
            weights_snapshot,
            config_snapshot: None,
            score_breakdown,
            final_score: score.total,
            pipeline_stages: Vec::new(),
            ai_involved: score.ai_logic_score > 0.0,
            ai_record: None,
            resolution_stage: "scoring".to_string(),
            resolution_details: None,
            total_latency_ms: 0, // Will be set by caller if needed
            created_at: chrono::Utc::now(),
            review_status: None,
            reviewed_by: None,
            reviewed_at: None,
            review_notes: None,
            session_id: session_id.map(|s| s.to_string()),
            client_metadata: None,
        }
    }

    /// Build a MatchAuditRecord with AI involvement details
    /// Feature: debug-recordings-persistence (Requirements 1.3)
    pub fn build_audit_record_with_ai(
        &self,
        offer: &Offer,
        request: &Request,
        score: &MatchScore,
        action: &MatchAction,
        session_id: Option<&str>,
        ai_record: super::AIInvolvementRecord,
    ) -> super::MatchAuditRecord {
        let mut record = self.build_audit_record(offer, request, score, action, session_id);
        record.ai_involved = true;
        record.ai_record = Some(ai_record);
        record
    }

    // =========================================================================
    // Scoring
    // =========================================================================

    /// Score a match between offer and request
    /// Uses: Blocklist + Scorer + DosageGate + ExpiryScorer + WarmStart + ABTest + AutoAction + ConfidenceManager + HistoricalLearner
    pub async fn score_match(
        &self,
        offer: &Offer,
        request: &Request,
        medication_score: f64,
        user_id: Option<&str>,
    ) -> (MatchScore, MatchAction) {
        // 1. Check blocklist first - return 0.0 if blocked (Requirement 3.5)
        {
            let blocklist = self.blocklist.read().await;
            if let Some(entry) = blocklist.is_blocked(&offer.medication, &request.medication) {
                tracing::warn!(
                    offer_med = %offer.medication,
                    request_med = %request.medication,
                    reason = %entry.reason,
                    severity = ?entry.severity,
                    "Blocked medication pair detected - returning zero score"
                );
                let score = MatchScore {
                    total: 0.0,
                    medication_score,
                    pharmaceutical_score: 0.0,
                    dosage_score: 0.0,
                    recency_score: 0.0,
                    ai_logic_score: 0.0,
                    confidence: crate::domain::ConfidenceBand::None,
                    breakdown: "Blocked medication pair - zero score".to_string(),
                };
                return (score, MatchAction::Ignore);
            }
        }

        // Get weights (consider A/B test if user_id provided)
        let weights = self.get_weights_for_scoring(user_id).await;

        // Apply warm start blending
        let sample_count = *self.sample_count.read().await;
        let effective_weights = self
            .warm_start
            .get_effective_weights(&weights, sample_count);

        // Update scorer with effective weights
        self.scorer.update_weights(effective_weights);

        // Apply dual-language scoring for Arabic medication names (Requirement 2.5)
        let effective_medication_score = self.compute_dual_language_score(
            &offer.medication,
            &request.medication,
            medication_score,
        );

        // Score the match
        let mut score = self
            .scorer
            .score_match(offer, request, effective_medication_score, None);

        let expiry_scorer = self.expiry_scorer.read().await;
        // Use expiry_info (text) instead of expiry_date
        // For now, skip expiry scoring if we don't have structured date
        // TODO: Parse expiry_info to date when needed
        let expiry_result = ExpiryResult {
            score: 1.0,
            is_expired: false,
            days_until_expiry: None,
            warning: None,
        };

        if expiry_result.is_expired {
            // Expired offers get zero score
            score.total = 0.0;
            tracing::warn!(
                offer_id = %offer.id,
                days_past = ?expiry_result.days_until_expiry,
                "Expired offer detected - returning zero score"
            );
        } else if expiry_result.score < 1.0 {
            // Apply expiry penalty to total score
            let expiry_weight = expiry_scorer.config().weight;
            let expiry_penalty = (1.0 - expiry_result.score) * expiry_weight;
            score.total = (score.total - expiry_penalty).max(0.0);
            tracing::debug!(
                offer_id = %offer.id,
                expiry_score = expiry_result.score,
                days_remaining = ?expiry_result.days_until_expiry,
                "Near-expiry penalty applied"
            );
        }

        // Apply historical learning bonus/penalty
        let historical_bonus = self
            .historical_learner
            .get_historical_bonus(&offer.medication, &request.medication);
        if historical_bonus.abs() > 0.001 {
            score.total = (score.total + historical_bonus).clamp(0.0, 1.0);
            tracing::debug!(
                offer_med = %offer.medication,
                request_med = %request.medication,
                bonus = historical_bonus,
                "Applied historical learning bonus"
            );
        }

        // Evaluate confidence (tracks statistics and may adjust thresholds)
        let _meets_strict = self.confidence_manager.evaluate(score.total);

        // Determine action based on score
        let action = self.auto_action.determine_action(score.total).await;

        // Record audit data if persistent recorder is configured
        // Feature: debug-recordings-persistence (Requirements 1.1, 1.2, 1.4)
        if let Some(recorder) = &self.persistent_audit_recorder {
            let record = self.build_audit_record(offer, request, &score, &action, None);
            if !recorder.record(record).await {
                tracing::warn!(
                    offer_id = %offer.id,
                    request_id = %request.id,
                    "Failed to record audit - continuing with scoring result"
                );
            }
        }

        (score, action)
    }

    /// Score a match with optional AI-driven logic scoring
    pub async fn score_match_ai(
        &self,
        offer: &Offer,
        request: &Request,
        medication_score: f64,
        user_id: Option<&str>,
        use_ai_logic: bool,
    ) -> (MatchScore, MatchAction, Option<ReviewResult>) {
        let mut ai_score = None;
        let mut review_result = None;
        let mut ai_record: Option<super::AIInvolvementRecord> = None;

        if use_ai_logic {
            // Track AI call timing for audit record
            let ai_start = std::time::Instant::now();

            // Call AI for logic scoring
            let review_res = self
                .ai_reviewer
                .audit_match(
                    offer,
                    request,
                    medication_score,
                    "Internal AI Matching Request",
                )
                .await;

            let ai_latency_ms = ai_start.elapsed().as_millis() as u64;

            if let Ok(res) = review_res {
                ai_score = Some(match res.status {
                    ReviewStatus::Approved => (res.confidence as f64).max(0.95),
                    ReviewStatus::Flagged => (res.confidence as f64).min(0.70),
                    ReviewStatus::Rejected => 0.1,
                });

                // Build AI involvement record for audit
                // Feature: debug-recordings-persistence (Requirements 1.3)
                ai_record = Some(super::AIInvolvementRecord {
                    model: "ai-reviewer".to_string(),
                    prompt_tokens: None,
                    completion_tokens: None,
                    latency_ms: ai_latency_ms,
                    response: serde_json::json!({
                        "status": format!("{:?}", res.status),
                        "confidence": res.confidence,
                        "explanation": res.explanation,
                        "suggested_action": res.suggested_action,
                    }),
                });

                review_result = Some(res);
            }
        }

        // Get weights
        let weights = self.get_weights_for_scoring(user_id).await;
        self.scorer.update_weights(weights);

        // Score
        let score = self
            .scorer
            .score_match(offer, request, medication_score, ai_score);

        let action = self.auto_action.determine_action(score.total).await;

        // Record audit data with AI details if persistent recorder is configured
        // Feature: debug-recordings-persistence (Requirements 1.3)
        if let Some(recorder) = &self.persistent_audit_recorder {
            let record = if let Some(ai_rec) = ai_record {
                self.build_audit_record_with_ai(offer, request, &score, &action, None, ai_rec)
            } else {
                self.build_audit_record(offer, request, &score, &action, None)
            };
            if !recorder.record(record).await {
                tracing::warn!(
                    offer_id = %offer.id,
                    request_id = %request.id,
                    ai_used = use_ai_logic,
                    "Failed to record AI audit - continuing with scoring result"
                );
            }
        }

        (score, action, review_result)
    }

    /// Score a match with historical bonus and pharmaceutical validation details
    /// Returns the score, action, historical bonus, and pharmaceutical validation result
    pub async fn score_match_with_details(
        &self,
        offer: &Offer,
        request: &Request,
        medication_score: f64,
        user_id: Option<&str>,
    ) -> (
        MatchScore,
        MatchAction,
        f64,
        super::PharmaceuticalValidationResult,
    ) {
        let historical_bonus = self
            .historical_learner
            .get_historical_bonus(&offer.medication, &request.medication);

        // Get pharmaceutical validation details
        let pharmaceutical_result = self
            .scorer
            .pharmaceutical_validator()
            .validate(offer, request);

        let (score, action) = self
            .score_match(offer, request, medication_score, user_id)
            .await;

        (score, action, historical_bonus, pharmaceutical_result)
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
                    .notify_auto_confirmed(match_entity.id, match_entity.score)
                    .await?;
            }
            MatchAction::SuggestToOperator => {
                self.notifier.notify_suggested(match_entity).await?;
            }
            MatchAction::QueueForReview => {
                self.notifier
                    .notify_queued_for_review(match_entity.id, "low_confidence_match")
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

    /// Compute dual-language medication score (Requirement 2.5)
    ///
    /// If either medication name contains Arabic text, computes scores for both
    /// Arabic (using phonetic matching) and English representations, returning
    /// the maximum score from both.
    ///
    /// # Arguments
    /// * `offer_med` - The offer medication name
    /// * `request_med` - The request medication name
    /// * `base_score` - The base medication similarity score (e.g., from embeddings)
    ///
    /// # Returns
    /// The maximum of the base score and Arabic phonetic similarity (if applicable)
    fn compute_dual_language_score(
        &self,
        offer_med: &str,
        request_med: &str,
        base_score: f64,
    ) -> f64 {
        // Check if either medication contains Arabic text
        let offer_has_arabic = contains_arabic(offer_med);
        let request_has_arabic = contains_arabic(request_med);

        if !offer_has_arabic && !request_has_arabic {
            // No Arabic text - return base score
            return base_score;
        }

        // Compute Arabic phonetic similarity
        let arabic_score = self
            .arabic_matcher
            .phonetic_similarity(offer_med, request_med);

        // Return maximum of base score and Arabic phonetic score
        let max_score = base_score.max(arabic_score);

        if max_score > base_score {
            tracing::debug!(
                offer_med = %offer_med,
                request_med = %request_med,
                base_score = base_score,
                arabic_score = arabic_score,
                "Dual-language matching improved score"
            );
        }

        max_score
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

    /// Record operator feedback with medication names for historical learning
    /// Uses: OutlierDetector + ABTest + SampleCount + Calibrator + HistoricalLearner
    pub async fn record_feedback_with_medications(
        &self,
        user_id: &str,
        confirmed: bool,
        total_score: f64,
        offer_medication: &str,
        request_medication: &str,
    ) -> std::result::Result<(), String> {
        // Record basic feedback
        self.record_feedback(user_id, confirmed, total_score)
            .await?;

        // Record for historical learning (medication pair affinity)
        self.historical_learner
            .record_feedback(offer_medication, request_medication, confirmed);

        tracing::debug!(
            offer_med = offer_medication,
            request_med = request_medication,
            confirmed = confirmed,
            "Historical learning feedback recorded"
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
            sample_size = stats.total_feedback,
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

        // Add the learning job with state tracking
        let engine = self.clone();
        let state = self.scheduler_state.clone();
        let job = Job::new_async(schedule.as_str(), move |_uuid, _lock| {
            let engine = engine.clone();
            let state = state.clone();
            Box::pin(async move {
                tracing::info!("📅 Learning job triggered by scheduler");

                // Update state: job started
                {
                    let mut s = state.write().await;
                    s.run_count += 1;
                    s.last_run = Some(Utc::now());
                    s.last_status = JobStatus::Running;
                    s.last_error = None;
                }

                // Execute the learning job
                let result = engine.run_learning_job_internal().await;

                // Update state based on result
                {
                    let mut s = state.write().await;
                    match &result {
                        Ok(metrics) => {
                            s.last_status = JobStatus::Success;
                            s.last_metrics = Some(metrics.clone());
                            s.success_count += 1;
                            tracing::info!("✅ Learning job completed successfully");
                        }
                        Err(e) => {
                            s.last_status = JobStatus::Failed;
                            s.last_error = Some(e.to_string());
                            s.failure_count += 1;
                            tracing::error!(error = %e, "❌ Learning job failed");

                            // Record failure in metrics
                            metrics::counter!("learning_job_failures_total").increment(1);
                        }
                    }
                }

                // Record job completion metric
                metrics::counter!("learning_job_runs_total").increment(1);
            })
        })
        .map_err(|e| e.to_string())?;

        scheduler.add(job).await.map_err(|e| e.to_string())?;
        scheduler.start().await.map_err(|e| e.to_string())?;

        *self.scheduler_handle.write().await = Some(scheduler);

        tracing::info!(schedule = %schedule, "📅 Learning scheduler started");
        Ok(())
    }

    /// Internal learning job implementation (returns metrics for tracking)
    async fn run_learning_job_internal(&self) -> crate::Result<PerformanceMetrics> {
        let feedback_repo = self.feedback_repo.as_ref().ok_or_else(|| {
            crate::Error::Internal("Feedback repository not set in matching engine".to_owned())
        })?;

        // 1. Get stats for the last 30 days
        let end = Utc::now();
        let start = end - chrono::Duration::days(30);
        let stats = feedback_repo.get_stats(start, end).await?;

        if stats.total_feedback < 50 {
            tracing::info!(
                count = stats.total_feedback,
                "Not enough feedback for learning (minimum 50)"
            );
            // Return default metrics for skipped job
            return Ok(PerformanceMetrics {
                sample_size: stats.total_feedback,
                ..Default::default()
            });
        }

        // 2. Calculate new weights
        let (new_weights, metrics) = self
            .calculate_new_weights(&stats)
            .await
            .map_err(|e| crate::Error::Internal(format!("Weight calculation failed: {}", e)))?;

        // 3. Adjust thresholds based on confirmation rate
        let calculator = self.auto_action.calculator();
        let mut threshold_config = calculator.config().read().await.clone();

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
                "sample_size": stats.total_feedback
            }));
            let _ = audit_repo.save(&audit_log).await;
        }

        // 6. Record success metrics
        metrics::gauge!("learning_job_confirmation_rate").set(metrics.confirmation_rate);
        metrics::gauge!("learning_job_sample_size").set(stats.total_feedback as f64);

        Ok(metrics)
    }

    /// Primary learning job: updates weights and thresholds (public wrapper)
    pub async fn run_learning_job(&self) -> crate::Result<()> {
        self.run_learning_job_internal().await?;
        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop_scheduler(&self) {
        if let Some(mut scheduler) = self.scheduler_handle.write().await.take() {
            scheduler.shutdown().await.ok();
            tracing::info!("📅 Learning scheduler stopped");
        }
    }

    /// Get scheduler status with real-time job tracking
    pub async fn scheduler_status(&self) -> SchedulerStatus {
        let config = self.config.read().await;
        let state = self.scheduler_state.read().await;

        SchedulerStatus {
            enabled: config.scheduler.enabled,
            schedule: config.scheduler.schedule.clone(),
            last_run: state.last_run,
            last_status: state.last_status,
            last_error: state.last_error.clone(),
            last_metrics: state.last_metrics.clone(),
            pending_apply: None,
            pending_reason: None,
        }
    }

    /// Get detailed scheduler statistics
    pub async fn scheduler_stats(&self) -> SchedulerStats {
        let state = self.scheduler_state.read().await;
        SchedulerStats {
            run_count: state.run_count,
            success_count: state.success_count,
            failure_count: state.failure_count,
            last_run: state.last_run,
            last_status: state.last_status,
        }
    }

    /// Manually trigger the learning job (for testing or admin use)
    pub async fn trigger_learning_job(&self) -> crate::Result<PerformanceMetrics> {
        tracing::info!("📅 Learning job manually triggered");

        // Update state
        {
            let mut s = self.scheduler_state.write().await;
            s.run_count += 1;
            s.last_run = Some(Utc::now());
            s.last_status = JobStatus::Running;
        }

        let result = self.run_learning_job_internal().await;

        // Update state based on result
        {
            let mut s = self.scheduler_state.write().await;
            match &result {
                Ok(metrics) => {
                    s.last_status = JobStatus::Success;
                    s.last_metrics = Some(metrics.clone());
                    s.success_count += 1;
                }
                Err(e) => {
                    s.last_status = JobStatus::Failed;
                    s.last_error = Some(e.to_string());
                    s.failure_count += 1;
                }
            }
        }

        result
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

    /// Get audit recorder reference
    pub fn get_audit_recorder(&self) -> &AuditRecorder {
        &self.audit_recorder
    }

    /// Get uncertainty estimator reference
    pub fn get_uncertainty_estimator(&self) -> &UncertaintyEstimator {
        &self.uncertainty_estimator
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

        // Update historical learner
        self.historical_learner
            .set_config(config.historical.clone());

        // Update expiry scorer
        self.expiry_scorer
            .write()
            .await
            .set_config(config.expiry.clone());

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
    // Pharmaceutical Validator Configuration
    // =========================================================================

    /// Get pharmaceutical validator configuration
    pub fn get_pharmaceutical_validator_config(&self) -> super::PharmaceuticalValidatorConfig {
        self.scorer.get_pharmaceutical_config()
    }

    /// Set pharmaceutical validator configuration
    pub fn set_pharmaceutical_validator_config(
        &self,
        config: super::PharmaceuticalValidatorConfig,
    ) {
        tracing::info!(
            concentration_tolerance = config.concentration_tolerance_percent,
            concentration_reject_threshold = config.concentration_reject_threshold_percent,
            concentration_check_enabled = config.enable_concentration_check,
            form_check_enabled = config.enable_form_check,
            "Pharmaceutical validator configuration updated"
        );
        self.scorer.set_pharmaceutical_config(config);
    }

    /// Enable or disable concentration validation
    pub fn enable_concentration_check(&self, enabled: bool) {
        let mut config = self.scorer.get_pharmaceutical_config();
        config.enable_concentration_check = enabled;
        self.scorer.set_pharmaceutical_config(config);
        tracing::info!(enabled = enabled, "Concentration check toggled");
    }

    /// Enable or disable form validation
    pub fn enable_form_check(&self, enabled: bool) {
        let mut config = self.scorer.get_pharmaceutical_config();
        config.enable_form_check = enabled;
        self.scorer.set_pharmaceutical_config(config);
        tracing::info!(enabled = enabled, "Form check toggled");
    }

    /// Set concentration tolerance threshold
    pub fn set_concentration_tolerance(&self, tolerance_percent: f64) {
        let mut config = self.scorer.get_pharmaceutical_config();
        config.concentration_tolerance_percent = tolerance_percent;
        self.scorer.set_pharmaceutical_config(config);
        tracing::info!(
            tolerance_percent = tolerance_percent,
            "Concentration tolerance updated"
        );
    }

    /// Set concentration rejection threshold
    pub fn set_concentration_reject_threshold(&self, threshold_percent: f64) {
        let mut config = self.scorer.get_pharmaceutical_config();
        config.concentration_reject_threshold_percent = threshold_percent;
        self.scorer.set_pharmaceutical_config(config);
        tracing::info!(
            threshold_percent = threshold_percent,
            "Concentration rejection threshold updated"
        );
    }

    /// Get pharmaceutical validation statistics
    pub fn get_pharmaceutical_validator_stats(
        &self,
    ) -> super::PharmaceuticalValidationStatsSnapshot {
        self.scorer.pharmaceutical_validator().get_stats()
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

    /// Refresh the embedding cache from medication master records
    pub fn refresh_embedding_cache(&self, masters: &[MedicationMaster]) {
        self.embedding_cache.refresh(masters);
        tracing::info!(
            count = masters.len(),
            "Refreshed embedding cache from medication_master"
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
            .log_match_action(super::MatchActionParams {
                match_id: match_entity.id,
                offer_id: match_entity.offer_id,
                request_id: match_entity.request_id,
                action: action_type,
                score: match_entity.score,
                status: match_entity.status,
                reason: reason.to_string(),
                metadata: Some(serde_json::json!({
                    "reasoning": &match_entity.reasoning,
                })),
            })
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

    // =========================================================================
    // Historical Learning (Medication Pair Affinity)
    // =========================================================================

    /// Get historical learning statistics
    pub fn get_historical_stats(&self) -> HistoricalLearnerStats {
        self.historical_learner.get_stats()
    }

    /// Get historical learning configuration
    pub fn get_historical_config(&self) -> HistoricalLearningConfig {
        self.historical_learner.get_config()
    }

    /// Update historical learning configuration
    pub fn set_historical_config(&self, config: HistoricalLearningConfig) {
        self.historical_learner.set_config(config);
    }

    /// Enable or disable historical learning
    pub fn enable_historical_learning(&self, enabled: bool) {
        self.historical_learner.enable(enabled);
    }

    /// Check if historical learning is enabled
    pub fn is_historical_learning_enabled(&self) -> bool {
        self.historical_learner.is_enabled()
    }

    /// Get affinity score for a medication pair
    pub fn get_medication_affinity(&self, med_a: &str, med_b: &str) -> Option<f64> {
        self.historical_learner.get_affinity(med_a, med_b)
    }

    /// Get full affinity record for a medication pair
    pub fn get_medication_affinity_record(
        &self,
        med_a: &str,
        med_b: &str,
    ) -> Option<MedicationAffinity> {
        self.historical_learner.get_affinity_record(med_a, med_b)
    }

    /// Get top medication affinities (highest affinity pairs)
    pub fn get_top_medication_affinities(&self, limit: usize) -> Vec<MedicationAffinity> {
        self.historical_learner.get_top_affinities(limit)
    }

    /// Get bottom medication affinities (problematic pairs)
    pub fn get_bottom_medication_affinities(&self, limit: usize) -> Vec<MedicationAffinity> {
        self.historical_learner.get_bottom_affinities(limit)
    }

    /// Apply time decay to all medication affinities
    pub fn apply_historical_decay(&self) {
        self.historical_learner.apply_decay();
    }

    /// Clear all learned medication affinities
    pub fn clear_historical_learning(&self) {
        self.historical_learner.clear();
    }

    /// Load medication affinities from external source
    pub fn load_medication_affinities(&self, affinities: Vec<MedicationAffinity>) {
        self.historical_learner.load_affinities(affinities);
    }

    /// Export all medication affinities for persistence
    pub fn export_medication_affinities(&self) -> Vec<MedicationAffinity> {
        self.historical_learner.export_affinities()
    }

    /// Get number of tracked medication pairs
    pub fn medication_pair_count(&self) -> usize {
        self.historical_learner.pair_count()
    }

    // =========================================================================
    // Medication Blocklist (Dangerous Pair Prevention)
    // Requirements: 3.1, 3.5
    // =========================================================================

    /// Check if a medication pair is blocked
    pub async fn is_medication_pair_blocked(
        &self,
        med_a: &str,
        med_b: &str,
    ) -> Option<super::BlocklistEntry> {
        self.blocklist
            .read()
            .await
            .is_blocked(med_a, med_b)
            .cloned()
    }

    /// Add an entry to the medication blocklist
    pub async fn add_blocklist_entry(&self, entry: super::BlocklistEntry) {
        self.blocklist.write().await.add_entry(entry);
    }

    /// Remove an entry from the medication blocklist
    pub async fn remove_blocklist_entry(&self, med_a: &str, med_b: &str) -> bool {
        self.blocklist.write().await.remove_entry(med_a, med_b)
    }

    /// Get the number of blocklist entries
    pub async fn blocklist_len(&self) -> usize {
        self.blocklist.read().await.len()
    }

    /// Check if blocklist is empty
    pub async fn is_blocklist_empty(&self) -> bool {
        self.blocklist.read().await.is_empty()
    }

    /// Clear all blocklist entries
    pub async fn clear_blocklist(&self) {
        self.blocklist.write().await.clear();
    }

    /// Reload blocklist with default dangerous pairs
    pub async fn reload_default_blocklist(&self) {
        *self.blocklist.write().await = MedicationBlocklist::with_defaults();
    }

    // =========================================================================
    // Expiry Scorer (Expiry Date Validation)
    // Requirements: 5.1, 5.2, 5.4, 5.5
    // =========================================================================

    /// Score an offer's expiry date
    pub async fn score_expiry(&self, expiry_date: Option<DateTime<Utc>>) -> ExpiryResult {
        self.expiry_scorer
            .read()
            .await
            .score(expiry_date, Utc::now())
    }

    /// Check if an offer meets minimum shelf life requirement
    pub async fn meets_shelf_life(
        &self,
        expiry_date: Option<DateTime<Utc>>,
        min_days: u32,
    ) -> bool {
        self.expiry_scorer
            .read()
            .await
            .meets_shelf_life(expiry_date, min_days)
    }

    /// Check if an offer is expired
    pub async fn is_offer_expired(&self, expiry_date: Option<DateTime<Utc>>) -> bool {
        self.expiry_scorer
            .read()
            .await
            .is_expired(expiry_date, Utc::now())
    }

    /// Get expiry scorer configuration
    pub async fn get_expiry_config(&self) -> ExpiryConfig {
        self.expiry_scorer.read().await.config().clone()
    }

    /// Update expiry scorer configuration
    pub async fn set_expiry_config(&self, config: ExpiryConfig) {
        self.expiry_scorer.write().await.set_config(config);
    }

    // =========================================================================
    // Arabic Phonetic Matching (Dual-Language Support)
    // Requirements: 2.5
    // =========================================================================

    /// Get Arabic phonetic similarity between two medication names
    ///
    /// Returns a similarity score between 0.0 and 1.0, where 1.0 indicates
    /// identical phonetic keys.
    pub fn get_arabic_phonetic_similarity(&self, med_a: &str, med_b: &str) -> f64 {
        self.arabic_matcher.phonetic_similarity(med_a, med_b)
    }

    /// Check if two medication names are phonetically equivalent in Arabic
    pub fn are_arabic_phonetic_match(&self, med_a: &str, med_b: &str) -> bool {
        self.arabic_matcher.is_phonetic_match(med_a, med_b)
    }

    /// Check if a medication name contains Arabic text
    pub fn medication_contains_arabic(&self, medication: &str) -> bool {
        contains_arabic(medication)
    }

    /// Get the canonical Arabic form of a medication name (if known)
    pub fn get_arabic_canonical(&self, medication: &str) -> Option<String> {
        self.arabic_matcher
            .get_canonical(medication)
            .map(|s| s.to_string())
    }

    /// Compute dual-language medication score
    ///
    /// If either medication name contains Arabic text, computes scores for both
    /// Arabic (using phonetic matching) and English representations, returning
    /// the maximum score from both.
    pub fn get_dual_language_score(
        &self,
        offer_med: &str,
        request_med: &str,
        base_score: f64,
    ) -> f64 {
        self.compute_dual_language_score(offer_med, request_med, base_score)
    }

    // =========================================================================
    // Class Mismatch Detection (Requirement 3.2)
    // =========================================================================

    /// Threshold for high embedding similarity that triggers class mismatch check
    const CLASS_MISMATCH_SIMILARITY_THRESHOLD: f64 = 0.8;

    /// Detect class mismatch when embedding similarity is high (>0.8)
    ///
    /// When two medications have high embedding similarity but belong to different
    /// therapeutic classes, this indicates a potentially dangerous match that should
    /// be flagged for review.
    ///
    /// # Arguments
    /// * `offer_med` - The offer medication name
    /// * `request_med` - The request medication name
    /// * `embedding_similarity` - The embedding similarity score (0.0-1.0)
    ///
    /// # Returns
    /// A `ClassMismatchResult` indicating whether a mismatch was detected
    pub async fn detect_class_mismatch(
        &self,
        offer_med: &str,
        request_med: &str,
        embedding_similarity: f64,
    ) -> ClassMismatchResult {
        // Only check for class mismatch when embedding similarity is high
        if embedding_similarity < Self::CLASS_MISMATCH_SIMILARITY_THRESHOLD {
            return ClassMismatchResult::no_mismatch();
        }

        let index = self.class_index.read().await;

        // Get therapeutic classes for both medications
        let offer_class = index.get_class(offer_med).cloned();
        let request_class = index.get_class(request_med).cloned();

        // Check for mismatch
        match (&offer_class, &request_class) {
            (Some(oc), Some(rc)) if oc != rc => {
                tracing::warn!(
                    offer_med = %offer_med,
                    request_med = %request_med,
                    offer_class = %oc,
                    request_class = %rc,
                    embedding_similarity = embedding_similarity,
                    validation_type = "therapeutic_class_mismatch",
                    severity = "high",
                    "Therapeutic class mismatch detected with high embedding similarity - potential safety risk"
                );
                ClassMismatchResult::mismatch(Some(oc.clone()), Some(rc.clone()))
            }
            (Some(_), None) | (None, Some(_)) => {
                // One medication has unknown class - flag as suspicious if similarity is very high
                if embedding_similarity > 0.9 {
                    tracing::warn!(
                        offer_med = %offer_med,
                        request_med = %request_med,
                        offer_class = ?offer_class,
                        request_class = ?request_class,
                        embedding_similarity = embedding_similarity,
                        validation_type = "therapeutic_class_partial",
                        severity = "medium",
                        "High similarity with incomplete class information - flagging for manual review"
                    );
                    ClassMismatchResult::mismatch(offer_class, request_class)
                } else {
                    ClassMismatchResult::no_mismatch()
                }
            }
            _ => ClassMismatchResult::no_mismatch(),
        }
    }

    /// Add a medication to the therapeutic class index
    ///
    /// This allows the engine to track therapeutic classes for medications
    /// and detect class mismatches during matching.
    ///
    /// # Arguments
    /// * `medication` - The medication name
    /// * `therapeutic_class` - The therapeutic class (e.g., "Antidiabetic", "Beta-blocker")
    pub async fn add_medication_class(&self, medication: &str, therapeutic_class: &str) {
        let mut index = self.class_index.write().await;
        index.add_medication(medication, Some(therapeutic_class));
    }

    /// Get the therapeutic class for a medication
    ///
    /// # Arguments
    /// * `medication` - The medication name
    ///
    /// # Returns
    /// The therapeutic class if known, None otherwise
    pub async fn get_medication_class(&self, medication: &str) -> Option<String> {
        let index = self.class_index.read().await;
        index.get_class(medication).cloned()
    }

    /// Check if the class index has been populated
    pub async fn is_class_index_ready(&self) -> bool {
        let index = self.class_index.read().await;
        index.is_built()
    }

    /// Get the number of medications in the class index
    pub async fn class_index_medication_count(&self) -> usize {
        let index = self.class_index.read().await;
        index.medication_count()
    }

    /// Get the number of therapeutic classes in the index
    pub async fn class_index_class_count(&self) -> usize {
        let index = self.class_index.read().await;
        index.class_count()
    }

    /// Mark the class index as built (ready for use)
    pub async fn mark_class_index_built(&self) {
        let mut index = self.class_index.write().await;
        index.mark_built();
    }

    /// Clear the class index
    pub async fn clear_class_index(&self) {
        let mut index = self.class_index.write().await;
        index.clear();
    }

    /// Bulk load medications into the class index
    ///
    /// # Arguments
    /// * `medications` - List of (medication_name, therapeutic_class) pairs
    pub async fn load_medication_classes(&self, medications: &[(String, String)]) {
        let mut index = self.class_index.write().await;
        for (med, class) in medications {
            index.add_medication(med, Some(class));
        }
        index.mark_built();
        tracing::info!(
            count = medications.len(),
            "Loaded medication classes into index"
        );
    }

    // =========================================================================
    // AI Supervision Auto-Approve Methods
    // Requirements: 3.2, 4.1, 4.2, 5.1
    // =========================================================================

    /// Get auto-approve statistics
    /// Requirements: 3.2
    pub async fn get_auto_approve_stats(&self) -> Result<super::AutoApproveStats, String> {
        Ok(self.auto_approve_processor.get_stats().await)
    }

    /// Get auto-approve configuration
    /// Requirements: 5.1
    pub async fn get_auto_approve_config(&self) -> Result<super::AutoApproveConfig, String> {
        Ok(self.auto_approve_processor.get_config().await)
    }

    /// Update auto-approve configuration
    /// Requirements: 5.1
    pub async fn update_auto_approve_config(
        &self,
        config: super::AutoApproveConfig,
    ) -> Result<(), String> {
        self.auto_approve_processor
            .update_config(config)
            .await
            .map_err(|e| format!("Failed to update config: {}", e))
    }

    /// Get supervision audit log
    /// Feature: ai-supervision-persistence (Requirements 2.1, 2.2, 2.3, 2.4)
    pub async fn get_supervision_audit_log(
        &self,
        filter: &super::SupervisionAuditFilter,
    ) -> Result<Vec<super::SupervisionAuditEntry>, String> {
        self.supervision_audit
            .query(filter)
            .await
            .map_err(|e| format!("Failed to query supervision audit log: {}", e))
    }

    /// Override an auto-approve decision
    /// Feature: ai-supervision-persistence (Requirements 1.5, 3.5, 4.1)
    pub async fn override_auto_approve_decision(
        &self,
        match_id: uuid::Uuid,
        user_id: uuid::Uuid,
        reason: &str,
        original_confidence: f64,
        original_explanation: &str,
    ) -> Result<(), String> {
        // Log to supervision audit trail
        if let Err(e) = self
            .supervision_audit
            .log_override(
                match_id,
                user_id,
                reason.to_string(),
                original_confidence,
                original_explanation.to_string(),
            )
            .await
        {
            tracing::warn!(
                error = %e,
                match_id = %match_id,
                "Failed to log override to supervision audit trail"
            );
        }

        // Record the override in the processor for tracking
        self.auto_approve_processor.record_override().await;

        tracing::info!(
            match_id = %match_id,
            user_id = %user_id,
            reason = %reason,
            original_confidence = %original_confidence,
            "Auto-approve decision overridden"
        );
        Ok(())
    }

    /// Undo an auto-approval
    /// Feature: ai-supervision-persistence (Requirements 1.6, 3.5, 4.2)
    pub async fn undo_auto_approval(
        &self,
        match_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<(), String> {
        // Log to supervision audit trail
        if let Err(e) = self.supervision_audit.log_undo(match_id, user_id).await {
            tracing::warn!(
                error = %e,
                match_id = %match_id,
                "Failed to log undo to supervision audit trail"
            );
        }

        tracing::info!(
            match_id = %match_id,
            user_id = %user_id,
            "Auto-approval undone"
        );
        Ok(())
    }

    /// Pause the auto-approve system
    pub async fn pause_auto_approve(
        &self,
        user_id: uuid::Uuid,
        reason: &str,
    ) -> Result<(), String> {
        self.auto_approve_processor
            .pause(PauseReason::ManualPause {
                user_id,
                reason: reason.to_string(),
            })
            .await;

        tracing::info!(
            user_id = %user_id,
            reason = %reason,
            "Auto-approve system paused"
        );
        Ok(())
    }

    /// Resume the auto-approve system
    pub async fn resume_auto_approve(&self) -> Result<(), String> {
        self.auto_approve_processor.resume().await;
        tracing::info!("Auto-approve system resumed");
        Ok(())
    }

    /// Get the auto-approve processor for direct access
    /// Used by background jobs and integration points
    pub fn get_auto_approve_processor(&self) -> Arc<AutoApproveProcessor> {
        self.auto_approve_processor.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Offer, Request};
    use chrono::Utc;

    /// Helper function to create a test offer
    fn create_test_offer(
        medication: &str,
        concentration: Option<&str>,
        form: Option<&str>,
    ) -> Offer {
        Offer {
            id: uuid::Uuid::new_v4(),
            medication: medication.to_string(),
            concentration: concentration.map(|s| s.to_string()),
            form: form.map(|s| s.to_string()),
            participant_id: uuid::Uuid::new_v4(),
            group_id: uuid::Uuid::new_v4(),
            raw_message_id: uuid::Uuid::new_v4(),
            created_at: Utc::now(),
            ..Default::default()
        }
    }

    /// Helper function to create a test request
    fn create_test_request(
        medication: &str,
        concentration: Option<&str>,
        form: Option<&str>,
    ) -> Request {
        Request {
            id: uuid::Uuid::new_v4(),
            medication: medication.to_string(),
            concentration: concentration.map(|s| s.to_string()),
            form: form.map(|s| s.to_string()),
            participant_id: uuid::Uuid::new_v4(),
            group_id: uuid::Uuid::new_v4(),
            raw_message_id: uuid::Uuid::new_v4(),
            created_at: Utc::now(),
            ..Default::default()
        }
    }

    // =========================================================================
    // Task 7.2.1: Test end-to-end scoring with pharmaceutical validation
    // =========================================================================

    #[tokio::test]
    async fn test_end_to_end_scoring_with_pharmaceutical_validation() {
        let engine = MatchingEngine::default();

        // Create matching offer and request with identical pharmaceutical properties
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score the match
        let (score, action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is included and high (identical properties)
        assert!(
            score.pharmaceutical_score > 0.95,
            "Expected high pharmaceutical score for identical properties, got {}",
            score.pharmaceutical_score
        );

        // Verify total score includes pharmaceutical component
        assert!(
            score.total > 0.0,
            "Expected positive total score, got {}",
            score.total
        );

        // Verify breakdown includes pharmaceutical score
        assert!(
            score.breakdown.contains("Pharma:"),
            "Expected breakdown to include pharmaceutical score"
        );

        // Verify action is reasonable for high score
        assert!(
            matches!(
                action,
                MatchAction::AutoConfirm | MatchAction::SuggestToOperator
            ),
            "Expected AutoConfirm or SuggestToOperator for high score, got {:?}",
            action
        );
    }

    // =========================================================================
    // Task 7.2.2: Test concentration mismatch reduces score appropriately
    // =========================================================================

    #[tokio::test]
    async fn test_concentration_mismatch_reduces_score() {
        let engine = MatchingEngine::default();

        // Create offer and request with significant concentration difference (150mg vs 15mg = 900% difference)
        let offer = create_test_offer("Aspirin", Some("150mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("15mg"), Some("اقراص"));

        // Score the match with high medication similarity
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is significantly reduced due to concentration mismatch
        assert!(
            score.pharmaceutical_score < 0.5,
            "Expected low pharmaceutical score for large concentration mismatch, got {}",
            score.pharmaceutical_score
        );

        // Verify total score is reduced (but may still be relatively high due to other factors)
        assert!(
            score.total < 0.9,
            "Expected reduced total score for concentration mismatch, got {}",
            score.total
        );
    }

    #[tokio::test]
    async fn test_moderate_concentration_difference_applies_penalty() {
        let engine = MatchingEngine::default();

        // Create offer and request with moderate concentration difference (100mg vs 75mg = 33% difference)
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("75mg"), Some("اقراص"));

        // Score the match
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is reduced but not zero (graduated penalty)
        assert!(
            score.pharmaceutical_score > 0.3 && score.pharmaceutical_score < 0.9,
            "Expected moderate pharmaceutical score for 33% concentration difference, got {}",
            score.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_missing_concentration_applies_moderate_penalty() {
        let engine = MatchingEngine::default();

        // Create offer with concentration, request without
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", None, Some("اقراص"));

        // Score the match
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is reduced moderately (default 15% penalty)
        assert!(
            score.pharmaceutical_score > 0.7 && score.pharmaceutical_score < 1.0,
            "Expected moderate penalty for missing concentration, got {}",
            score.pharmaceutical_score
        );
    }

    // =========================================================================
    // Task 7.2.3: Test form incompatibility reduces score appropriately
    // =========================================================================

    #[tokio::test]
    async fn test_form_incompatibility_reduces_score() {
        let engine = MatchingEngine::default();

        // Create offer and request with incompatible forms (امبول vs اقراص)
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("امبول"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score the match
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is significantly reduced due to form incompatibility
        assert!(
            score.pharmaceutical_score < 0.5,
            "Expected low pharmaceutical score for incompatible forms, got {}",
            score.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_compatible_forms_low_penalty() {
        let engine = MatchingEngine::default();

        // Create offer and request with compatible forms (امبول vs فايل)
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("امبول"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("فايل"));

        // Score the match
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score has low penalty for compatible forms
        assert!(
            score.pharmaceutical_score > 0.8,
            "Expected high pharmaceutical score for compatible forms, got {}",
            score.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_identical_forms_no_penalty() {
        let engine = MatchingEngine::default();

        // Create offer and request with identical forms
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score the match
        let (score, _action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is high (no form penalty)
        assert!(
            score.pharmaceutical_score > 0.95,
            "Expected no penalty for identical forms, got {}",
            score.pharmaceutical_score
        );
    }

    // =========================================================================
    // Task 7.2.4: Test therapeutic class mismatch returns zero score
    // =========================================================================

    #[tokio::test]
    async fn test_therapeutic_class_mismatch_detection() {
        let engine = MatchingEngine::default();

        // Add medications to class index with different classes
        engine.add_medication_class("Aspirin", "Analgesic").await;
        engine
            .add_medication_class("Metformin", "Antidiabetic")
            .await;

        // Detect class mismatch with high embedding similarity
        let result = engine
            .detect_class_mismatch("Aspirin", "Metformin", 0.85)
            .await;

        // Verify mismatch is detected
        assert!(result.is_mismatch, "Expected class mismatch to be detected");
        assert!(
            result.suspicious,
            "Expected match to be flagged as suspicious"
        );
        // Note: Classes are normalized to lowercase by HardNegativeIndex
        assert_eq!(result.offer_class, Some("analgesic".to_string()));
        assert_eq!(result.request_class, Some("antidiabetic".to_string()));
    }

    #[tokio::test]
    async fn test_no_class_mismatch_for_same_class() {
        let engine = MatchingEngine::default();

        // Add medications to class index with same class
        engine.add_medication_class("Aspirin", "Analgesic").await;
        engine.add_medication_class("Ibuprofen", "Analgesic").await;

        // Detect class mismatch
        let result = engine
            .detect_class_mismatch("Aspirin", "Ibuprofen", 0.85)
            .await;

        // Verify no mismatch is detected
        assert!(
            !result.is_mismatch,
            "Expected no class mismatch for same class"
        );
        assert!(!result.suspicious, "Expected match not to be flagged");
    }

    #[tokio::test]
    async fn test_no_class_check_for_low_similarity() {
        let engine = MatchingEngine::default();

        // Add medications with different classes
        engine.add_medication_class("Aspirin", "Analgesic").await;
        engine
            .add_medication_class("Metformin", "Antidiabetic")
            .await;

        // Detect class mismatch with low embedding similarity (below threshold)
        let result = engine
            .detect_class_mismatch("Aspirin", "Metformin", 0.75)
            .await;

        // Verify no mismatch is detected (similarity too low to check)
        assert!(
            !result.is_mismatch,
            "Expected no class mismatch check for low similarity"
        );
    }

    // =========================================================================
    // Task 7.2.5: Test audit trail includes pharmaceutical validation details
    // =========================================================================

    #[tokio::test]
    async fn test_audit_record_includes_pharmaceutical_details() {
        let engine = MatchingEngine::default();

        // Create offer and request with concentration mismatch
        let offer = create_test_offer("Aspirin", Some("150mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score the match
        let (score, action) = engine.score_match(&offer, &request, 0.95, None).await;

        // Build audit record
        let audit_record = engine.build_audit_record(&offer, &request, &score, &action, None);

        // Verify pharmaceutical score is in breakdown
        let breakdown = audit_record.score_breakdown;
        assert!(
            breakdown.get("pharmaceutical_score").is_some(),
            "Expected pharmaceutical_score in breakdown"
        );

        // Verify pharmaceutical validation details are present
        let pharma_validation = breakdown.get("pharmaceutical_validation");
        assert!(
            pharma_validation.is_some(),
            "Expected pharmaceutical_validation in breakdown"
        );

        let pharma_details = pharma_validation.unwrap();
        assert!(
            pharma_details.get("passed").is_some(),
            "Expected 'passed' field in pharmaceutical validation"
        );
        assert!(
            pharma_details.get("score").is_some(),
            "Expected 'score' field in pharmaceutical validation"
        );
        assert!(
            pharma_details.get("concentration_check").is_some(),
            "Expected 'concentration_check' in pharmaceutical validation"
        );
        assert!(
            pharma_details.get("form_check").is_some(),
            "Expected 'form_check' in pharmaceutical validation"
        );
    }

    #[tokio::test]
    async fn test_audit_record_includes_pharmaceutical_weight() {
        let engine = MatchingEngine::default();

        let offer = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        let (score, action) = engine.score_match(&offer, &request, 0.95, None).await;
        let audit_record = engine.build_audit_record(&offer, &request, &score, &action, None);

        // Verify pharmaceutical weight is in weights snapshot
        let weights = audit_record.weights_snapshot;
        assert!(
            weights.get("pharmaceutical").is_some(),
            "Expected pharmaceutical weight in weights snapshot"
        );

        let pharma_weight = weights.get("pharmaceutical").unwrap().as_f64().unwrap();
        // Pharmaceutical weight should be present (may be 0.0 if warm start overrides it)
        assert!(
            pharma_weight >= 0.0,
            "Expected pharmaceutical weight to be present, got {}",
            pharma_weight
        );
    }

    // =========================================================================
    // Task 7.2.6: Test config changes propagate through engine
    // =========================================================================

    #[tokio::test]
    async fn test_pharmaceutical_config_changes_affect_scoring() {
        let engine = MatchingEngine::default();

        // Create offer and request with 30% concentration difference
        let offer = create_test_offer("Aspirin", Some("130mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score with default config (tolerance: 20%, reject: 50%)
        let (score1, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Update config to be more lenient (tolerance: 40%, reject: 70%)
        let mut config = engine.get_pharmaceutical_validator_config();
        config.concentration_tolerance_percent = 40.0;
        config.concentration_reject_threshold_percent = 70.0;
        engine.set_pharmaceutical_validator_config(config);

        // Score again with new config
        let (score2, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify score improved with more lenient config
        assert!(
            score2.pharmaceutical_score > score1.pharmaceutical_score,
            "Expected higher pharmaceutical score with more lenient config: {} vs {}",
            score2.pharmaceutical_score,
            score1.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_disable_concentration_check() {
        let engine = MatchingEngine::default();

        // Create offer and request with large concentration difference
        let offer = create_test_offer("Aspirin", Some("150mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("15mg"), Some("اقراص"));

        // Score with concentration check enabled
        let (score1, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Disable concentration check
        engine.enable_concentration_check(false);

        // Score again with concentration check disabled
        let (score2, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is higher when check is disabled
        assert!(
            score2.pharmaceutical_score > score1.pharmaceutical_score,
            "Expected higher pharmaceutical score with concentration check disabled: {} vs {}",
            score2.pharmaceutical_score,
            score1.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_disable_form_check() {
        let engine = MatchingEngine::default();

        // Create offer and request with incompatible forms
        let offer = create_test_offer("Aspirin", Some("100mg"), Some("امبول"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score with form check enabled
        let (score1, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Disable form check
        engine.enable_form_check(false);

        // Score again with form check disabled
        let (score2, _) = engine.score_match(&offer, &request, 0.95, None).await;

        // Verify pharmaceutical score is higher when check is disabled
        assert!(
            score2.pharmaceutical_score > score1.pharmaceutical_score,
            "Expected higher pharmaceutical score with form check disabled: {} vs {}",
            score2.pharmaceutical_score,
            score1.pharmaceutical_score
        );
    }

    #[tokio::test]
    async fn test_get_pharmaceutical_validator_stats() {
        let engine = MatchingEngine::default();

        // Perform some scoring operations
        let offer1 = create_test_offer("Aspirin", Some("100mg"), Some("اقراص"));
        let request1 = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));
        let _ = engine.score_match(&offer1, &request1, 0.95, None).await;

        let offer2 = create_test_offer("Aspirin", Some("150mg"), Some("امبول"));
        let request2 = create_test_request("Aspirin", Some("15mg"), Some("اقراص"));
        let _ = engine.score_match(&offer2, &request2, 0.95, None).await;

        // Get stats
        let stats = engine.get_pharmaceutical_validator_stats();

        // Verify stats are tracked
        assert!(
            stats.total_validations >= 2,
            "Expected at least 2 validations, got {}",
            stats.total_validations
        );
    }

    #[tokio::test]
    async fn test_score_match_with_details_returns_pharmaceutical_result() {
        let engine = MatchingEngine::default();

        // Create offer and request with concentration mismatch
        let offer = create_test_offer("Aspirin", Some("150mg"), Some("اقراص"));
        let request = create_test_request("Aspirin", Some("100mg"), Some("اقراص"));

        // Score with details
        let (_score, _action, _historical_bonus, pharma_result) = engine
            .score_match_with_details(&offer, &request, 0.95, None)
            .await;

        // Verify pharmaceutical result is returned
        assert!(
            pharma_result.score < 1.0,
            "Expected reduced pharmaceutical score for concentration mismatch"
        );

        // Verify concentration check details are present
        assert!(
            pharma_result.concentration_check.is_some(),
            "Expected concentration check result"
        );

        let conc_check = pharma_result.concentration_check.unwrap();
        assert!(
            conc_check.offer_value.is_some(),
            "Expected offer concentration value"
        );
        assert!(
            conc_check.request_value.is_some(),
            "Expected request concentration value"
        );
        assert!(
            conc_check.difference_percent.is_some(),
            "Expected concentration difference percentage"
        );

        // Verify form check details are present
        assert!(
            pharma_result.form_check.is_some(),
            "Expected form check result"
        );
    }
}
