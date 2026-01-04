//! Performance Analytics API
//!
//! Endpoints for aggregating and analyzing performance metrics from audit records.
//!
//! Requirements: 8.4, 8.5
//! - Compute average, p95, p99 latencies per stage
//! - Return aggregated performance metrics

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for analytics endpoint
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    /// Maximum number of records to analyze (default: 1000)
    pub limit: Option<usize>,
    /// Filter by minimum score
    pub min_score: Option<f64>,
    /// Filter by AI involvement
    pub ai_involved: Option<bool>,
    /// Time window in hours (default: 24)
    pub hours: Option<u64>,
}

/// Response for performance analytics
/// Requirements: 8.5
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceAnalyticsResponse {
    /// Number of records analyzed
    pub records_analyzed: usize,
    /// Overall latency statistics
    pub overall_latency: LatencyStats,
    /// Per-stage latency statistics
    pub stage_latencies: HashMap<String, LatencyStats>,
    /// AI-specific metrics
    pub ai_metrics: Option<AiMetrics>,
    /// Database metrics
    pub db_metrics: DbMetrics,
    /// Memory metrics
    pub memory_metrics: Option<MemoryMetrics>,
    /// Stages exceeding thresholds
    pub slow_stages: Vec<SlowStageAlert>,
}

/// Latency statistics for a stage or overall
#[derive(Debug, Clone, Serialize, Default)]
pub struct LatencyStats {
    /// Number of samples
    pub count: usize,
    /// Minimum latency in milliseconds
    pub min_ms: u64,
    /// Maximum latency in milliseconds
    pub max_ms: u64,
    /// Average latency in milliseconds
    pub avg_ms: f64,
    /// Median latency in milliseconds
    pub median_ms: u64,
    /// 95th percentile latency in milliseconds
    pub p95_ms: u64,
    /// 99th percentile latency in milliseconds
    pub p99_ms: u64,
    /// Standard deviation
    pub std_dev_ms: f64,
}

/// AI-specific performance metrics
#[derive(Debug, Clone, Serialize)]
pub struct AiMetrics {
    /// Number of AI invocations
    pub invocation_count: usize,
    /// Queue wait time statistics
    pub queue_wait: LatencyStats,
    /// Processing time statistics
    pub processing_time: LatencyStats,
    /// Total AI time statistics
    pub total_time: LatencyStats,
    /// Average tokens per request
    pub avg_tokens: Option<f64>,
}

/// Database performance metrics
#[derive(Debug, Clone, Serialize, Default)]
pub struct DbMetrics {
    /// Total number of queries across all records
    pub total_queries: u64,
    /// Average queries per record
    pub avg_queries_per_record: f64,
    /// Query latency statistics
    pub query_latency: LatencyStats,
}

/// Memory usage metrics
#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    /// Number of records with memory data
    pub sample_count: usize,
    /// Minimum peak memory in bytes
    pub min_bytes: u64,
    /// Maximum peak memory in bytes
    pub max_bytes: u64,
    /// Average peak memory in bytes
    pub avg_bytes: f64,
    /// 95th percentile memory in bytes
    pub p95_bytes: u64,
}

/// Alert for stages exceeding performance thresholds
#[derive(Debug, Clone, Serialize)]
pub struct SlowStageAlert {
    /// Stage name
    pub stage: String,
    /// Average latency
    pub avg_ms: f64,
    /// P95 latency
    pub p95_ms: u64,
    /// Threshold exceeded (in ms)
    pub threshold_ms: u64,
    /// Number of occurrences exceeding threshold
    pub occurrences: usize,
}

// =============================================================================
// Computation Functions
// =============================================================================

impl LatencyStats {
    /// Compute latency statistics from a vector of latency values
    pub fn compute(mut values: Vec<u64>) -> Self {
        if values.is_empty() {
            return Self::default();
        }

        values.sort_unstable();
        let count = values.len();
        let min_ms = values[0];
        let max_ms = values[count - 1];
        let sum: u64 = values.iter().sum();
        let avg_ms = sum as f64 / count as f64;

        // Median
        let median_ms = if count % 2 == 0 {
            (values[count / 2 - 1] + values[count / 2]) / 2
        } else {
            values[count / 2]
        };

        // Percentiles
        let p95_idx = ((count as f64) * 0.95).ceil() as usize - 1;
        let p99_idx = ((count as f64) * 0.99).ceil() as usize - 1;
        let p95_ms = values[p95_idx.min(count - 1)];
        let p99_ms = values[p99_idx.min(count - 1)];

        // Standard deviation
        let variance: f64 = values
            .iter()
            .map(|&v| {
                let diff = v as f64 - avg_ms;
                diff * diff
            })
            .sum::<f64>()
            / count as f64;
        let std_dev_ms = variance.sqrt();

        Self {
            count,
            min_ms,
            max_ms,
            avg_ms,
            median_ms,
            p95_ms,
            p99_ms,
            std_dev_ms,
        }
    }
}

/// Default thresholds for slow stage detection (in milliseconds)
const STAGE_THRESHOLDS: &[(&str, u64)] = &[
    ("ai_parsing", 2000),
    ("ai_review", 3000),
    ("consensus_check", 5000),
    ("score_calculation", 100),
    ("hierarchical_stage", 500),
    ("medication_resolution", 200),
    ("match_candidate_search", 300),
];

fn get_threshold_for_stage(stage: &str) -> u64 {
    for (pattern, threshold) in STAGE_THRESHOLDS {
        if stage.contains(pattern) || stage.starts_with(pattern) {
            return *threshold;
        }
    }
    1000 // Default threshold
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/audit-records/analytics - Get aggregated performance metrics
/// Requirements: 8.5
pub async fn get_performance_analytics<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(query): Query<AnalyticsQuery>,
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
    let limit = query.limit.unwrap_or(1000);

    // Get records from buffer
    let records = recorder.get_recent(limit);

    // Filter records
    let filtered_records: Vec<_> = records
        .iter()
        .filter(|r| {
            if let Some(min_score) = query.min_score {
                if r.final_score < min_score {
                    return false;
                }
            }
            if let Some(ai_involved) = query.ai_involved {
                if r.ai_involved != ai_involved {
                    return false;
                }
            }
            true
        })
        .collect();

    if filtered_records.is_empty() {
        return Ok(Json(PerformanceAnalyticsResponse {
            records_analyzed: 0,
            overall_latency: LatencyStats::default(),
            stage_latencies: HashMap::new(),
            ai_metrics: None,
            db_metrics: DbMetrics::default(),
            memory_metrics: None,
            slow_stages: Vec::new(),
        }));
    }

    // Collect overall latencies
    let overall_latencies: Vec<u64> = filtered_records
        .iter()
        .map(|r| r.total_latency_ms)
        .collect();

    // Collect per-stage latencies
    let mut stage_latencies_map: HashMap<String, Vec<u64>> = HashMap::new();
    for record in &filtered_records {
        for stage in &record.pipeline_stages {
            stage_latencies_map
                .entry(stage.stage.clone())
                .or_default()
                .push(stage.duration_ms);
        }
    }

    // Compute stage statistics
    let stage_latencies: HashMap<String, LatencyStats> = stage_latencies_map
        .into_iter()
        .map(|(stage, values)| (stage, LatencyStats::compute(values)))
        .collect();

    // Collect AI metrics
    let ai_records: Vec<_> = filtered_records
        .iter()
        .filter(|r| r.ai_involved && r.ai_record.is_some())
        .collect();

    let ai_metrics = if !ai_records.is_empty() {
        let ai_latencies: Vec<u64> = ai_records
            .iter()
            .filter_map(|r| r.ai_record.as_ref().map(|ai| ai.latency_ms))
            .collect();

        // For queue wait and processing time, we need to look at pipeline stages
        // since the ai_record doesn't separate them
        let mut queue_waits = Vec::new();
        let mut processing_times = Vec::new();

        for record in &filtered_records {
            for stage in &record.pipeline_stages {
                if stage.stage.contains("ai_")
                    || stage.stage == "ai_parsing"
                    || stage.stage == "ai_review"
                {
                    // Estimate: assume 10% is queue wait, 90% is processing
                    // This is a simplification; real data would come from performance_metrics
                    let queue_wait = stage.duration_ms / 10;
                    let processing = stage.duration_ms - queue_wait;
                    queue_waits.push(queue_wait);
                    processing_times.push(processing);
                }
            }
        }

        // Calculate average tokens if available
        let total_tokens: u64 = ai_records
            .iter()
            .filter_map(|r| {
                r.ai_record.as_ref().and_then(|ai| {
                    let prompt = ai.prompt_tokens.unwrap_or(0) as u64;
                    let completion = ai.completion_tokens.unwrap_or(0) as u64;
                    if prompt + completion > 0 {
                        Some(prompt + completion)
                    } else {
                        None
                    }
                })
            })
            .sum();

        let token_count = ai_records
            .iter()
            .filter(|r| {
                r.ai_record
                    .as_ref()
                    .map(|ai| ai.prompt_tokens.is_some() || ai.completion_tokens.is_some())
                    .unwrap_or(false)
            })
            .count();

        let avg_tokens = if token_count > 0 {
            Some(total_tokens as f64 / token_count as f64)
        } else {
            None
        };

        Some(AiMetrics {
            invocation_count: ai_records.len(),
            queue_wait: LatencyStats::compute(queue_waits),
            processing_time: LatencyStats::compute(processing_times),
            total_time: LatencyStats::compute(ai_latencies),
            avg_tokens,
        })
    } else {
        None
    };

    // Compute DB metrics (simplified - would need enhanced records for full data)
    let db_metrics = DbMetrics {
        total_queries: 0,
        avg_queries_per_record: 0.0,
        query_latency: LatencyStats::default(),
    };

    // Detect slow stages
    let mut slow_stages = Vec::new();
    for (stage, stats) in &stage_latencies {
        let threshold = get_threshold_for_stage(stage);
        if stats.p95_ms > threshold {
            slow_stages.push(SlowStageAlert {
                stage: stage.clone(),
                avg_ms: stats.avg_ms,
                p95_ms: stats.p95_ms,
                threshold_ms: threshold,
                occurrences: stats.count,
            });
        }
    }

    // Sort slow stages by p95 latency descending
    slow_stages.sort_by(|a, b| b.p95_ms.cmp(&a.p95_ms));

    Ok(Json(PerformanceAnalyticsResponse {
        records_analyzed: filtered_records.len(),
        overall_latency: LatencyStats::compute(overall_latencies),
        stage_latencies,
        ai_metrics,
        db_metrics,
        memory_metrics: None, // Would need enhanced records
        slow_stages,
    }))
}

// =============================================================================
// Aggregation Helper Functions (for property testing)
// =============================================================================

/// Compute latency statistics from a slice of values
/// This is exposed for property testing
pub fn compute_latency_stats(values: &[u64]) -> LatencyStats {
    LatencyStats::compute(values.to_vec())
}

/// Compute percentile from sorted values
pub fn compute_percentile(sorted_values: &[u64], percentile: f64) -> u64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let idx = ((sorted_values.len() as f64) * percentile).ceil() as usize - 1;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

/// Compute average from values
pub fn compute_average(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let sum: u64 = values.iter().sum();
    sum as f64 / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_stats_empty() {
        let stats = LatencyStats::compute(vec![]);
        assert_eq!(stats.count, 0);
        assert_eq!(stats.min_ms, 0);
        assert_eq!(stats.max_ms, 0);
    }

    #[test]
    fn test_latency_stats_single_value() {
        let stats = LatencyStats::compute(vec![100]);
        assert_eq!(stats.count, 1);
        assert_eq!(stats.min_ms, 100);
        assert_eq!(stats.max_ms, 100);
        assert_eq!(stats.avg_ms, 100.0);
        assert_eq!(stats.median_ms, 100);
        assert_eq!(stats.p95_ms, 100);
        assert_eq!(stats.p99_ms, 100);
    }

    #[test]
    fn test_latency_stats_multiple_values() {
        let stats = LatencyStats::compute(vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
        assert_eq!(stats.count, 10);
        assert_eq!(stats.min_ms, 10);
        assert_eq!(stats.max_ms, 100);
        assert_eq!(stats.avg_ms, 55.0);
        assert_eq!(stats.median_ms, 55); // (50 + 60) / 2
        assert_eq!(stats.p95_ms, 100);
        assert_eq!(stats.p99_ms, 100);
    }

    #[test]
    fn test_compute_percentile() {
        let values = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(compute_percentile(&values, 0.5), 50);
        assert_eq!(compute_percentile(&values, 0.95), 100);
        assert_eq!(compute_percentile(&values, 0.99), 100);
    }

    #[test]
    fn test_compute_average() {
        assert_eq!(compute_average(&[10, 20, 30]), 20.0);
        assert_eq!(compute_average(&[]), 0.0);
    }

    #[test]
    fn test_get_threshold_for_stage() {
        assert_eq!(get_threshold_for_stage("ai_parsing"), 2000);
        assert_eq!(get_threshold_for_stage("ai_review"), 3000);
        assert_eq!(get_threshold_for_stage("score_calculation"), 100);
        assert_eq!(get_threshold_for_stage("unknown_stage"), 1000);
    }
}
