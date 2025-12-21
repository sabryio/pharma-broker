//! Embedding Cache API Endpoints
//!
//! REST API for managing medication embedding cache

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::matching::EmbeddingCacheStatsSnapshot;
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

// =============================================================================
// Response Types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct EmbeddingCacheResponse {
    pub stats: EmbeddingCacheStatsSnapshot,
    pub is_empty: bool,
}

#[derive(Debug, Serialize)]
pub struct SynonymCheckResponse {
    pub term1: String,
    pub term2: String,
    pub are_synonyms: bool,
}

#[derive(Debug, Serialize)]
pub struct CanonicalResponse {
    pub term: String,
    pub canonical: Option<String>,
    pub synonyms: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub term: String,
    pub has_embedding: bool,
    pub embedding_dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SynonymCheckRequest {
    pub term1: String,
    pub term2: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/embedding-cache - Get embedding cache stats
pub async fn get_cache_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let response = EmbeddingCacheResponse {
        stats: engine.get_embedding_cache_stats(),
        is_empty: engine.is_embedding_cache_empty(),
    };

    Json(response).into_response()
}

/// POST /api/embedding-cache/refresh - Refresh cache from database
pub async fn refresh_cache<RQ, A, MM>(State(state): State<AppState<RQ, A, MM>>) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    // Fetch all mappings from repository (use large limit to get all)
    match state.medication_mapping_repo.get_all(100_000, 0).await {
        Ok(mappings) => {
            engine.refresh_embedding_cache(&mappings);

            Json(serde_json::json!({
                "message": "Embedding cache refreshed",
                "mappings_loaded": mappings.len(),
                "stats": engine.get_embedding_cache_stats()
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to fetch mappings: {}", e)})),
        )
            .into_response(),
    }
}

/// POST /api/embedding-cache/clear - Clear the cache
pub async fn clear_cache<RQ, A, MM>(State(state): State<AppState<RQ, A, MM>>) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    engine.clear_embedding_cache();

    Json(serde_json::json!({
        "message": "Embedding cache cleared"
    }))
    .into_response()
}

/// GET /api/embedding-cache/lookup/{term} - Lookup a medication term
pub async fn lookup_term<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(term): Path<String>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let canonical = engine.get_canonical_medication(&term);
    let synonyms = engine.get_medication_synonyms(&term);

    let response = CanonicalResponse {
        term,
        canonical,
        synonyms,
    };

    Json(response).into_response()
}

/// POST /api/embedding-cache/synonyms - Check if two terms are synonyms
pub async fn check_synonyms<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<SynonymCheckRequest>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let are_synonyms = engine.are_medications_synonyms(&req.term1, &req.term2);

    let response = SynonymCheckResponse {
        term1: req.term1,
        term2: req.term2,
        are_synonyms,
    };

    Json(response).into_response()
}

/// GET /api/embedding-cache/embedding/{term} - Get embedding info for a term
pub async fn get_embedding<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(term): Path<String>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let Some(engine) = &state.matching_engine else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Matching engine not available"})),
        )
            .into_response();
    };

    let embedding = engine.get_medication_embedding(&term);

    let response = EmbeddingResponse {
        term,
        has_embedding: embedding.is_some(),
        embedding_dimensions: embedding.map(|e| e.len()),
    };

    Json(response).into_response()
}
