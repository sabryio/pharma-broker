//! Match Audit Recorder
//!
//! Stores complete snapshots of all inputs and parameters for debugging
//! and reproducibility. Designed to integrate with frontend debug recordings.
//!
//! Features:
//! - Complete offer/request snapshots
//! - Weight and config snapshots
//! - Pipeline execution trace
//! - AI involvement tracking
//! - Replay capability for debugging
//! - Session tracking for frontend integration

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Offer, Request};
use crate::matching::hierarchical_matcher::MatchStage;
use crate::matching::{NormalizedWeights, ScoreBreakdown};

// =============================================================================
// Types
// =============================================================================

/// Pipeline stage execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageRecord {
    pub stage: String,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub candidates_in: usize,
    pub candidates_out: usize,
    pub details: Option<serde_json::Value>,
}

/// AI involvement record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIInvolvementRecord {
    pub model: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub latency_ms: u64,
    pub response: serde_json::Value,
}

/// Resolution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionDetails {
    pub offer_master_id: Option<Uuid>,
    pub request_master_id: Option<Uuid>,
    pub offer_alias_id: Option<Uuid>,
    pub request_alias_id: Option<Uuid>,
    pub similarity_score: Option<f64>,
    pub embedding_distance: Option<f64>,
}

/// Client metadata for frontend integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMetadata {
    pub user_agent: Option<String>,
    pub client_version: Option<String>,
    pub recording_id: Option<String>,
}

/// Complete match audit record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAuditRecord {
    pub id: Uuid,
    pub match_id: Uuid,
    pub offer_id: Uuid,
    pub request_id: Uuid,
    pub pipeline_version: String,
    pub offer_snapshot: serde_json::Value,
    pub request_snapshot: serde_json::Value,
    pub weights_snapshot: serde_json::Value,
    pub config_snapshot: Option<serde_json::Value>,
    pub score_breakdown: serde_json::Value,
    pub final_score: f64,
    pub pipeline_stages: Vec<PipelineStageRecord>,
    pub ai_involved: bool,
    pub ai_record: Option<AIInvolvementRecord>,
    pub resolution_stage: String,
    pub resolution_details: Option<ResolutionDetails>,
    pub total_latency_ms: u64,
    pub created_at: DateTime<Utc>,
    pub review_status: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub session_id: Option<String>,
    pub client_metadata: Option<ClientMetadata>,
}

// =============================================================================
// Builder for creating audit records
// =============================================================================

/// Builder for constructing match audit records
pub struct AuditRecordBuilder {
    match_id: Uuid,
    offer: Offer,
    request: Request,
    pipeline_version: String,
    weights: NormalizedWeights,
    config: Option<serde_json::Value>,
    stages: Vec<PipelineStageRecord>,
    enhanced_stages: Vec<crate::matching::EnhancedPipelineStageRecord>,
    ai_record: Option<AIInvolvementRecord>,
    resolution_stage: MatchStage,
    resolution_details: Option<ResolutionDetails>,
    consensus_details: Option<crate::matching::EnhancedConsensusDetails>,
    contrastive_details: Option<crate::matching::EnhancedContrastiveDetails>,
    calibration_details: Option<crate::matching::CalibrationDetails>,
    performance_metrics: crate::matching::PipelinePerformanceMetrics,
    session_id: Option<String>,
    client_metadata: Option<ClientMetadata>,
    start_time: Instant,
}

impl AuditRecordBuilder {
    /// Create a new builder
    pub fn new(match_id: Uuid, offer: Offer, request: Request, weights: NormalizedWeights) -> Self {
        Self {
            match_id,
            offer,
            request,
            pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
            weights,
            config: None,
            stages: Vec::new(),
            enhanced_stages: Vec::new(),
            ai_record: None,
            resolution_stage: MatchStage::FuzzyValidated,
            resolution_details: None,
            consensus_details: None,
            contrastive_details: None,
            calibration_details: None,
            performance_metrics: crate::matching::PipelinePerformanceMetrics::new(),
            session_id: None,
            client_metadata: None,
            start_time: Instant::now(),
        }
    }

    /// Set pipeline version
    pub fn pipeline_version(mut self, version: impl Into<String>) -> Self {
        self.pipeline_version = version.into();
        self
    }

    /// Set config snapshot
    pub fn config(mut self, config: serde_json::Value) -> Self {
        self.config = Some(config);
        self
    }

    /// Add a pipeline stage record (legacy method for backward compatibility)
    pub fn add_stage(&mut self, stage: PipelineStageRecord) {
        self.stages.push(stage);
    }

    /// Add an enhanced pipeline stage record
    pub fn add_enhanced_stage(&mut self, stage: crate::matching::EnhancedPipelineStageRecord) {
        // Also add to legacy stages for backward compatibility
        self.stages.push(PipelineStageRecord {
            stage: stage.stage_name.clone(),
            started_at: stage.started_at,
            duration_ms: stage.duration_ms,
            candidates_in: stage.candidates_in,
            candidates_out: stage.candidates_out,
            details: serde_json::to_value(&stage.details).ok(),
        });

        // Record stage latency in performance metrics
        self.performance_metrics
            .record_stage_latency(&stage.stage_name, stage.duration_ms);

        self.enhanced_stages.push(stage);
    }

    /// Add a parsing stage with details
    pub fn add_parsing_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        details: crate::matching::ParsingDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::AiParsing,
            started_at,
            completed_at,
            1,
            if details.success { 1 } else { 0 },
            PipelineStageDetails::Parsing(details.clone()),
        );

        // Record AI timing
        self.performance_metrics
            .record_ai_timing(0, details.latency_ms);

        self.add_enhanced_stage(stage);
    }

    /// Add a hierarchical matching stage with details
    pub fn add_hierarchical_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        candidates_out: usize,
        details: crate::matching::HierarchicalStageDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::HierarchicalStage {
                stage_number: details.stage_number,
            },
            started_at,
            completed_at,
            candidates_in,
            candidates_out,
            PipelineStageDetails::Hierarchical(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a score calculation stage with details
    pub fn add_scoring_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        details: crate::matching::ScoringDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::ScoreCalculation,
            started_at,
            completed_at,
            candidates_in,
            candidates_in, // Score calculation doesn't filter candidates
            PipelineStageDetails::Scoring(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add an AI review stage with details
    pub fn add_ai_review_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        details: crate::matching::AiReviewDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        // Record AI timing
        self.performance_metrics
            .record_ai_timing(0, details.latency_ms);

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::AiReview,
            started_at,
            completed_at,
            candidates_in,
            candidates_in,
            PipelineStageDetails::AiReview(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a consensus check stage with details
    pub fn add_consensus_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        details: crate::matching::EnhancedConsensusDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        // Store consensus details for the final record
        self.consensus_details = Some(details.clone());

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::ConsensusCheck,
            started_at,
            completed_at,
            candidates_in,
            candidates_in,
            PipelineStageDetails::Consensus(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a contrastive validation stage with details
    pub fn add_contrastive_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        details: crate::matching::EnhancedContrastiveDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        // Store contrastive details for the final record
        self.contrastive_details = Some(details.clone());

        let candidates_out = if details.valid { candidates_in } else { 0 };

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::ContrastiveValidation,
            started_at,
            completed_at,
            candidates_in,
            candidates_out,
            PipelineStageDetails::Contrastive(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a calibration stage with details
    pub fn add_calibration_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        details: crate::matching::CalibrationDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        // Store calibration details for the final record
        self.calibration_details = Some(details.clone());

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::Calibration,
            started_at,
            completed_at,
            1,
            1,
            PipelineStageDetails::Calibration(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a resolution stage with details
    pub fn add_resolution_stage(
        &mut self,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        candidates_out: usize,
        details: crate::matching::PipelineResolutionDetails,
    ) {
        use crate::matching::{
            EnhancedPipelineStageRecord, PipelineStageDetails, PipelineStageType,
        };

        let stage = EnhancedPipelineStageRecord::new(
            PipelineStageType::MedicationResolution,
            started_at,
            completed_at,
            candidates_in,
            candidates_out,
            PipelineStageDetails::Resolution(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Add a generic stage with custom details
    pub fn add_generic_stage(
        &mut self,
        stage_type: crate::matching::PipelineStageType,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        candidates_out: usize,
        details: serde_json::Value,
    ) {
        use crate::matching::{EnhancedPipelineStageRecord, PipelineStageDetails};

        let stage = EnhancedPipelineStageRecord::new(
            stage_type,
            started_at,
            completed_at,
            candidates_in,
            candidates_out,
            PipelineStageDetails::Generic(details),
        );

        self.add_enhanced_stage(stage);
    }

    /// Record AI involvement
    pub fn ai_involvement(mut self, record: AIInvolvementRecord) -> Self {
        self.ai_record = Some(record);
        self
    }

    /// Set resolution stage
    pub fn resolution_stage(mut self, stage: MatchStage) -> Self {
        self.resolution_stage = stage;
        self
    }

    /// Set resolution details (legacy)
    pub fn resolution_details(mut self, details: ResolutionDetails) -> Self {
        self.resolution_details = Some(details);
        self
    }

    /// Set consensus details
    pub fn consensus_details(mut self, details: crate::matching::EnhancedConsensusDetails) -> Self {
        self.consensus_details = Some(details);
        self
    }

    /// Set contrastive details
    pub fn contrastive_details(
        mut self,
        details: crate::matching::EnhancedContrastiveDetails,
    ) -> Self {
        self.contrastive_details = Some(details);
        self
    }

    /// Set calibration details
    pub fn calibration_details(mut self, details: crate::matching::CalibrationDetails) -> Self {
        self.calibration_details = Some(details);
        self
    }

    /// Record memory usage
    pub fn record_memory(&mut self, bytes: u64) {
        self.performance_metrics.record_memory(bytes);
    }

    /// Record database query
    pub fn record_db_query(&mut self, duration_ms: u64) {
        self.performance_metrics.record_db_query(duration_ms);
    }

    /// Record AI timing (queue wait + processing)
    pub fn record_ai_timing(&mut self, queue_wait_ms: u64, processing_ms: u64) {
        self.performance_metrics
            .record_ai_timing(queue_wait_ms, processing_ms);
    }

    /// Get performance metrics reference
    pub fn performance_metrics(&self) -> &crate::matching::PipelinePerformanceMetrics {
        &self.performance_metrics
    }

    /// Get mutable performance metrics reference
    pub fn performance_metrics_mut(&mut self) -> &mut crate::matching::PipelinePerformanceMetrics {
        &mut self.performance_metrics
    }

    /// Set session ID for frontend integration
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Set client metadata
    pub fn client_metadata(mut self, metadata: ClientMetadata) -> Self {
        self.client_metadata = Some(metadata);
        self
    }

    /// Get the enhanced stages
    pub fn get_enhanced_stages(&self) -> &[crate::matching::EnhancedPipelineStageRecord] {
        &self.enhanced_stages
    }

    /// Build the final audit record
    pub fn build(self, score_breakdown: &ScoreBreakdown) -> MatchAuditRecord {
        let total_latency = self.start_time.elapsed();

        MatchAuditRecord {
            id: Uuid::new_v4(),
            match_id: self.match_id,
            offer_id: self.offer.id,
            request_id: self.request.id,
            pipeline_version: self.pipeline_version,
            offer_snapshot: serde_json::to_value(&self.offer).unwrap_or_default(),
            request_snapshot: serde_json::to_value(&self.request).unwrap_or_default(),
            weights_snapshot: serde_json::to_value(&self.weights).unwrap_or_default(),
            config_snapshot: self.config,
            score_breakdown: serde_json::to_value(score_breakdown).unwrap_or_default(),
            final_score: score_breakdown.total.value(),
            pipeline_stages: self.stages,
            ai_involved: self.ai_record.is_some()
                || self.enhanced_stages.iter().any(|s| s.involves_ai()),
            ai_record: self.ai_record,
            resolution_stage: self.resolution_stage.to_string(),
            resolution_details: self.resolution_details,
            total_latency_ms: total_latency.as_millis() as u64,
            created_at: Utc::now(),
            review_status: None,
            reviewed_by: None,
            reviewed_at: None,
            review_notes: None,
            session_id: self.session_id,
            client_metadata: self.client_metadata,
        }
    }

    /// Build the final audit record with enhanced data
    pub fn build_enhanced(self, score_breakdown: &ScoreBreakdown) -> EnhancedMatchAuditRecord {
        let total_latency = self.start_time.elapsed();
        let ai_involved =
            self.ai_record.is_some() || self.enhanced_stages.iter().any(|s| s.involves_ai());

        EnhancedMatchAuditRecord {
            id: Uuid::new_v4(),
            match_id: self.match_id,
            offer_id: self.offer.id,
            request_id: self.request.id,
            pipeline_version: self.pipeline_version,
            offer_snapshot: serde_json::to_value(&self.offer).unwrap_or_default(),
            request_snapshot: serde_json::to_value(&self.request).unwrap_or_default(),
            weights_snapshot: serde_json::to_value(&self.weights).unwrap_or_default(),
            config_snapshot: self.config,
            score_breakdown: serde_json::to_value(score_breakdown).unwrap_or_default(),
            final_score: score_breakdown.total.value(),
            pipeline_stages: self.stages,
            enhanced_stages: self.enhanced_stages,
            ai_involved,
            ai_record: self.ai_record,
            resolution_stage: self.resolution_stage.to_string(),
            resolution_details: self.resolution_details,
            consensus_details: self.consensus_details,
            contrastive_details: self.contrastive_details,
            calibration_details: self.calibration_details,
            performance_metrics: self.performance_metrics,
            total_latency_ms: total_latency.as_millis() as u64,
            created_at: Utc::now(),
            review_status: None,
            reviewed_by: None,
            reviewed_at: None,
            review_notes: None,
            session_id: self.session_id,
            client_metadata: self.client_metadata,
        }
    }
}

// =============================================================================
// Enhanced Match Audit Record
// =============================================================================

/// Enhanced match audit record with detailed pipeline data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedMatchAuditRecord {
    pub id: Uuid,
    pub match_id: Uuid,
    pub offer_id: Uuid,
    pub request_id: Uuid,
    pub pipeline_version: String,
    pub offer_snapshot: serde_json::Value,
    pub request_snapshot: serde_json::Value,
    pub weights_snapshot: serde_json::Value,
    pub config_snapshot: Option<serde_json::Value>,
    pub score_breakdown: serde_json::Value,
    pub final_score: f64,
    /// Legacy pipeline stages for backward compatibility
    pub pipeline_stages: Vec<PipelineStageRecord>,
    /// Enhanced pipeline stages with detailed data
    pub enhanced_stages: Vec<crate::matching::EnhancedPipelineStageRecord>,
    pub ai_involved: bool,
    pub ai_record: Option<AIInvolvementRecord>,
    pub resolution_stage: String,
    pub resolution_details: Option<ResolutionDetails>,
    /// Consensus auditing details
    pub consensus_details: Option<crate::matching::EnhancedConsensusDetails>,
    /// Contrastive validation details
    pub contrastive_details: Option<crate::matching::EnhancedContrastiveDetails>,
    /// Calibration details
    pub calibration_details: Option<crate::matching::CalibrationDetails>,
    /// Performance metrics
    pub performance_metrics: crate::matching::PipelinePerformanceMetrics,
    pub total_latency_ms: u64,
    pub created_at: DateTime<Utc>,
    pub review_status: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_notes: Option<String>,
    pub session_id: Option<String>,
    pub client_metadata: Option<ClientMetadata>,
}

impl EnhancedMatchAuditRecord {
    /// Convert to legacy MatchAuditRecord
    pub fn to_legacy(&self) -> MatchAuditRecord {
        MatchAuditRecord {
            id: self.id,
            match_id: self.match_id,
            offer_id: self.offer_id,
            request_id: self.request_id,
            pipeline_version: self.pipeline_version.clone(),
            offer_snapshot: self.offer_snapshot.clone(),
            request_snapshot: self.request_snapshot.clone(),
            weights_snapshot: self.weights_snapshot.clone(),
            config_snapshot: self.config_snapshot.clone(),
            score_breakdown: self.score_breakdown.clone(),
            final_score: self.final_score,
            pipeline_stages: self.pipeline_stages.clone(),
            ai_involved: self.ai_involved,
            ai_record: self.ai_record.clone(),
            resolution_stage: self.resolution_stage.clone(),
            resolution_details: self.resolution_details.clone(),
            total_latency_ms: self.total_latency_ms,
            created_at: self.created_at,
            review_status: self.review_status.clone(),
            reviewed_by: self.reviewed_by,
            reviewed_at: self.reviewed_at,
            review_notes: self.review_notes.clone(),
            session_id: self.session_id.clone(),
            client_metadata: self.client_metadata.clone(),
        }
    }

    /// Get pipeline stage by name
    pub fn get_stage(&self, stage_name: &str) -> Option<&PipelineStageRecord> {
        self.pipeline_stages.iter().find(|s| s.stage == stage_name)
    }

    /// Get enhanced pipeline stage by type
    pub fn get_enhanced_stage(
        &self,
        stage_type: crate::matching::PipelineStageType,
    ) -> Option<&crate::matching::EnhancedPipelineStageRecord> {
        self.enhanced_stages
            .iter()
            .find(|s| s.stage_type == stage_type)
    }

    /// Get total pipeline duration
    pub fn pipeline_duration_ms(&self) -> u64 {
        self.pipeline_stages.iter().map(|s| s.duration_ms).sum()
    }

    /// Check if AI was the bottleneck
    pub fn ai_was_bottleneck(&self) -> bool {
        let ai_time = self.performance_metrics.total_ai_time_ms();
        if self.total_latency_ms > 0 {
            return ai_time as f64 / self.total_latency_ms as f64 > 0.5;
        }
        false
    }
}

// =============================================================================
// Stage Timer Helper
// =============================================================================

/// Helper for timing pipeline stages
pub struct StageTimer {
    stage: String,
    started_at: DateTime<Utc>,
    start_instant: Instant,
    candidates_in: usize,
}

impl StageTimer {
    /// Start timing a stage
    pub fn start(stage: impl Into<String>, candidates_in: usize) -> Self {
        Self {
            stage: stage.into(),
            started_at: Utc::now(),
            start_instant: Instant::now(),
            candidates_in,
        }
    }

    /// Finish timing and create a record
    pub fn finish(
        self,
        candidates_out: usize,
        details: Option<serde_json::Value>,
    ) -> PipelineStageRecord {
        PipelineStageRecord {
            stage: self.stage,
            started_at: self.started_at,
            duration_ms: self.start_instant.elapsed().as_millis() as u64,
            candidates_in: self.candidates_in,
            candidates_out,
            details,
        }
    }
}

// =============================================================================
// Audit Recorder Service
// =============================================================================

/// Configuration for the audit recorder
#[derive(Debug, Clone)]
pub struct AuditRecorderConfig {
    /// Whether recording is enabled
    pub enabled: bool,
    /// Maximum records to keep in memory buffer
    pub buffer_size: usize,
    /// Whether to persist to database
    pub persist_to_db: bool,
    /// Minimum score to record (filter out low-score matches)
    pub min_score_threshold: Option<f64>,
    /// Sample rate (0.0 - 1.0) for high-volume scenarios
    pub sample_rate: f64,
}

impl Default for AuditRecorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 1000,
            persist_to_db: true,
            min_score_threshold: None,
            sample_rate: 1.0,
        }
    }
}

impl AuditRecorderConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("AUDIT_RECORDER_ENABLED")
                .map(|v| v != "false")
                .unwrap_or(true),
            buffer_size: std::env::var("AUDIT_RECORDER_BUFFER_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            persist_to_db: std::env::var("AUDIT_RECORDER_PERSIST_DB")
                .map(|v| v != "false")
                .unwrap_or(true),
            min_score_threshold: std::env::var("AUDIT_RECORDER_MIN_SCORE")
                .ok()
                .and_then(|v| v.parse().ok()),
            sample_rate: std::env::var("AUDIT_RECORDER_SAMPLE_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

/// Statistics for the audit recorder
#[derive(Debug, Default)]
pub struct AuditRecorderStats {
    pub records_created: std::sync::atomic::AtomicU64,
    pub records_persisted: std::sync::atomic::AtomicU64,
    pub records_sampled_out: std::sync::atomic::AtomicU64,
    pub records_filtered_by_score: std::sync::atomic::AtomicU64,
    pub persist_errors: std::sync::atomic::AtomicU64,
}

impl AuditRecorderStats {
    pub fn snapshot(&self) -> AuditRecorderStatsSnapshot {
        use std::sync::atomic::Ordering;
        AuditRecorderStatsSnapshot {
            records_created: self.records_created.load(Ordering::Relaxed),
            records_persisted: self.records_persisted.load(Ordering::Relaxed),
            records_sampled_out: self.records_sampled_out.load(Ordering::Relaxed),
            records_filtered_by_score: self.records_filtered_by_score.load(Ordering::Relaxed),
            persist_errors: self.persist_errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditRecorderStatsSnapshot {
    pub records_created: u64,
    pub records_persisted: u64,
    pub records_sampled_out: u64,
    pub records_filtered_by_score: u64,
    pub persist_errors: u64,
}

/// Audit recorder service
pub struct AuditRecorder {
    config: AuditRecorderConfig,
    buffer: std::sync::RwLock<std::collections::VecDeque<MatchAuditRecord>>,
    stats: AuditRecorderStats,
}

impl AuditRecorder {
    /// Create a new audit recorder
    pub fn new(config: AuditRecorderConfig) -> Self {
        Self {
            config,
            buffer: std::sync::RwLock::new(std::collections::VecDeque::new()),
            stats: AuditRecorderStats::default(),
        }
    }

    /// Create with default config from environment
    pub fn from_env() -> Self {
        Self::new(AuditRecorderConfig::from_env())
    }

    /// Check if recording is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Record a match audit
    pub fn record(&self, record: MatchAuditRecord) -> bool {
        use std::sync::atomic::Ordering;

        if !self.config.enabled {
            return false;
        }

        // Check score threshold
        if let Some(min_score) = self.config.min_score_threshold
            && record.final_score < min_score
        {
            self.stats
                .records_filtered_by_score
                .fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // Check sample rate
        if self.config.sample_rate < 1.0 {
            use rand::Rng;
            let should_sample = rand::rng().random::<f64>() < self.config.sample_rate;

            if !should_sample {
                self.stats
                    .records_sampled_out
                    .fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }

        // Add to buffer
        if let Ok(mut buffer) = self.buffer.write() {
            // Enforce buffer size limit
            while buffer.len() >= self.config.buffer_size {
                buffer.pop_front();
            }
            buffer.push_back(record);
            self.stats.records_created.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get recent records from buffer
    pub fn get_recent(&self, limit: usize) -> Vec<MatchAuditRecord> {
        self.buffer
            .read()
            .map(|buffer| buffer.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    /// Get record by match ID
    pub fn get_by_match_id(&self, match_id: Uuid) -> Option<MatchAuditRecord> {
        self.buffer
            .read()
            .ok()
            .and_then(|buffer| buffer.iter().find(|r| r.match_id == match_id).cloned())
    }

    /// Get records by session ID (for frontend integration)
    pub fn get_by_session(&self, session_id: &str) -> Vec<MatchAuditRecord> {
        self.buffer
            .read()
            .map(|buffer| {
                buffer
                    .iter()
                    .filter(|r| r.session_id.as_deref() == Some(session_id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Update review status for a record
    pub fn update_review_status(
        &self,
        match_id: Uuid,
        status: &str,
        reviewed_by: Uuid,
        notes: Option<&str>,
    ) -> bool {
        if let Ok(mut buffer) = self.buffer.write()
            && let Some(record) = buffer.iter_mut().find(|r| r.match_id == match_id)
        {
            record.review_status = Some(status.to_string());
            record.reviewed_by = Some(reviewed_by);
            record.reviewed_at = Some(Utc::now());
            record.review_notes = notes.map(|s| s.to_string());
            return true;
        }
        false
    }

    /// Drain buffer for persistence
    pub fn drain_buffer(&self) -> Vec<MatchAuditRecord> {
        self.buffer
            .write()
            .map(|mut buffer| buffer.drain(..).collect())
            .unwrap_or_default()
    }

    /// Get statistics
    pub fn stats(&self) -> AuditRecorderStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get buffer size
    pub fn buffer_len(&self) -> usize {
        self.buffer.read().map(|b| b.len()).unwrap_or(0)
    }

    /// Get configuration
    pub fn config(&self) -> &AuditRecorderConfig {
        &self.config
    }
}

// =============================================================================
// Replay Support
// =============================================================================

/// Replay context for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayContext {
    pub record: MatchAuditRecord,
    pub offer: Offer,
    pub request: Request,
    pub weights: NormalizedWeights,
}

impl MatchAuditRecord {
    /// Create a replay context from this record
    pub fn to_replay_context(&self) -> Result<ReplayContext, serde_json::Error> {
        let offer: Offer = serde_json::from_value(self.offer_snapshot.clone())?;
        let request: Request = serde_json::from_value(self.request_snapshot.clone())?;
        let weights: NormalizedWeights = serde_json::from_value(self.weights_snapshot.clone())?;

        Ok(ReplayContext {
            record: self.clone(),
            offer,
            request,
            weights,
        })
    }

    /// Get pipeline stage by name
    pub fn get_stage(&self, stage_name: &str) -> Option<&PipelineStageRecord> {
        self.pipeline_stages.iter().find(|s| s.stage == stage_name)
    }

    /// Get total pipeline duration
    pub fn pipeline_duration_ms(&self) -> u64 {
        self.pipeline_stages.iter().map(|s| s.duration_ms).sum()
    }

    /// Check if AI was the bottleneck
    pub fn ai_was_bottleneck(&self) -> bool {
        if let Some(ai) = &self.ai_record {
            let pipeline_duration = self.pipeline_duration_ms();
            if pipeline_duration > 0 {
                return ai.latency_ms as f64 / pipeline_duration as f64 > 0.5;
            }
        }
        false
    }
}

// =============================================================================
// Frontend Integration Types
// =============================================================================

/// Simplified record for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendAuditRecord {
    pub id: Uuid,
    pub match_id: Uuid,
    pub offer_product: String,
    pub request_product: String,
    pub final_score: f64,
    pub resolution_stage: String,
    pub ai_involved: bool,
    pub total_latency_ms: u64,
    pub created_at: DateTime<Utc>,
    pub review_status: Option<String>,
    pub pipeline_summary: Vec<PipelineStageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageSummary {
    pub stage: String,
    pub duration_ms: u64,
    pub candidates_out: usize,
}

impl From<&MatchAuditRecord> for FrontendAuditRecord {
    fn from(record: &MatchAuditRecord) -> Self {
        let offer_product = record
            .offer_snapshot
            .get("medication")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let request_product = record
            .request_snapshot
            .get("medication")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        Self {
            id: record.id,
            match_id: record.match_id,
            offer_product,
            request_product,
            final_score: record.final_score,
            resolution_stage: record.resolution_stage.clone(),
            ai_involved: record.ai_involved,
            total_latency_ms: record.total_latency_ms,
            created_at: record.created_at,
            review_status: record.review_status.clone(),
            pipeline_summary: record
                .pipeline_stages
                .iter()
                .map(|s| PipelineStageSummary {
                    stage: s.stage.clone(),
                    duration_ms: s.duration_ms,
                    candidates_out: s.candidates_out,
                })
                .collect(),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_offer() -> Offer {
        Offer {
            id: Uuid::new_v4(),
            medication: "Test Product".to_string(),
            ..Default::default()
        }
    }

    fn create_test_request() -> Request {
        Request {
            id: Uuid::new_v4(),
            medication: "Test Product".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_audit_recorder_basic() {
        let recorder = AuditRecorder::new(AuditRecorderConfig::default());

        let offer = create_test_offer();
        let request = create_test_request();
        let weights = NormalizedWeights::default();
        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6);

        let builder = AuditRecordBuilder::new(Uuid::new_v4(), offer, request, weights);
        let record = builder.build(&breakdown);

        assert!(recorder.record(record.clone()));
        assert_eq!(recorder.buffer_len(), 1);

        let recent = recorder.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].match_id, record.match_id);
    }

    #[test]
    fn test_stage_timer() {
        let timer = StageTimer::start("exact_match", 100);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let record = timer.finish(5, None);

        assert_eq!(record.stage, "exact_match");
        assert_eq!(record.candidates_in, 100);
        assert_eq!(record.candidates_out, 5);
        assert!(record.duration_ms >= 10);
    }

    #[test]
    fn test_sample_rate() {
        let config = AuditRecorderConfig {
            sample_rate: 0.0, // Never sample
            ..Default::default()
        };
        let recorder = AuditRecorder::new(config);

        let offer = create_test_offer();
        let request = create_test_request();
        let weights = NormalizedWeights::default();
        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.8, 0.7, 0.6);

        let builder = AuditRecordBuilder::new(Uuid::new_v4(), offer, request, weights);
        let record = builder.build(&breakdown);

        assert!(!recorder.record(record));
        assert_eq!(recorder.buffer_len(), 0);
    }

    #[test]
    fn test_score_threshold() {
        let config = AuditRecorderConfig {
            min_score_threshold: Some(0.8),
            ..Default::default()
        };
        let recorder = AuditRecorder::new(config);

        let offer = create_test_offer();
        let request = create_test_request();
        let weights = NormalizedWeights::default();

        // Low score - should be filtered
        let breakdown = ScoreBreakdown::new(&weights, 0.5, 0.5, 0.5, 0.5);
        let builder = AuditRecordBuilder::new(
            Uuid::new_v4(),
            offer.clone(),
            request.clone(),
            weights.clone(),
        );
        let record = builder.build(&breakdown);
        assert!(!recorder.record(record));

        // High score - should be recorded
        let breakdown = ScoreBreakdown::new(&weights, 0.9, 0.9, 0.9, 0.9);
        let builder = AuditRecordBuilder::new(Uuid::new_v4(), offer, request, weights);
        let record = builder.build(&breakdown);
        assert!(recorder.record(record));
    }
}
