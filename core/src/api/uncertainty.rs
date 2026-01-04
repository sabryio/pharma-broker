//! Uncertainty Estimation API
//!
//! Endpoints for estimating match prediction uncertainty.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::routes::AppState;
use crate::matching::{UncertaintyConfig, UncertaintyEstimator, UncertaintyResult, Weights};
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct EstimateUncertaintyRequest {
    pub offer_id: Uuid,
    pub request_id: Uuid,
    /// Optional custom config
    pub config: Option<UncertaintyConfigRequest>,
}

#[derive(Debug, Deserialize)]
pub struct UncertaintyConfigRequest {
    pub num_samples: Option<usize>,
    pub perturbation_std: Option<f64>,
    pub confidence_level: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct UncertaintyResponse {
    pub offer_id: Uuid,
    pub request_id: Uuid,
    pub result: UncertaintyResultResponse,
}

#[derive(Debug, Serialize)]
pub struct UncertaintyResultResponse {
    pub mean_score: f64,
    pub std_dev: f64,
    pub coefficient_of_variation: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub num_samples: usize,
    pub is_certain: bool,
    pub original_score: f64,
    pub uncertainty_level: String,
    pub is_robust: bool,
}

impl From<UncertaintyResult> for UncertaintyResultResponse {
    fn from(r: UncertaintyResult) -> Self {
        Self {
            mean_score: r.mean_score,
            std_dev: r.std_dev,
            coefficient_of_variation: r.coefficient_of_variation,
            ci_lower: r.ci_lower,
            ci_upper: r.ci_upper,
            num_samples: r.num_samples,
            is_certain: r.is_certain,
            original_score: r.original_score,
            uncertainty_level: r.uncertainty_level().to_string(),
            is_robust: r.is_robust(0.05),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UncertaintyStatusResponse {
    pub default_config: UncertaintyConfigResponse,
}

#[derive(Debug, Serialize)]
pub struct UncertaintyConfigResponse {
    pub num_samples: usize,
    pub perturbation_std: f64,
    pub confidence_level: f64,
    pub max_uncertainty_threshold: f64,
    pub symmetric_perturbation: bool,
}

impl From<&UncertaintyConfig> for UncertaintyConfigResponse {
    fn from(c: &UncertaintyConfig) -> Self {
        Self {
            num_samples: c.num_samples,
            perturbation_std: c.perturbation_std,
            confidence_level: c.confidence_level,
            max_uncertainty_threshold: c.max_uncertainty_threshold,
            symmetric_perturbation: c.symmetric_perturbation,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BatchEstimateRequest {
    pub pairs: Vec<MatchPair>,
    pub config: Option<UncertaintyConfigRequest>,
}

#[derive(Debug, Deserialize)]
pub struct MatchPair {
    pub offer_id: Uuid,
    pub request_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct BatchUncertaintyResponse {
    pub results: Vec<UncertaintyResponse>,
    pub summary: BatchSummary,
}

#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub total: usize,
    pub certain_count: usize,
    pub uncertain_count: usize,
    pub avg_std_dev: f64,
    pub avg_mean_score: f64,
}

// =============================================================================
// Handlers
// =============================================================================

/// POST /api/uncertainty/estimate - Estimate uncertainty for a match
pub async fn estimate_uncertainty<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<EstimateUncertaintyRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Fetch offer and request using correct method name: get_by_id
    let offer = state
        .offer_repo
        .get_by_id(req.offer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Offer {} not found", req.offer_id),
        ))?;

    let request = state
        .request_repo
        .get_by_id(req.request_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Request {} not found", req.request_id),
        ))?;

    // Get weights from engine or use defaults
    let weights = if let Some(engine) = &state.matching_engine {
        engine.get_weights()
    } else {
        Weights::default()
    };

    // Build config
    let config = if let Some(cfg) = req.config {
        UncertaintyConfig {
            num_samples: cfg.num_samples.unwrap_or(20),
            perturbation_std: cfg.perturbation_std.unwrap_or(0.1),
            confidence_level: cfg.confidence_level.unwrap_or(0.95),
            ..UncertaintyConfig::default()
        }
    } else {
        UncertaintyConfig::default()
    };

    let estimator = UncertaintyEstimator::new(config, weights);
    let result = estimator.estimate(&offer, &request);

    Ok(Json(UncertaintyResponse {
        offer_id: req.offer_id,
        request_id: req.request_id,
        result: result.into(),
    }))
}

/// POST /api/uncertainty/batch - Batch estimate uncertainty
pub async fn batch_estimate_uncertainty<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BatchEstimateRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Get weights from engine or use defaults
    let weights = if let Some(engine) = &state.matching_engine {
        engine.get_weights()
    } else {
        Weights::default()
    };

    // Build config
    let config = if let Some(cfg) = req.config {
        UncertaintyConfig {
            num_samples: cfg.num_samples.unwrap_or(10), // Fewer samples for batch
            perturbation_std: cfg.perturbation_std.unwrap_or(0.1),
            confidence_level: cfg.confidence_level.unwrap_or(0.95),
            ..UncertaintyConfig::default()
        }
    } else {
        UncertaintyConfig::fast() // Use fast config for batch
    };

    let estimator = UncertaintyEstimator::new(config, weights);
    let mut results = Vec::with_capacity(req.pairs.len());
    let mut total_std_dev = 0.0;
    let mut total_mean = 0.0;
    let mut certain_count = 0;

    for pair in &req.pairs {
        // Fetch offer and request using get_by_id
        let offer = match state.offer_repo.get_by_id(pair.offer_id).await {
            Ok(Some(o)) => o,
            _ => continue,
        };

        let request = match state.request_repo.get_by_id(pair.request_id).await {
            Ok(Some(r)) => r,
            _ => continue,
        };

        let result = estimator.estimate(&offer, &request);
        total_std_dev += result.std_dev;
        total_mean += result.mean_score;
        if result.is_certain {
            certain_count += 1;
        }

        results.push(UncertaintyResponse {
            offer_id: pair.offer_id,
            request_id: pair.request_id,
            result: result.into(),
        });
    }

    let total = results.len();
    let summary = BatchSummary {
        total,
        certain_count,
        uncertain_count: total - certain_count,
        avg_std_dev: if total > 0 {
            total_std_dev / total as f64
        } else {
            0.0
        },
        avg_mean_score: if total > 0 {
            total_mean / total as f64
        } else {
            0.0
        },
    };

    Ok(Json(BatchUncertaintyResponse { results, summary }))
}

/// GET /api/uncertainty/status - Get uncertainty estimator status
pub async fn get_uncertainty_status<RQ, A, MM>(
    State(_state): State<AppState<RQ, A, MM>>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let default_config = UncertaintyConfig::from_env();

    Ok(Json(UncertaintyStatusResponse {
        default_config: (&default_config).into(),
    }))
}

/// GET /api/uncertainty/match/:match_id - Estimate uncertainty for existing match
pub async fn estimate_match_uncertainty<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(match_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Fetch the match using get_by_id
    let match_entity = state
        .match_repo
        .get_by_id(match_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Match {} not found", match_id),
        ))?;

    // Fetch offer and request using get_by_id
    let offer = state
        .offer_repo
        .get_by_id(match_entity.offer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Offer not found".to_string()))?;

    let request = state
        .request_repo
        .get_by_id(match_entity.request_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Request not found".to_string()))?;

    // Get weights from engine or use defaults
    let weights = if let Some(engine) = &state.matching_engine {
        engine.get_weights()
    } else {
        Weights::default()
    };

    let estimator = UncertaintyEstimator::new(UncertaintyConfig::default(), weights);
    let result = estimator.estimate(&offer, &request);

    Ok(Json(UncertaintyResponse {
        offer_id: match_entity.offer_id,
        request_id: match_entity.request_id,
        result: result.into(),
    }))
}
