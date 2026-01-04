//! Pipeline Visualization API
//!
//! Provides endpoints and types for visualizing pipeline execution traces.
//! This module transforms audit records into a format suitable for frontend
//! pipeline visualization components.
//!
//! Requirements: 4.1, 4.2, 4.3, 4.4, 4.5

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::matching::{
    AiReviewDetails, CalibrationDetails, CandidateScore, EnhancedConsensusDetails,
    EnhancedContrastiveDetails, EnhancedPipelineStageRecord, HierarchicalStageDetails,
    MatchAuditRecord, PipelinePerformanceMetrics, PipelineResolutionDetails, ScoringDetails,
};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

// =============================================================================
// Response Types - Requirements 4.1, 4.2, 4.3, 4.4, 4.5
// =============================================================================

/// Complete pipeline visualization response
/// Requirement 4.1: Return structured pipeline stages with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineVisualizationResponse {
    /// Match ID this visualization is for
    pub match_id: Uuid,
    /// Offer ID involved in the match
    pub offer_id: Uuid,
    /// Request ID involved in the match
    pub request_id: Uuid,
    /// Pipeline version used
    pub pipeline_version: String,
    /// Final match score
    pub final_score: f64,
    /// Total pipeline latency in milliseconds
    pub total_latency_ms: u64,
    /// Whether AI was involved in this match
    pub ai_involved: bool,
    /// Resolution stage that produced the match
    pub resolution_stage: String,
    /// All pipeline stages with visualization data
    pub stages: Vec<PipelineStageVisualization>,
    /// Hierarchical matching details (if present)
    /// Requirement 4.2: Return candidate lists with scores for each stage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchical_details: Option<Vec<HierarchicalStageVisualization>>,
    /// Score breakdown visualization
    /// Requirement 4.4: Return all component scores with weights and formulas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_breakdown: Option<ScoreBreakdownVisualization>,
    /// AI review details (if present)
    /// Requirement 4.3: Return model details, reasoning, and confidence scores
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_review: Option<AiReviewVisualization>,
    /// Resolution path details
    /// Requirement 4.5: Return resolution path with intermediate results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionVisualization>,
    /// Consensus auditing details (if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus: Option<ConsensusVisualization>,
    /// Contrastive validation details (if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrastive: Option<ContrastiveVisualization>,
    /// Calibration details (if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration: Option<CalibrationVisualization>,
    /// Performance metrics
    pub performance_metrics: PerformanceMetricsVisualization,
    /// Timestamp when the match was created
    pub created_at: String,
}

/// Individual pipeline stage visualization
/// Requirement 4.1: Each stage includes timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageVisualization {
    /// Stage type identifier
    pub stage_type: String,
    /// Human-readable stage name
    pub stage_name: String,
    /// Stage execution status
    pub status: String,
    /// When the stage started (ISO 8601)
    pub started_at: String,
    /// When the stage completed (ISO 8601, None if still running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of candidates entering this stage
    pub candidates_in: usize,
    /// Number of candidates after this stage
    pub candidates_out: usize,
    /// Whether this stage involves AI processing
    pub involves_ai: bool,
    /// Stage-specific details as JSON
    pub details: serde_json::Value,
}

/// Hierarchical matching stage visualization
/// Requirement 4.2: Candidate lists with scores for each stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalStageVisualization {
    /// Stage number (1, 2, 3, etc.)
    pub stage_number: u8,
    /// Stage name (exact, alias, fts, embedding, fuzzy)
    pub stage_name: String,
    /// Threshold used for this stage
    pub threshold: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of candidates entering
    pub candidates_in: usize,
    /// Number of candidates passing
    pub candidates_out: usize,
    /// Whether any candidates passed this stage
    pub has_matches: bool,
    /// Candidates with their scores
    pub candidates: Vec<CandidateVisualization>,
}

/// Candidate score visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateVisualization {
    /// Candidate ID
    pub id: Uuid,
    /// Score at this stage
    pub score: f64,
    /// Whether this candidate passed the threshold
    pub passed: bool,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Score breakdown visualization
/// Requirement 4.4: All component scores with weights and formulas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdownVisualization {
    /// Final combined score
    pub final_score: f64,
    /// Formula used for combination
    pub formula: String,
    /// Individual component scores
    pub components: Vec<ScoreComponentVisualization>,
    /// Total weight sum (should be 1.0 for normalized weights)
    pub total_weight: f64,
}

/// Individual score component visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreComponentVisualization {
    /// Component name (e.g., "name", "quantity", "price")
    pub name: String,
    /// Raw score value (0.0 - 1.0)
    pub raw_score: f64,
    /// Weight applied to this component
    pub weight: f64,
    /// Weighted contribution to final score
    pub weighted_score: f64,
    /// Percentage contribution to final score
    pub contribution_percent: f64,
}

/// AI review visualization
/// Requirement 4.3: Model details, reasoning, and confidence scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewVisualization {
    /// Model used for review
    pub model_name: String,
    /// Review decision (approved, rejected, flagged)
    pub decision: String,
    /// Confidence in the decision (0.0 - 1.0)
    pub confidence: f64,
    /// Reasoning provided by AI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Token usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsageVisualization>,
}

/// Token usage visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageVisualization {
    /// Prompt tokens used
    pub prompt_tokens: u32,
    /// Completion tokens used
    pub completion_tokens: u32,
    /// Total tokens used
    pub total_tokens: u32,
}

/// Resolution path visualization
/// Requirement 4.5: Resolution path with intermediate results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionVisualization {
    /// Resolution stage that succeeded
    pub resolution_stage: String,
    /// Master medication ID if resolved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_id: Option<Uuid>,
    /// Alias ID if matched via alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_id: Option<Uuid>,
    /// Similarity score for fuzzy/semantic matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity_score: Option<f64>,
    /// Embedding distance for semantic matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_distance: Option<f64>,
    /// Intermediate results from each resolution stage
    pub stage_results: Vec<ResolutionStageVisualization>,
}

/// Resolution stage result visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionStageVisualization {
    /// Stage name (exact_alias, exact_name, fuzzy, semantic)
    pub stage: String,
    /// Whether this stage found a match
    pub matched: bool,
    /// Number of candidates found
    pub candidates_found: usize,
    /// Best match score (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_score: Option<f64>,
    /// Duration of this stage in milliseconds
    pub duration_ms: u64,
}

/// Consensus auditing visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVisualization {
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
    /// Combined explanation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Individual model results
    pub model_results: Vec<ModelResultVisualization>,
}

/// Individual model result in consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResultVisualization {
    /// Model identifier
    pub model_id: String,
    /// Model's decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Model's confidence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error if model failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Contrastive validation visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastiveVisualization {
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
}

/// Calibration visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationVisualization {
    /// Raw score before calibration
    pub raw_score: f64,
    /// Calibrated score after adjustment
    pub calibrated_score: f64,
    /// Calibration method used
    pub method: String,
    /// Score adjustment (calibrated - raw)
    pub adjustment: f64,
    /// Whether calibration was applied
    pub calibration_applied: bool,
    /// Expected Calibration Error (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ece: Option<f64>,
    /// Bin index used for calibration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_index: Option<usize>,
}

/// Performance metrics visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetricsVisualization {
    /// Peak memory usage in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_peak_bytes: Option<u64>,
    /// Time spent waiting in AI queue (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_queue_wait_ms: Option<u64>,
    /// Time spent in AI processing (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_processing_ms: Option<u64>,
    /// Total AI time (queue + processing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_ai_time_ms: Option<u64>,
    /// Number of database queries executed
    pub db_query_count: u32,
    /// Total time spent in database queries (milliseconds)
    pub db_total_ms: u64,
    /// Latency breakdown by stage
    pub stage_latencies: std::collections::HashMap<String, u64>,
}

// =============================================================================
// Conversion Functions
// =============================================================================

impl From<&EnhancedPipelineStageRecord> for PipelineStageVisualization {
    fn from(record: &EnhancedPipelineStageRecord) -> Self {
        Self {
            stage_type: format!("{:?}", record.stage_type),
            stage_name: record.stage_name.clone(),
            status: if record.completed_at.is_some() {
                "completed".to_string()
            } else {
                "running".to_string()
            },
            started_at: record.started_at.to_rfc3339(),
            completed_at: record.completed_at.map(|t| t.to_rfc3339()),
            duration_ms: record.duration_ms,
            candidates_in: record.candidates_in,
            candidates_out: record.candidates_out,
            involves_ai: record.involves_ai(),
            details: serde_json::to_value(&record.details).unwrap_or_default(),
        }
    }
}

impl From<&CandidateScore> for CandidateVisualization {
    fn from(candidate: &CandidateScore) -> Self {
        Self {
            id: candidate.id,
            score: candidate.score,
            passed: candidate.passed,
            metadata: candidate.metadata.clone(),
        }
    }
}

impl From<&HierarchicalStageDetails> for HierarchicalStageVisualization {
    fn from(details: &HierarchicalStageDetails) -> Self {
        Self {
            stage_number: details.stage_number,
            stage_name: details.stage_name.clone(),
            threshold: details.threshold,
            duration_ms: 0, // Will be set from parent record
            candidates_in: 0,
            candidates_out: details.candidates.iter().filter(|c| c.passed).count(),
            has_matches: details.has_matches,
            candidates: details
                .candidates
                .iter()
                .map(CandidateVisualization::from)
                .collect(),
        }
    }
}

impl From<&ScoringDetails> for ScoreBreakdownVisualization {
    fn from(details: &ScoringDetails) -> Self {
        let total_weight: f64 = details.weights.values().sum();
        let components: Vec<ScoreComponentVisualization> = details
            .components
            .iter()
            .map(|(name, component)| {
                let contribution_percent = if details.final_score > 0.0 {
                    (component.weighted_score / details.final_score) * 100.0
                } else {
                    0.0
                };
                ScoreComponentVisualization {
                    name: name.clone(),
                    raw_score: component.raw_score,
                    weight: component.weight,
                    weighted_score: component.weighted_score,
                    contribution_percent,
                }
            })
            .collect();

        Self {
            final_score: details.final_score,
            formula: details.formula.clone(),
            components,
            total_weight,
        }
    }
}

impl From<&AiReviewDetails> for AiReviewVisualization {
    fn from(details: &AiReviewDetails) -> Self {
        let token_usage = match (details.prompt_tokens, details.completion_tokens) {
            (Some(prompt), Some(completion)) => Some(TokenUsageVisualization {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }),
            _ => None,
        };

        Self {
            model_name: details.model_name.clone(),
            decision: details.decision.clone(),
            confidence: details.confidence,
            reasoning: details.reasoning.clone(),
            latency_ms: details.latency_ms,
            token_usage,
        }
    }
}

impl From<&PipelineResolutionDetails> for ResolutionVisualization {
    fn from(details: &PipelineResolutionDetails) -> Self {
        Self {
            resolution_stage: details.resolution_stage.clone(),
            master_id: details.master_id,
            alias_id: details.alias_id,
            similarity_score: details.similarity_score,
            embedding_distance: details.embedding_distance,
            stage_results: details
                .stage_results
                .iter()
                .map(|r| ResolutionStageVisualization {
                    stage: r.stage.clone(),
                    matched: r.matched,
                    candidates_found: r.candidates_found,
                    best_score: r.best_score,
                    duration_ms: r.duration_ms,
                })
                .collect(),
        }
    }
}

impl From<&EnhancedConsensusDetails> for ConsensusVisualization {
    fn from(details: &EnhancedConsensusDetails) -> Self {
        Self {
            status: details.status.clone(),
            confidence: details.confidence,
            agreement_ratio: details.agreement_ratio,
            agreeing_models: details.agreeing_models,
            total_models: details.total_models,
            consensus_reached: details.consensus_reached,
            explanation: details.explanation.clone(),
            model_results: details
                .model_results
                .iter()
                .map(|r| ModelResultVisualization {
                    model_id: r.model_id.clone(),
                    status: r.status.clone(),
                    confidence: r.confidence,
                    duration_ms: r.duration_ms,
                    error: r.error.clone(),
                })
                .collect(),
        }
    }
}

impl From<&EnhancedContrastiveDetails> for ContrastiveVisualization {
    fn from(details: &EnhancedContrastiveDetails) -> Self {
        Self {
            valid: details.valid,
            positive_score: details.positive_score,
            avg_negative_score: details.avg_negative_score,
            max_negative_score: details.max_negative_score,
            margin_vs_avg: details.margin_vs_avg,
            margin_vs_max: details.margin_vs_max,
            num_negatives: details.num_negatives,
            reason: details.reason.clone(),
        }
    }
}

impl From<&CalibrationDetails> for CalibrationVisualization {
    fn from(details: &CalibrationDetails) -> Self {
        Self {
            raw_score: details.raw_score,
            calibrated_score: details.calibrated_score,
            method: details.method.clone(),
            adjustment: details.calibrated_score - details.raw_score,
            calibration_applied: details.calibration_applied,
            ece: details.ece,
            bin_index: details.bin_index,
        }
    }
}

impl From<&PipelinePerformanceMetrics> for PerformanceMetricsVisualization {
    fn from(metrics: &PipelinePerformanceMetrics) -> Self {
        let total_ai_time_ms = match (metrics.ai_queue_wait_ms, metrics.ai_processing_ms) {
            (Some(queue), Some(processing)) => Some(queue + processing),
            (Some(queue), None) => Some(queue),
            (None, Some(processing)) => Some(processing),
            (None, None) => None,
        };

        Self {
            memory_peak_bytes: metrics.memory_peak_bytes,
            ai_queue_wait_ms: metrics.ai_queue_wait_ms,
            ai_processing_ms: metrics.ai_processing_ms,
            total_ai_time_ms,
            db_query_count: metrics.db_query_count,
            db_total_ms: metrics.db_total_ms,
            stage_latencies: metrics.stage_latencies.clone(),
        }
    }
}

// =============================================================================
// Transformation Functions
// =============================================================================

/// Transform a MatchAuditRecord into a PipelineVisualizationResponse
/// This extracts and formats all pipeline data for frontend visualization
pub fn transform_to_visualization(record: &MatchAuditRecord) -> PipelineVisualizationResponse {
    // Extract stages from pipeline_stages (legacy format)
    let stages: Vec<PipelineStageVisualization> = record
        .pipeline_stages
        .iter()
        .map(|stage| PipelineStageVisualization {
            stage_type: stage.stage.clone(),
            stage_name: stage.stage.clone(),
            status: "completed".to_string(),
            started_at: stage.started_at.to_rfc3339(),
            completed_at: Some(
                (stage.started_at + chrono::Duration::milliseconds(stage.duration_ms as i64))
                    .to_rfc3339(),
            ),
            duration_ms: stage.duration_ms,
            candidates_in: stage.candidates_in,
            candidates_out: stage.candidates_out,
            involves_ai: stage.stage.contains("ai") || stage.stage.contains("parsing"),
            details: stage.details.clone().unwrap_or(serde_json::Value::Null),
        })
        .collect();

    // Extract hierarchical details from stage details
    let hierarchical_details = extract_hierarchical_details(record);

    // Extract score breakdown from the record
    let score_breakdown = extract_score_breakdown(record);

    // Extract AI review details
    let ai_review = extract_ai_review(record);

    // Extract resolution details
    let resolution = extract_resolution(record);

    // Extract consensus details
    let consensus = extract_consensus(record);

    // Extract contrastive details
    let contrastive = extract_contrastive(record);

    // Extract calibration details
    let calibration = extract_calibration(record);

    // Create performance metrics (from legacy record, limited data)
    let performance_metrics = PerformanceMetricsVisualization {
        memory_peak_bytes: None,
        ai_queue_wait_ms: None,
        ai_processing_ms: record.ai_record.as_ref().map(|ai| ai.latency_ms),
        total_ai_time_ms: record.ai_record.as_ref().map(|ai| ai.latency_ms),
        db_query_count: 0,
        db_total_ms: 0,
        stage_latencies: record
            .pipeline_stages
            .iter()
            .map(|s| (s.stage.clone(), s.duration_ms))
            .collect(),
    };

    PipelineVisualizationResponse {
        match_id: record.match_id,
        offer_id: record.offer_id,
        request_id: record.request_id,
        pipeline_version: record.pipeline_version.clone(),
        final_score: record.final_score,
        total_latency_ms: record.total_latency_ms,
        ai_involved: record.ai_involved,
        resolution_stage: record.resolution_stage.clone(),
        stages,
        hierarchical_details,
        score_breakdown,
        ai_review,
        resolution,
        consensus,
        contrastive,
        calibration,
        performance_metrics,
        created_at: record.created_at.to_rfc3339(),
    }
}

/// Extract hierarchical stage details from pipeline stages
fn extract_hierarchical_details(
    record: &MatchAuditRecord,
) -> Option<Vec<HierarchicalStageVisualization>> {
    let hierarchical_stages: Vec<HierarchicalStageVisualization> = record
        .pipeline_stages
        .iter()
        .filter(|s| s.stage.starts_with("hierarchical_stage"))
        .filter_map(|stage| {
            stage.details.as_ref().and_then(|details| {
                // Try to parse as HierarchicalStageDetails
                if let Ok(h_details) = serde_json::from_value::<HierarchicalStageDetails>(
                    details
                        .get("hierarchical")
                        .cloned()
                        .unwrap_or(details.clone()),
                ) {
                    let mut viz = HierarchicalStageVisualization::from(&h_details);
                    viz.duration_ms = stage.duration_ms;
                    viz.candidates_in = stage.candidates_in;
                    Some(viz)
                } else {
                    None
                }
            })
        })
        .collect();

    if hierarchical_stages.is_empty() {
        None
    } else {
        Some(hierarchical_stages)
    }
}

/// Extract score breakdown from the audit record
fn extract_score_breakdown(record: &MatchAuditRecord) -> Option<ScoreBreakdownVisualization> {
    // Try to parse score_breakdown as ScoringDetails
    if let Ok(scoring) = serde_json::from_value::<ScoringDetails>(record.score_breakdown.clone()) {
        return Some(ScoreBreakdownVisualization::from(&scoring));
    }

    // Fallback: try to extract from the raw JSON structure
    if let Some(obj) = record.score_breakdown.as_object() {
        let mut components = Vec::new();
        let mut total_weight = 0.0;

        // Common score component names
        let component_names = [
            "name",
            "quantity",
            "price",
            "expiry",
            "recency",
            "medication",
            "dosage",
        ];

        for name in &component_names {
            if let Some(value) = obj.get(*name)
                && let Some(score) = value.as_f64()
            {
                let weight = 1.0 / component_names.len() as f64; // Default equal weights
                total_weight += weight;
                components.push(ScoreComponentVisualization {
                    name: name.to_string(),
                    raw_score: score,
                    weight,
                    weighted_score: score * weight,
                    contribution_percent: if record.final_score > 0.0 {
                        (score * weight / record.final_score) * 100.0
                    } else {
                        0.0
                    },
                });
            }
        }

        if !components.is_empty() {
            return Some(ScoreBreakdownVisualization {
                final_score: record.final_score,
                formula: "weighted_sum".to_string(),
                components,
                total_weight,
            });
        }
    }

    None
}

/// Extract AI review details from the audit record
fn extract_ai_review(record: &MatchAuditRecord) -> Option<AiReviewVisualization> {
    // First check pipeline stages for AI review stage
    for stage in &record.pipeline_stages {
        if stage.stage.contains("ai_review")
            && let Some(details) = &stage.details
            && let Ok(ai_details) = serde_json::from_value::<AiReviewDetails>(details.clone())
        {
            return Some(AiReviewVisualization::from(&ai_details));
        }
    }

    // Fallback to ai_record if present
    record.ai_record.as_ref().map(|ai| AiReviewVisualization {
        model_name: ai.model.clone(),
        decision: "processed".to_string(),
        confidence: 0.0,
        reasoning: None,
        latency_ms: ai.latency_ms,
        token_usage: match (ai.prompt_tokens, ai.completion_tokens) {
            (Some(prompt), Some(completion)) => Some(TokenUsageVisualization {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }),
            _ => None,
        },
    })
}

/// Extract resolution details from the audit record
fn extract_resolution(record: &MatchAuditRecord) -> Option<ResolutionVisualization> {
    // Check pipeline stages for resolution stage
    for stage in &record.pipeline_stages {
        if (stage.stage.contains("resolution") || stage.stage.contains("medication_resolution"))
            && let Some(details) = &stage.details
            && let Ok(res_details) =
                serde_json::from_value::<PipelineResolutionDetails>(details.clone())
        {
            return Some(ResolutionVisualization::from(&res_details));
        }
    }

    // Fallback to resolution_details if present
    record.resolution_details.as_ref().map(|res| {
        ResolutionVisualization {
            resolution_stage: record.resolution_stage.clone(),
            master_id: res.offer_master_id.or(res.request_master_id),
            alias_id: res.offer_alias_id.or(res.request_alias_id),
            similarity_score: res.similarity_score,
            embedding_distance: res.embedding_distance,
            stage_results: vec![], // Legacy format doesn't have stage results
        }
    })
}

/// Extract consensus details from the audit record
fn extract_consensus(record: &MatchAuditRecord) -> Option<ConsensusVisualization> {
    for stage in &record.pipeline_stages {
        if stage.stage.contains("consensus")
            && let Some(details) = &stage.details
            && let Ok(consensus_details) =
                serde_json::from_value::<EnhancedConsensusDetails>(details.clone())
        {
            return Some(ConsensusVisualization::from(&consensus_details));
        }
    }
    None
}

/// Extract contrastive validation details from the audit record
fn extract_contrastive(record: &MatchAuditRecord) -> Option<ContrastiveVisualization> {
    for stage in &record.pipeline_stages {
        if stage.stage.contains("contrastive")
            && let Some(details) = &stage.details
            && let Ok(contrastive_details) =
                serde_json::from_value::<EnhancedContrastiveDetails>(details.clone())
        {
            return Some(ContrastiveVisualization::from(&contrastive_details));
        }
    }
    None
}

/// Extract calibration details from the audit record
fn extract_calibration(record: &MatchAuditRecord) -> Option<CalibrationVisualization> {
    for stage in &record.pipeline_stages {
        if stage.stage.contains("calibration")
            && let Some(details) = &stage.details
            && let Ok(calibration_details) =
                serde_json::from_value::<CalibrationDetails>(details.clone())
        {
            return Some(CalibrationVisualization::from(&calibration_details));
        }
    }
    None
}

// =============================================================================
// API Handler - Requirement 4.1, 4.2, 4.3, 4.4, 4.5
// =============================================================================

/// GET /api/audit-records/{match_id}/pipeline - Get pipeline visualization for a match
///
/// Returns structured pipeline stages with timing information, hierarchical details,
/// AI review details, and score breakdown for frontend visualization.
///
/// Requirements:
/// - 4.1: Return structured pipeline stages with timing information
/// - 4.2: Return candidate lists with scores for each hierarchical stage
/// - 4.3: Return AI model details, reasoning, and confidence scores
/// - 4.4: Return all component scores with weights and formulas
/// - 4.5: Return resolution path with intermediate results
pub async fn get_pipeline_visualization<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let engine = state.matching_engine.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Matching engine not available".to_string(),
    ))?;

    let recorder = engine.get_audit_recorder();

    let record = recorder.get_by_match_id(match_id).ok_or((
        StatusCode::NOT_FOUND,
        format!("Audit record not found for match {}", match_id),
    ))?;

    let visualization = transform_to_visualization(&record);

    Ok(Json(visualization))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::{AIInvolvementRecord, PipelineStageRecord};
    use chrono::Utc;

    fn create_test_audit_record() -> MatchAuditRecord {
        MatchAuditRecord {
            id: Uuid::new_v4(),
            match_id: Uuid::new_v4(),
            offer_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            pipeline_version: "1.0.0".to_string(),
            offer_snapshot: serde_json::json!({"medication": "Aspirin"}),
            request_snapshot: serde_json::json!({"medication": "Aspirin"}),
            weights_snapshot: serde_json::json!({}),
            config_snapshot: None,
            score_breakdown: serde_json::json!({
                "name": 0.9,
                "quantity": 0.8,
                "price": 0.7
            }),
            final_score: 0.85,
            pipeline_stages: vec![
                PipelineStageRecord {
                    stage: "message_received".to_string(),
                    started_at: Utc::now(),
                    duration_ms: 10,
                    candidates_in: 0,
                    candidates_out: 1,
                    details: None,
                },
                PipelineStageRecord {
                    stage: "ai_parsing".to_string(),
                    started_at: Utc::now(),
                    duration_ms: 150,
                    candidates_in: 1,
                    candidates_out: 1,
                    details: Some(serde_json::json!({
                        "model_name": "gpt-4",
                        "latency_ms": 150
                    })),
                },
                PipelineStageRecord {
                    stage: "hierarchical_stage_1".to_string(),
                    started_at: Utc::now(),
                    duration_ms: 50,
                    candidates_in: 100,
                    candidates_out: 20,
                    details: None,
                },
            ],
            ai_involved: true,
            ai_record: Some(AIInvolvementRecord {
                model: "gpt-4".to_string(),
                prompt_tokens: Some(100),
                completion_tokens: Some(50),
                latency_ms: 150,
                response: serde_json::json!({"medication": "Aspirin"}),
            }),
            resolution_stage: "exact_match".to_string(),
            resolution_details: None,
            total_latency_ms: 210,
            created_at: Utc::now(),
            review_status: None,
            reviewed_by: None,
            reviewed_at: None,
            review_notes: None,
            session_id: Some("test-session".to_string()),
            client_metadata: None,
        }
    }

    #[test]
    fn test_transform_to_visualization() {
        let record = create_test_audit_record();
        let viz = transform_to_visualization(&record);

        assert_eq!(viz.match_id, record.match_id);
        assert_eq!(viz.final_score, record.final_score);
        assert_eq!(viz.total_latency_ms, record.total_latency_ms);
        assert!(viz.ai_involved);
        assert_eq!(viz.stages.len(), 3);
    }

    #[test]
    fn test_stage_visualization() {
        let record = create_test_audit_record();
        let viz = transform_to_visualization(&record);

        let ai_stage = viz.stages.iter().find(|s| s.stage_name == "ai_parsing");
        assert!(ai_stage.is_some());

        let ai_stage = ai_stage.unwrap();
        assert!(ai_stage.involves_ai);
        assert_eq!(ai_stage.duration_ms, 150);
    }

    #[test]
    fn test_score_breakdown_extraction() {
        let record = create_test_audit_record();
        let viz = transform_to_visualization(&record);

        assert!(viz.score_breakdown.is_some());
        let breakdown = viz.score_breakdown.unwrap();
        assert_eq!(breakdown.final_score, 0.85);
        assert!(!breakdown.components.is_empty());
    }

    #[test]
    fn test_ai_review_extraction() {
        let record = create_test_audit_record();
        let viz = transform_to_visualization(&record);

        assert!(viz.ai_review.is_some());
        let ai_review = viz.ai_review.unwrap();
        assert_eq!(ai_review.model_name, "gpt-4");
        assert_eq!(ai_review.latency_ms, 150);
    }

    #[test]
    fn test_performance_metrics() {
        let record = create_test_audit_record();
        let viz = transform_to_visualization(&record);

        assert_eq!(viz.performance_metrics.stage_latencies.len(), 3);
        assert!(viz.performance_metrics.ai_processing_ms.is_some());
    }
}
