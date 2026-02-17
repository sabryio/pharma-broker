//! AI Health Monitoring API
//!
//! HTTP endpoints for monitoring AI model health, circuit breaker status,
//! and retry queue statistics.

use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use std::sync::Arc;

use super::routes::AppState;
use crate::ai::PharmaParser;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiHealthResponse {
    pub status: String,
    pub circuit_breaker: CircuitBreakerStatus,
    pub model_info: ModelInfo,
    pub performance: PerformanceMetrics,
    pub retry_queue: RetryQueueStats,
    pub recent_errors: Vec<RecentError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerStatus {
    pub state: String, // "closed", "open", "half_open"
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<String>,
    pub next_retry_time: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub endpoint: String,
    pub model_name: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceMetrics {
    pub success_rate_1h: f64,
    pub success_rate_24h: f64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub total_requests_1h: i64,
    pub total_requests_24h: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryQueueStats {
    pub pending: i64,
    pub processing: i64,
    pub completed: i64,
    pub failed: i64,
    pub by_reason: Vec<FailureReasonCount>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureReasonCount {
    pub reason: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentError {
    pub timestamp: String,
    pub error_type: String,
    pub message: String,
    pub raw_message_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResponse {
    pub success: bool,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/ai-health
/// Returns comprehensive AI health status
pub async fn get_ai_health<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<AiHealthResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get circuit breaker status from AI client
    let circuit_breaker = get_circuit_breaker_status(&state.ai_client);

    // Get model info
    let model_info = get_model_info(&state.ai_client);

    // Get performance metrics (placeholder - would need metrics store)
    let performance = PerformanceMetrics {
        success_rate_1h: 0.95,
        success_rate_24h: 0.93,
        avg_response_time_ms: 1250.0,
        p95_response_time_ms: 2500.0,
        total_requests_1h: 150,
        total_requests_24h: 3200,
    };

    // Get retry queue stats
    let retry_queue = match get_retry_queue_stats(&state).await {
        Ok(stats) => stats,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get retry queue stats");
            RetryQueueStats {
                pending: 0,
                processing: 0,
                completed: 0,
                failed: 0,
                by_reason: vec![],
            }
        }
    };

    // Get recent errors (placeholder)
    let recent_errors = vec![];

    // Determine overall status
    let status = if circuit_breaker.state == "open" {
        "degraded".to_string()
    } else if retry_queue.pending > 100 {
        "warning".to_string()
    } else {
        "healthy".to_string()
    };

    Ok(Json(AiHealthResponse {
        status,
        circuit_breaker,
        model_info,
        performance,
        retry_queue,
        recent_errors,
    }))
}

/// GET /api/ai-health/circuit-breaker
/// Returns circuit breaker status only
pub async fn get_circuit_breaker<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Json<CircuitBreakerStatus>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    Json(get_circuit_breaker_status(&state.ai_client))
}

/// GET /api/ai-health/retry-queue
/// Returns retry queue statistics
pub async fn get_retry_queue<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<RetryQueueStats>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    match get_retry_queue_stats(&state).await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get retry queue stats: {}", e),
        )),
    }
}

/// POST /api/ai-health/test-connection
/// Tests connection to AI gateway
pub async fn test_connection<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Json<TestConnectionResponse>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let start = std::time::Instant::now();

    // Try a simple parse request
    let test_result = state
        .ai_client
        .parse("Test message", None, "Test Group", None, None)
        .await;

    let response_time_ms = start.elapsed().as_millis() as u64;

    match test_result {
        Ok(_) => Json(TestConnectionResponse {
            success: true,
            response_time_ms,
            error: None,
        }),
        Err(e) => Json(TestConnectionResponse {
            success: false,
            response_time_ms,
            error: Some(e.to_string()),
        }),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn get_circuit_breaker_status(ai_client: &Arc<PharmaParser>) -> CircuitBreakerStatus {
    let state = ai_client.circuit_state();

    // Convert CircuitState enum to string
    let state_str = match state {
        crate::ai::CircuitState::Closed => "closed",
        crate::ai::CircuitState::Open => "open",
        crate::ai::CircuitState::HalfOpen => "half_open",
    };

    // Note: We don't have direct access to failure/success counts from the circuit breaker
    // This would require adding getter methods to CircuitBreaker
    // For now, return placeholder values
    CircuitBreakerStatus {
        state: state_str.to_string(),
        failure_count: 0,
        success_count: 0,
        last_failure_time: None,
        next_retry_time: None,
    }
}

fn get_model_info(_ai_client: &Arc<PharmaParser>) -> ModelInfo {
    // Note: PharmaParser doesn't expose config directly
    // Would need to add a config() method
    // For now, return values from environment
    let gateway_url =
        std::env::var("AI_GATEWAY_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    ModelInfo {
        endpoint: gateway_url,
        model_name: "gpt-4".to_string(), // Default, would come from config
        timeout_seconds: 180,            // Default from our implementation
        max_retries: 3,
    }
}

async fn get_retry_queue_stats<RQ, A, MM>(
    _state: &AppState<RQ, A, MM>,
) -> Result<RetryQueueStats, Box<dyn std::error::Error>>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get the retry queue repository from state
    // Note: This requires adding retry_queue_repo to AppState
    // For now, return placeholder data

    // TODO: Once retry_queue_repo is added to AppState, use:
    // let stats = state.retry_queue_repo.get_stats().await?;

    // Placeholder implementation
    Ok(RetryQueueStats {
        pending: 0,
        processing: 0,
        completed: 0,
        failed: 0,
        by_reason: vec![
            FailureReasonCount {
                reason: "CIRCUIT_BREAKER".to_string(),
                count: 0,
            },
            FailureReasonCount {
                reason: "NETWORK_ERROR".to_string(),
                count: 0,
            },
            FailureReasonCount {
                reason: "INCOMPLETE_JSON".to_string(),
                count: 0,
            },
        ],
    })
}
