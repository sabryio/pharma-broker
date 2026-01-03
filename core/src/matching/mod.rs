//! Matching engine module
//!
//! Ported from legacy/matching/*.go

mod abtest;
mod actions;
mod alias_learner;
pub mod arabic;
mod audit;
mod audit_recorder;
mod auditor_factory;
mod blocklist;
mod calibration;
mod confidence;
mod consensus_auditor;
mod contrastive_validator;
mod dosage;
mod dosage_gate;
mod embedding_cache;
mod engine;
mod ensemble;
mod fallback_matcher;
mod filter;
mod fts_search;
mod fuzzy;
mod hierarchical_matcher;
mod historical;
mod hybrid_filter;
mod learner;
mod medication_resolver;
mod reviewer;
mod scheduler;
mod score_types;
mod scorer;
mod service;
mod thresholds;
mod uncertainty_estimator;
mod warm_start;
mod weights;

// =============================================================================
// Explicit Re-exports
// =============================================================================

// --- weights ---
pub use weights::{Thresholds, Weights};

// --- ai_client ---
pub use ai_client::Client as AIClient;

// --- scorer ---
pub use scorer::{MatchScore, Scorer};

// --- actions ---
pub use actions::{AutoActionConfig, AutoActionHandler, MatchAction, ParseAction};

// --- abtest ---
pub use abtest::{
    ABTestConfig, ABTestManager, ABTestResult, ABTestStats, AutoDecision, AutoRollbackConfig,
};

// --- audit ---
pub use audit::{
    ActionType, AuditEntry, AuditError, AuditEventType, AuditFilter, AuditLogger, AuditTrail,
    AuditTrailConfig, MatchActionParams, MemoryAuditLogger,
};

// --- calibration ---
pub use calibration::{
    CalibrationBin, CalibrationConfig, CalibrationReport, CalibrationStatsSnapshot,
    ConfidenceCalibrator,
};

// --- confidence ---
pub use confidence::{ConfidenceConfig, ConfidenceManager, ConfidenceManagerStats};

// --- dosage ---
pub use dosage::{Dosage, compare_dosages, is_same_dosage, parse_dosage};

// --- dosage_gate ---
pub use dosage_gate::{DosageFlag, DosageGate, DosageGateConfig, DosageGateResult};

// --- blocklist ---
pub use blocklist::{BlocklistEntry, BlocklistError, BlocklistSeverity, MedicationBlocklist};

// --- arabic ---
pub use arabic::normalize_arabic;

// --- fuzzy ---
pub use fuzzy::{medication_similarity, medication_similarity_with_raw};

// --- embedding_cache ---
pub use embedding_cache::{EmbeddingCache, EmbeddingCacheStatsSnapshot, SynonymIndex};

// --- engine ---
pub use engine::{MatchingEngine, MatchingEngineConfig, SchedulerStats};

// --- ensemble ---
pub use ensemble::{
    EnsembleConfig, EnsembleMatcher, MatchExplanation, MatchingStrategy, StrategyContext,
    StrategyScore,
};

// --- filter ---
pub use filter::{
    FilterReason, FilterResult, MatchFilter, MatchFilterConfig, MatchFilterStatsSnapshot,
};

// --- fts_search ---
pub use fts_search::{FtsSearchConfig, FtsSearchStatsSnapshot, FtsTokenSearcher};

// --- historical ---
pub use historical::{
    HistoricalLearner, HistoricalLearnerStats, HistoricalLearningConfig, MedicationAffinity,
};

// --- hybrid_filter ---
pub use hybrid_filter::{HybridFilterConfig, HybridFilterStatsSnapshot, HybridMappingFilter};

// --- learner ---
pub use learner::{
    FeedbackRecordRepository, LearnerError, LearningConfig, PerformanceMetrics, WeightHistory,
    WeightHistoryRepository, WeightLearner, WeightSource,
};

// --- scheduler ---
pub use scheduler::{
    AutoApplyConfig, JobStatus, LearningScheduler, NotificationConfig, SchedulerConfig,
    SchedulerStatus,
};

// --- service ---
pub use service::{MatchingError, MatchingService};

// --- thresholds ---
pub use thresholds::{SmoothConfidenceResult, SmoothThresholdCalculator, SmoothThresholdConfig};

// --- warm_start ---
pub use warm_start::{OutlierDetector, OutlierDetectorConfig, WarmStartConfig, WarmStartManager};

// --- medication_resolver ---
pub use medication_resolver::{
    MedicationResolver, MedicationResolverConfig, ResolutionMethod, ResolutionResult,
};

// --- hierarchical_matcher ---
pub use hierarchical_matcher::{
    HierarchicalConfig, HierarchicalStats, HierarchicalStatsSnapshot, MatchCandidate, MatchStage,
};

// --- alias_learner ---
pub use alias_learner::{AliasLearner, AliasLearnerConfig, AliasLearnerStatsSnapshot, LearnResult};

// --- fallback_matcher ---
pub use fallback_matcher::{
    FallbackMatchMethod, FallbackMatchResult, FallbackMatcher, FallbackMatcherConfig,
};

// --- score_types ---
pub use score_types::{
    ComponentScore, ConfidenceScore, NormalizedWeights, ScoreBreakdown, Weight, WeightError,
};

// --- reviewer ---
pub use reviewer::{AIReviewer, ReviewResult, ReviewStatus};

// --- consensus_auditor ---
pub use consensus_auditor::{
    ConsensusAuditor, ConsensusConfig, ConsensusResult, ConsensusStats, ConsensusStatsSnapshot,
    ModelAuditResult, ModelDetail,
};

// --- auditor_factory ---
pub use auditor_factory::{
    AuditorFactory, AuditorFactoryConfig, AuditorType, HybridAuditor, ModelConfig,
};

// --- contrastive_validator ---
pub use contrastive_validator::{
    ContrastiveConfig, ContrastiveResult, ContrastiveStats, ContrastiveStatsSnapshot,
    ContrastiveValidator,
};

// --- audit_recorder ---
pub use audit_recorder::{
    AIInvolvementRecord, AuditRecordBuilder, AuditRecorder, AuditRecorderConfig,
    AuditRecorderStats, AuditRecorderStatsSnapshot, ClientMetadata, FrontendAuditRecord,
    MatchAuditRecord, PipelineStageRecord, PipelineStageSummary, ReplayContext, ResolutionDetails,
    StageTimer,
};

// --- uncertainty_estimator ---
pub use uncertainty_estimator::{
    EnsembleUncertainty, EnsembleUncertaintyResult, UncertaintyConfig, UncertaintyEstimator,
    UncertaintyResult,
};

// =============================================================================
// Types defined in this module
// =============================================================================

/// Type of recency decay curve
/// Ported from Go: DecayType (interface.go:93-99)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecayType {
    /// Exponential decay: e^(-λt) - Natural decay (default)
    #[default]
    Exponential,
    /// Linear decay: 1 - t/max - Constant rate
    Linear,
    /// Logarithmic decay: sqrt(1 - age/maxAge) - Slower decay
    Logarithmic,
}

/// Compute cosine similarity between two embedding vectors
/// Ported from legacy/pkg/matcher/similarity/cosine.go
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f64, &'static str> {
    if a.len() != b.len() {
        return Err("vectors must have equal length");
    }
    if a.is_empty() {
        return Err("vectors must not be empty");
    }

    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;

    for (ai, bi) in a.iter().zip(b.iter()) {
        let ai = *ai as f64;
        let bi = *bi as f64;
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return Ok(0.0);
    }

    Ok(dot / (norm_a.sqrt() * norm_b.sqrt()))
}

#[cfg(test)]
mod similarity_tests {
    use super::*;

    #[test]
    fn test_cosine_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_error_length() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &b).is_err());
    }
}
