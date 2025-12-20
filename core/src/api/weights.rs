//! Weight Management API Handlers
//!
//! REST endpoints for managing matching weights:
//! - GET  /api/weights          - Get current weights
//! - PUT  /api/weights          - Update weights manually
//! - GET  /api/weights/scheduler - Get scheduler status
//! - POST /api/weights/scheduler/run - Trigger learning job
//! - GET  /api/weights/influence - Get warm start influence
//! - GET  /api/weights/abtest   - List A/B tests
//! - POST /api/weights/abtest   - Create A/B test

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::matching::{
    ABTestConfig, ABTestResult, MatchingEngineHandle, PerformanceMetrics, SchedulerStatus, Weights,
};
use crate::repository::{GroupRepository, MatchRepository, OfferRepository, RequestRepository};

use super::routes::AppState;

// =============================================================================
// DTOs (Data Transfer Objects)
// =============================================================================

/// Response for current weights
#[derive(Debug, Serialize)]
pub struct WeightsResponse {
    pub weights: Weights,
    pub prior_influence: f64,
    pub sample_count: usize,
}

/// Request to update weights
#[derive(Debug, Deserialize)]
pub struct UpdateWeightsRequest {
    pub medication: f64,
    pub dosage: f64,
    pub quantity: f64,
    pub price: f64,
    pub recency: f64,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Response for scheduler status
#[derive(Debug, Serialize)]
pub struct SchedulerStatusResponse {
    pub enabled: bool,
    pub schedule: String,
    pub last_run: Option<DateTime<Utc>>,
    pub last_status: String,
    pub last_error: Option<String>,
    pub pending_weights: Option<Weights>,
    pub pending_reason: Option<String>,
}

/// Request to create A/B test
#[derive(Debug, Deserialize)]
pub struct CreateABTestRequest {
    pub test_id: String,
    pub name: String,
    pub description: String,
    #[serde(default = "default_control_pct")]
    pub control_pct: f64,
    pub test_weights: Weights,
    pub duration_hours: i64,
    #[serde(default = "default_min_samples")]
    pub min_samples: usize,
}

fn default_control_pct() -> f64 {
    0.5
}

fn default_min_samples() -> usize {
    100
}

/// Response for warm start influence
#[derive(Debug, Serialize)]
pub struct InfluenceResponse {
    pub prior_influence_pct: f64,
    pub sample_count: usize,
    pub effective_weights: Weights,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/weights - Get current weights and influence
pub async fn get_weights<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    let weights = engine.get_weights();
    let prior_influence = engine.get_prior_influence().await;
    let sample_count = engine.get_sample_count().await;

    Ok(Json(WeightsResponse {
        weights,
        prior_influence,
        sample_count,
    }))
}

/// PUT /api/weights - Update weights manually
pub async fn update_weights<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
    Json(req): Json<UpdateWeightsRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    // Validate weights sum approximately to 1.0
    let sum = req.medication + req.dosage + req.quantity + req.price + req.recency;
    if (sum - 1.0).abs() > 0.01 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Weights must sum to 1.0, got {:.3}", sum),
        ));
    }

    // Validate all weights are positive
    if req.medication < 0.0
        || req.dosage < 0.0
        || req.quantity < 0.0
        || req.price < 0.0
        || req.recency < 0.0
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "All weights must be non-negative".to_string(),
        ));
    }

    let new_weights = Weights {
        medication: req.medication,
        dosage: req.dosage,
        quantity: req.quantity,
        price: req.price,
        recency: req.recency,
    };

    let reason = req
        .reason
        .unwrap_or_else(|| "Manual update via API".to_string());
    engine.apply_weights(new_weights.clone(), &reason).await;

    tracing::info!(
        medication = %new_weights.medication,
        dosage = %new_weights.dosage,
        reason = %reason,
        "Weights updated via API"
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "weights": new_weights,
        "message": "Weights updated successfully"
    })))
}

/// GET /api/weights/scheduler - Get scheduler status
pub async fn get_scheduler_status<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    let status = engine.scheduler_status().await;

    Ok(Json(SchedulerStatusResponse {
        enabled: status.enabled,
        schedule: status.schedule,
        last_run: status.last_run,
        last_status: format!("{:?}", status.last_status),
        last_error: status.last_error,
        pending_weights: status.pending_apply,
        pending_reason: status.pending_reason,
    }))
}

/// GET /api/weights/influence - Get warm start influence
pub async fn get_influence<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    let sample_count = engine.get_sample_count().await;
    let prior_influence = engine.get_prior_influence().await;
    let effective_weights = engine.get_weights();

    Ok(Json(InfluenceResponse {
        prior_influence_pct: prior_influence,
        sample_count,
        effective_weights,
    }))
}

/// GET /api/weights/abtest - List all A/B tests
pub async fn list_ab_tests<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    let active_tests = engine.get_active_ab_tests();

    Ok(Json(serde_json::json!({
        "active_tests": active_tests.iter().map(|t| serde_json::json!({
            "test_id": t.test_id,
            "name": t.name,
            "description": t.description,
            "control_pct": t.control_pct,
            "start_time": t.start_time,
            "end_time": t.end_time,
            "min_samples": t.min_samples,
        })).collect::<Vec<_>>(),
        "count": active_tests.len()
    })))
}

/// POST /api/weights/abtest - Create a new A/B test
pub async fn create_ab_test<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
    Json(req): Json<CreateABTestRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    // Validate control percentage
    if req.control_pct <= 0.0 || req.control_pct >= 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "control_pct must be between 0 and 1 (exclusive)".to_string(),
        ));
    }

    let config = ABTestConfig {
        test_id: req.test_id.clone(),
        name: req.name,
        description: req.description,
        control_pct: req.control_pct,
        test_weights: req.test_weights,
        start_time: Utc::now(),
        end_time: Utc::now() + Duration::hours(req.duration_hours),
        min_samples: req.min_samples,
        active: true,
    };

    engine
        .create_ab_test(config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    tracing::info!(test_id = %req.test_id, "A/B test created via API");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "test_id": req.test_id,
            "message": "A/B test created successfully"
        })),
    ))
}

/// GET /api/weights/abtest/:id - Get A/B test result
pub async fn get_ab_test_result<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
    Path(test_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    match engine.get_ab_test_result(&test_id) {
        Some(result) => Ok(Json(serde_json::json!({
            "test_id": result.test_id,
            "control_samples": result.control_samples,
            "control_confirmed": result.control_confirmed,
            "control_avg_score": result.control_avg_score,
            "test_samples": result.test_samples,
            "test_confirmed": result.test_confirmed,
            "test_avg_score": result.test_avg_score,
            "statistically_significant": result.statistically_significant,
            "p_value": result.p_value,
            "uplift": result.uplift,
            "start_time": result.start_time,
            "last_updated": result.last_updated,
        }))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Test '{}' not found", test_id),
        )),
    }
}

/// DELETE /api/weights/abtest/:id - End an A/B test
pub async fn end_ab_test<O, R, M, G>(
    State(state): State<AppState<O, R, M, G>>,
    Path(test_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
{
    let engine = match &state.matching_engine {
        Some(engine) => engine,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Matching engine not available".to_string(),
            ));
        }
    };

    match engine.end_ab_test(&test_id) {
        Some(result) => {
            tracing::info!(test_id = %test_id, "A/B test ended via API");
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "A/B test ended",
                "final_result": {
                    "test_id": result.test_id,
                    "control_samples": result.control_samples,
                    "test_samples": result.test_samples,
                    "uplift": result.uplift,
                    "statistically_significant": result.statistically_significant,
                }
            })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Test '{}' not found", test_id),
        )),
    }
}
