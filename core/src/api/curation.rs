//! Curation API Handlers
//!
//! Provides REST endpoints for medication curation management.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::routes::AppState;
use crate::repository::{CurationStats, MedicationAliasModel, MedicationMasterModel};

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestionQuery {
    pub name: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SuggestionResponse {
    pub suggestions: Vec<MasterSuggestion>,
}

#[derive(Debug, Serialize)]
pub struct MasterSuggestion {
    pub id: Uuid,
    pub name: String,
    pub confidence: f32,
    pub source: String, // "semantic" or "fuzzy"
}

#[derive(Debug, Deserialize)]
pub struct CreateMasterRequest {
    pub name: String,
    pub name_ar: Option<String>,
    pub active_ingredient: Option<String>,
    pub strength: Option<String>,
    pub manufacturer: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApproveAliasRequest {
    pub alias_id: Uuid,
    pub master_id: Uuid,
    pub operator_id: String,
}

#[derive(Debug, Serialize)]
pub struct AliasListResponse {
    pub aliases: Vec<MedicationAliasModel>,
    pub total: i64,
}

// =============================================================================
// Handlers
// =============================================================================

/// Get overall curation statistics
pub async fn get_curation_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<CurationStats>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    state
        .medication_alias_repo
        .get_stats()
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// List pending or all medication aliases
pub async fn list_aliases<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<AliasListResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    let limit = pagination.limit.unwrap_or(50);
    let offset = pagination.offset.unwrap_or(0);

    let aliases = state
        .medication_alias_repo
        .get_pending(limit, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = state
        .medication_alias_repo
        .count_pending()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AliasListResponse { aliases, total }))
}

/// Create a new master medication record
pub async fn create_master<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<CreateMasterRequest>,
) -> Result<Json<MedicationMasterModel>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    // Generate embedding for AI search
    let embedding = state.ai_client.embed(&req.name).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("AI error: {}", e),
        )
    })?;

    let mut master = MedicationMasterModel::new(req.name);
    master.canonical_name_ar = req.name_ar;
    master.active_ingredient = req.active_ingredient;
    master.strength = req.strength;
    master.manufacturer = req.manufacturer;
    master.embedding = Some(pgvector::Vector::from(embedding));

    let saved = state
        .medication_master_repo
        .save(&master)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(saved))
}

/// Approve an alias and map it to a master medication
pub async fn approve_alias<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<ApproveAliasRequest>,
) -> Result<StatusCode, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    if let Some(mut alias) = state
        .medication_alias_repo
        .get_by_id(req.alias_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        alias.approve(req.master_id, req.operator_id);
        state
            .medication_alias_repo
            .save(&alias)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        return Err((StatusCode::NOT_FOUND, "Alias not found".to_string()));
    }

    Ok(StatusCode::OK)
}

/// Get AI-driven suggestions for a medication name
pub async fn get_suggestions<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(query): Query<SuggestionQuery>,
) -> Result<Json<SuggestionResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMappingRepository + 'static,
{
    let limit = query.limit.unwrap_or(5);
    let mut suggestions = Vec::new();

    // 1. Semantic Search (AI)
    if let Ok(embedding) = state.ai_client.embed(&query.name).await
        && let Ok(semantic_matches) = state
            .medication_master_repo
            .search_semantic(&embedding, limit)
            .await
    {
        for (model, score) in semantic_matches {
            suggestions.push(MasterSuggestion {
                id: model.id,
                name: model.full_display(),
                confidence: score,
                source: "semantic".to_string(),
            });
        }
    }

    // 2. Fuzzy/Literal Search (Fallback or Hybrid)
    if let Ok(fuzzy_matches) = state
        .medication_master_repo
        .search(&query.name, limit)
        .await
    {
        for model in fuzzy_matches {
            // Avoid duplicates from semantic search
            if suggestions.iter().any(|s| s.id == model.id) {
                continue;
            }
            suggestions.push(MasterSuggestion {
                id: model.id,
                name: model.full_display(),
                confidence: 0.8, // Fixed confidence for fuzzy matches
                source: "fuzzy".to_string(),
            });
        }
    }

    // Sort by confidence
    suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    suggestions.truncate(limit as usize);

    Ok(Json(SuggestionResponse { suggestions }))
}
