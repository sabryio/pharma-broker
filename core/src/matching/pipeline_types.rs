//! Enhanced Pipeline Stage Types for Debug Recording
//!
//! This module provides comprehensive types for recording detailed pipeline
//! execution traces, enabling debugging and analysis of the matching process.
//!
//! Features:
//! - Typed pipeline stages with all variants
//! - Stage-specific detail structures
//! - Performance metrics capture
//! - Serialization support for frontend integration

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// Pipeline Stage Type Enum
// =============================================================================

/// Enumeration of all pipeline stage types in the matching process.
/// Each variant represents a discrete step in the matching pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStageType {
    /// Initial message received from WhatsApp/source
    MessageReceived,
    /// AI parsing of the raw message
    AiParsing,
    /// Parsing complete with extracted data
    ParsingComplete,
    /// Medication resolution (name normalization)
    MedicationResolution,
    /// Offer created from parsed data
    OfferCreated,
    /// Request created from parsed data
    RequestCreated,
    /// Initial candidate search
    MatchCandidateSearch,
    /// Hierarchical matching stage (numbered)
    HierarchicalStage { stage_number: u8 },
    /// Score calculation for candidates
    ScoreCalculation,
    /// AI review of match candidates
    AiReview,
    /// Consensus check across multiple models
    ConsensusCheck,
    /// Contrastive validation against negatives
    ContrastiveValidation,
    /// Confidence calibration
    Calibration,
    /// Match record created
    MatchCreated,
    /// Added to review queue
    QueueAdded,
    /// Notification sent
    NotificationSent,
}

impl std::fmt::Display for PipelineStageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStageType::MessageReceived => write!(f, "message_received"),
            PipelineStageType::AiParsing => write!(f, "ai_parsing"),
            PipelineStageType::ParsingComplete => write!(f, "parsing_complete"),
            PipelineStageType::MedicationResolution => write!(f, "medication_resolution"),
            PipelineStageType::OfferCreated => write!(f, "offer_created"),
            PipelineStageType::RequestCreated => write!(f, "request_created"),
            PipelineStageType::MatchCandidateSearch => write!(f, "match_candidate_search"),
            PipelineStageType::HierarchicalStage { stage_number } => {
                write!(f, "hierarchical_stage_{}", stage_number)
            }
            PipelineStageType::ScoreCalculation => write!(f, "score_calculation"),
            PipelineStageType::AiReview => write!(f, "ai_review"),
            PipelineStageType::ConsensusCheck => write!(f, "consensus_check"),
            PipelineStageType::ContrastiveValidation => write!(f, "contrastive_validation"),
            PipelineStageType::Calibration => write!(f, "calibration"),
            PipelineStageType::MatchCreated => write!(f, "match_created"),
            PipelineStageType::QueueAdded => write!(f, "queue_added"),
            PipelineStageType::NotificationSent => write!(f, "notification_sent"),
        }
    }
}

impl PipelineStageType {
    /// Returns true if this stage involves AI processing
    pub fn involves_ai(&self) -> bool {
        matches!(
            self,
            PipelineStageType::AiParsing
                | PipelineStageType::AiReview
                | PipelineStageType::ConsensusCheck
        )
    }

    /// Returns true if this is a hierarchical matching stage
    pub fn is_hierarchical(&self) -> bool {
        matches!(self, PipelineStageType::HierarchicalStage { .. })
    }
}

// =============================================================================
// Enhanced Pipeline Stage Record
// =============================================================================

/// Enhanced pipeline stage record with detailed execution data.
/// Captures timing, candidate counts, memory usage, and stage-specific details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPipelineStageRecord {
    /// Type of pipeline stage
    pub stage_type: PipelineStageType,
    /// Human-readable stage name
    pub stage_name: String,
    /// When the stage started
    pub started_at: DateTime<Utc>,
    /// When the stage completed (None if still running)
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of candidates entering this stage
    pub candidates_in: usize,
    /// Number of candidates after this stage
    pub candidates_out: usize,
    /// Memory usage at stage completion (bytes)
    pub memory_usage_bytes: Option<u64>,
    /// Stage-specific details
    pub details: PipelineStageDetails,
}

impl EnhancedPipelineStageRecord {
    /// Create a new enhanced pipeline stage record
    pub fn new(
        stage_type: PipelineStageType,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        candidates_in: usize,
        candidates_out: usize,
        details: PipelineStageDetails,
    ) -> Self {
        let duration_ms = (completed_at - started_at).num_milliseconds().max(0) as u64;
        Self {
            stage_type,
            stage_name: stage_type.to_string(),
            started_at,
            completed_at: Some(completed_at),
            duration_ms,
            candidates_in,
            candidates_out,
            memory_usage_bytes: None,
            details,
        }
    }

    /// Create a record with memory usage tracking
    pub fn with_memory(mut self, memory_bytes: u64) -> Self {
        self.memory_usage_bytes = Some(memory_bytes);
        self
    }

    /// Check if this stage involves AI
    pub fn involves_ai(&self) -> bool {
        self.stage_type.involves_ai()
    }
}

// =============================================================================
// Pipeline Stage Details Enum
// =============================================================================

/// Stage-specific details for different pipeline stages.
/// Each variant contains data relevant to that particular stage type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineStageDetails {
    /// Details for AI parsing stage
    Parsing(ParsingDetails),
    /// Details for medication resolution stage
    Resolution(ResolutionDetails),
    /// Details for hierarchical matching stages
    Hierarchical(HierarchicalStageDetails),
    /// Details for score calculation stage
    Scoring(ScoringDetails),
    /// Details for AI review stage
    AiReview(AiReviewDetails),
    /// Details for consensus auditing stage
    Consensus(ConsensusDetails),
    /// Details for contrastive validation stage
    Contrastive(ContrastiveDetails),
    /// Details for calibration stage
    Calibration(CalibrationDetails),
    /// Generic details for other stages
    Generic(serde_json::Value),
}

impl Default for PipelineStageDetails {
    fn default() -> Self {
        PipelineStageDetails::Generic(serde_json::Value::Null)
    }
}

// =============================================================================
// Stage-Specific Detail Structures
// =============================================================================

/// Details for AI parsing stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsingDetails {
    /// AI model used for parsing
    pub model_name: String,
    /// Number of prompt tokens
    pub prompt_tokens: Option<u32>,
    /// Number of completion tokens
    pub completion_tokens: Option<u32>,
    /// Total tokens used
    pub total_tokens: Option<u32>,
    /// AI processing latency in milliseconds
    pub latency_ms: u64,
    /// Raw input text
    pub input_text: Option<String>,
    /// Parsed output (medication, quantity, etc.)
    pub parsed_output: serde_json::Value,
    /// Confidence score from AI
    pub confidence: Option<f64>,
    /// Whether parsing was successful
    pub success: bool,
    /// Error message if parsing failed
    pub error: Option<String>,
}

impl ParsingDetails {
    /// Create new parsing details for a successful parse
    pub fn success(
        model_name: impl Into<String>,
        latency_ms: u64,
        parsed_output: serde_json::Value,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            latency_ms,
            input_text: None,
            parsed_output,
            confidence: None,
            success: true,
            error: None,
        }
    }

    /// Create new parsing details for a failed parse
    pub fn failure(
        model_name: impl Into<String>,
        latency_ms: u64,
        error: impl Into<String>,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            latency_ms,
            input_text: None,
            parsed_output: serde_json::Value::Null,
            confidence: None,
            success: false,
            error: Some(error.into()),
        }
    }

    /// Add token counts
    pub fn with_tokens(mut self, prompt: u32, completion: u32) -> Self {
        self.prompt_tokens = Some(prompt);
        self.completion_tokens = Some(completion);
        self.total_tokens = Some(prompt + completion);
        self
    }

    /// Add input text
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input_text = Some(input.into());
        self
    }

    /// Add confidence score
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

/// Details for medication resolution stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionDetails {
    /// Resolution stage that succeeded (exact_alias, exact_name, fuzzy, semantic)
    pub resolution_stage: String,
    /// Master medication ID if resolved
    pub master_id: Option<Uuid>,
    /// Alias ID if matched via alias
    pub alias_id: Option<Uuid>,
    /// Similarity score for fuzzy/semantic matches
    pub similarity_score: Option<f64>,
    /// Embedding distance for semantic matches
    pub embedding_distance: Option<f64>,
    /// Intermediate results from each resolution stage
    pub stage_results: Vec<ResolutionStageResult>,
}

/// Result from a single resolution stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionStageResult {
    /// Stage name (exact_alias, exact_name, fuzzy, semantic)
    pub stage: String,
    /// Whether this stage found a match
    pub matched: bool,
    /// Number of candidates found
    pub candidates_found: usize,
    /// Best match score (if any)
    pub best_score: Option<f64>,
    /// Duration of this stage in milliseconds
    pub duration_ms: u64,
}

/// Details for hierarchical matching stages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalStageDetails {
    /// Stage number (1, 2, 3, etc.)
    pub stage_number: u8,
    /// Stage name (exact, alias, fts, embedding, fuzzy)
    pub stage_name: String,
    /// Threshold used for this stage
    pub threshold: f64,
    /// Candidates with their scores
    pub candidates: Vec<CandidateScore>,
    /// Whether any candidates passed this stage
    pub has_matches: bool,
}

/// A candidate with its score at a particular stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    /// Candidate ID (offer or request ID)
    pub id: Uuid,
    /// Score at this stage
    pub score: f64,
    /// Whether this candidate passed the threshold
    pub passed: bool,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Details for score calculation stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringDetails {
    /// Final combined score
    pub final_score: f64,
    /// Individual component scores
    pub components: HashMap<String, ComponentScore>,
    /// Weights used for scoring
    pub weights: HashMap<String, f64>,
    /// Formula used for combination
    pub formula: String,
}

/// Individual score component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentScore {
    /// Raw score value
    pub raw_score: f64,
    /// Weight applied
    pub weight: f64,
    /// Weighted contribution to final score
    pub weighted_score: f64,
}

/// Details for AI review stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewDetails {
    /// Model used for review
    pub model_name: String,
    /// Review decision (approved, rejected, flagged)
    pub decision: String,
    /// Confidence in the decision
    pub confidence: f64,
    /// Reasoning provided by AI
    pub reasoning: Option<String>,
    /// Token usage
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    /// Latency in milliseconds
    pub latency_ms: u64,
}

/// Details for consensus auditing stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusDetails {
    /// Final consensus status
    pub status: String,
    /// Consensus confidence
    pub confidence: f64,
    /// Agreement ratio among models
    pub agreement_ratio: f64,
    /// Number of models that agreed
    pub agreeing_models: usize,
    /// Total number of models queried
    pub total_models: usize,
    /// Whether consensus was reached
    pub consensus_reached: bool,
    /// Individual model results
    pub model_results: Vec<ModelAuditDetail>,
    /// Combined explanation
    pub explanation: Option<String>,
}

/// Detail from a single model in consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAuditDetail {
    /// Model identifier
    pub model_id: String,
    /// Model's decision
    pub status: Option<String>,
    /// Model's confidence
    pub confidence: Option<f64>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error if model failed
    pub error: Option<String>,
}

/// Details for contrastive validation stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastiveDetails {
    /// Whether validation passed
    pub valid: bool,
    /// Score of the positive (matched) pair
    pub positive_score: f64,
    /// Average score against negative samples
    pub avg_negative_score: f64,
    /// Maximum score against negative samples
    pub max_negative_score: f64,
    /// Margin between positive and average negative
    pub margin_vs_avg: f64,
    /// Margin between positive and max negative
    pub margin_vs_max: f64,
    /// Number of negative samples used
    pub num_negatives: usize,
    /// Reason for validation result
    pub reason: String,
    /// IDs of negative samples used
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub negative_ids: Vec<Uuid>,
}

/// Details for calibration stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationDetails {
    /// Raw score before calibration
    pub raw_score: f64,
    /// Calibrated score after adjustment
    pub calibrated_score: f64,
    /// Calibration method used
    pub method: String,
    /// Expected Calibration Error
    pub ece: Option<f64>,
    /// Bin index used for calibration
    pub bin_index: Option<usize>,
    /// Whether calibration was applied
    pub calibration_applied: bool,
}

// =============================================================================
// Performance Metrics
// =============================================================================

/// Performance metrics for a match operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Peak memory usage in bytes
    pub memory_peak_bytes: Option<u64>,
    /// Time spent waiting in AI queue (milliseconds)
    pub ai_queue_wait_ms: Option<u64>,
    /// Time spent in AI processing (milliseconds)
    pub ai_processing_ms: Option<u64>,
    /// Number of database queries executed
    pub db_query_count: u32,
    /// Total time spent in database queries (milliseconds)
    pub db_total_ms: u64,
    /// Latency breakdown by stage
    pub stage_latencies: HashMap<String, u64>,
}

impl PerformanceMetrics {
    /// Create new empty performance metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record memory usage
    pub fn record_memory(&mut self, bytes: u64) {
        self.memory_peak_bytes = Some(
            self.memory_peak_bytes
                .map(|current| current.max(bytes))
                .unwrap_or(bytes),
        );
    }

    /// Record AI timing
    pub fn record_ai_timing(&mut self, queue_wait_ms: u64, processing_ms: u64) {
        self.ai_queue_wait_ms = Some(
            self.ai_queue_wait_ms
                .map(|current| current + queue_wait_ms)
                .unwrap_or(queue_wait_ms),
        );
        self.ai_processing_ms = Some(
            self.ai_processing_ms
                .map(|current| current + processing_ms)
                .unwrap_or(processing_ms),
        );
    }

    /// Record a database query
    pub fn record_db_query(&mut self, duration_ms: u64) {
        self.db_query_count += 1;
        self.db_total_ms += duration_ms;
    }

    /// Record stage latency
    pub fn record_stage_latency(&mut self, stage: impl Into<String>, duration_ms: u64) {
        self.stage_latencies.insert(stage.into(), duration_ms);
    }

    /// Get total AI time (queue + processing)
    pub fn total_ai_time_ms(&self) -> u64 {
        self.ai_queue_wait_ms.unwrap_or(0) + self.ai_processing_ms.unwrap_or(0)
    }
}

// =============================================================================
// Conversion Implementations
// =============================================================================

impl From<crate::matching::ContrastiveResult> for ContrastiveDetails {
    fn from(result: crate::matching::ContrastiveResult) -> Self {
        Self {
            valid: result.valid,
            positive_score: result.positive_score,
            avg_negative_score: result.avg_negative_score,
            max_negative_score: result.max_negative_score,
            margin_vs_avg: result.margin_vs_avg,
            margin_vs_max: result.margin_vs_max,
            num_negatives: result.num_negatives,
            reason: result.reason,
            negative_ids: result.negative_ids,
        }
    }
}

impl From<crate::matching::ConsensusResult> for ConsensusDetails {
    fn from(result: crate::matching::ConsensusResult) -> Self {
        Self {
            status: format!("{:?}", result.status),
            confidence: result.confidence,
            agreement_ratio: result.agreement_ratio,
            agreeing_models: result.agreeing_models,
            total_models: result.total_models,
            consensus_reached: result.consensus_reached,
            model_results: result
                .model_details
                .into_iter()
                .map(|d| ModelAuditDetail {
                    model_id: d.model_id,
                    status: d.status,
                    confidence: d.confidence.map(|c| c as f64),
                    duration_ms: 0, // Not available in ModelDetail
                    error: d.error,
                })
                .collect(),
            explanation: Some(result.explanation),
        }
    }
}

impl CalibrationDetails {
    /// Create calibration details from raw and calibrated scores
    pub fn from_scores(raw_score: f64, calibrated_score: f64, applied: bool) -> Self {
        Self {
            raw_score,
            calibrated_score,
            method: if applied {
                "histogram".to_string()
            } else {
                "none".to_string()
            },
            ece: None,
            bin_index: None,
            calibration_applied: applied,
        }
    }

    /// Create calibration details with ECE
    pub fn with_ece(mut self, ece: f64) -> Self {
        self.ece = Some(ece);
        self
    }

    /// Create calibration details with bin index
    pub fn with_bin(mut self, bin_index: usize) -> Self {
        self.bin_index = Some(bin_index);
        self
    }
}

impl HierarchicalStageDetails {
    /// Create hierarchical stage details
    pub fn new(stage_number: u8, stage_name: impl Into<String>, threshold: f64) -> Self {
        Self {
            stage_number,
            stage_name: stage_name.into(),
            threshold,
            candidates: Vec::new(),
            has_matches: false,
        }
    }

    /// Add a candidate to the stage
    pub fn add_candidate(&mut self, id: Uuid, score: f64, passed: bool) {
        self.candidates.push(CandidateScore {
            id,
            score,
            passed,
            metadata: None,
        });
        if passed {
            self.has_matches = true;
        }
    }

    /// Add a candidate with metadata
    pub fn add_candidate_with_metadata(
        &mut self,
        id: Uuid,
        score: f64,
        passed: bool,
        metadata: serde_json::Value,
    ) {
        self.candidates.push(CandidateScore {
            id,
            score,
            passed,
            metadata: Some(metadata),
        });
        if passed {
            self.has_matches = true;
        }
    }
}

impl ResolutionDetails {
    /// Create new resolution details
    pub fn new(resolution_stage: impl Into<String>) -> Self {
        Self {
            resolution_stage: resolution_stage.into(),
            master_id: None,
            alias_id: None,
            similarity_score: None,
            embedding_distance: None,
            stage_results: Vec::new(),
        }
    }

    /// Set master ID
    pub fn with_master_id(mut self, id: Uuid) -> Self {
        self.master_id = Some(id);
        self
    }

    /// Set alias ID
    pub fn with_alias_id(mut self, id: Uuid) -> Self {
        self.alias_id = Some(id);
        self
    }

    /// Set similarity score
    pub fn with_similarity(mut self, score: f64) -> Self {
        self.similarity_score = Some(score);
        self
    }

    /// Set embedding distance
    pub fn with_embedding_distance(mut self, distance: f64) -> Self {
        self.embedding_distance = Some(distance);
        self
    }

    /// Add a stage result
    pub fn add_stage_result(&mut self, result: ResolutionStageResult) {
        self.stage_results.push(result);
    }
}

impl ResolutionStageResult {
    /// Create a new resolution stage result
    pub fn new(stage: impl Into<String>, matched: bool, duration_ms: u64) -> Self {
        Self {
            stage: stage.into(),
            matched,
            candidates_found: 0,
            best_score: None,
            duration_ms,
        }
    }

    /// Set candidates found
    pub fn with_candidates(mut self, count: usize, best_score: Option<f64>) -> Self {
        self.candidates_found = count;
        self.best_score = best_score;
        self
    }
}

impl ScoringDetails {
    /// Create new scoring details
    pub fn new(final_score: f64, formula: impl Into<String>) -> Self {
        Self {
            final_score,
            components: HashMap::new(),
            weights: HashMap::new(),
            formula: formula.into(),
        }
    }

    /// Add a component score
    pub fn add_component(&mut self, name: impl Into<String>, raw_score: f64, weight: f64) {
        let name = name.into();
        self.components.insert(
            name.clone(),
            ComponentScore {
                raw_score,
                weight,
                weighted_score: raw_score * weight,
            },
        );
        self.weights.insert(name, weight);
    }
}

impl AiReviewDetails {
    /// Create new AI review details
    pub fn new(
        model_name: impl Into<String>,
        decision: impl Into<String>,
        confidence: f64,
        latency_ms: u64,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            decision: decision.into(),
            confidence,
            reasoning: None,
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms,
        }
    }

    /// Add reasoning
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    /// Add token counts
    pub fn with_tokens(mut self, prompt: u32, completion: u32) -> Self {
        self.prompt_tokens = Some(prompt);
        self.completion_tokens = Some(completion);
        self
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stage_type_display() {
        assert_eq!(
            PipelineStageType::MessageReceived.to_string(),
            "message_received"
        );
        assert_eq!(PipelineStageType::AiParsing.to_string(), "ai_parsing");
        assert_eq!(
            PipelineStageType::HierarchicalStage { stage_number: 2 }.to_string(),
            "hierarchical_stage_2"
        );
    }

    #[test]
    fn test_pipeline_stage_type_involves_ai() {
        assert!(PipelineStageType::AiParsing.involves_ai());
        assert!(PipelineStageType::AiReview.involves_ai());
        assert!(PipelineStageType::ConsensusCheck.involves_ai());
        assert!(!PipelineStageType::ScoreCalculation.involves_ai());
        assert!(!PipelineStageType::MedicationResolution.involves_ai());
    }

    #[test]
    fn test_pipeline_stage_type_is_hierarchical() {
        assert!(PipelineStageType::HierarchicalStage { stage_number: 1 }.is_hierarchical());
        assert!(!PipelineStageType::ScoreCalculation.is_hierarchical());
    }

    #[test]
    fn test_parsing_details_success() {
        let details =
            ParsingDetails::success("gpt-4", 150, serde_json::json!({"medication": "Aspirin"}))
                .with_tokens(100, 50)
                .with_confidence(0.95);

        assert!(details.success);
        assert_eq!(details.model_name, "gpt-4");
        assert_eq!(details.latency_ms, 150);
        assert_eq!(details.prompt_tokens, Some(100));
        assert_eq!(details.completion_tokens, Some(50));
        assert_eq!(details.total_tokens, Some(150));
        assert_eq!(details.confidence, Some(0.95));
    }

    #[test]
    fn test_parsing_details_failure() {
        let details = ParsingDetails::failure("gpt-4", 50, "Rate limit exceeded");

        assert!(!details.success);
        assert_eq!(details.error, Some("Rate limit exceeded".to_string()));
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();

        metrics.record_memory(1000);
        metrics.record_memory(2000);
        assert_eq!(metrics.memory_peak_bytes, Some(2000));

        metrics.record_ai_timing(10, 100);
        metrics.record_ai_timing(5, 50);
        assert_eq!(metrics.ai_queue_wait_ms, Some(15));
        assert_eq!(metrics.ai_processing_ms, Some(150));
        assert_eq!(metrics.total_ai_time_ms(), 165);

        metrics.record_db_query(5);
        metrics.record_db_query(10);
        assert_eq!(metrics.db_query_count, 2);
        assert_eq!(metrics.db_total_ms, 15);

        metrics.record_stage_latency("parsing", 100);
        assert_eq!(metrics.stage_latencies.get("parsing"), Some(&100));
    }

    #[test]
    fn test_enhanced_pipeline_stage_record() {
        let started = Utc::now();
        let completed = started + chrono::Duration::milliseconds(100);

        let record = EnhancedPipelineStageRecord::new(
            PipelineStageType::AiParsing,
            started,
            completed,
            10,
            8,
            PipelineStageDetails::Parsing(ParsingDetails::success(
                "gpt-4",
                100,
                serde_json::json!({}),
            )),
        );

        assert_eq!(record.stage_type, PipelineStageType::AiParsing);
        assert_eq!(record.candidates_in, 10);
        assert_eq!(record.candidates_out, 8);
        assert!(record.involves_ai());
    }

    #[test]
    fn test_pipeline_stage_details_serialization() {
        let details = PipelineStageDetails::Parsing(ParsingDetails::success(
            "gpt-4",
            100,
            serde_json::json!({"medication": "Test"}),
        ));

        let json = serde_json::to_string(&details).unwrap();
        assert!(json.contains("\"type\":\"parsing\""));

        let deserialized: PipelineStageDetails = serde_json::from_str(&json).unwrap();
        match deserialized {
            PipelineStageDetails::Parsing(p) => {
                assert_eq!(p.model_name, "gpt-4");
            }
            _ => panic!("Expected Parsing variant"),
        }
    }
}
