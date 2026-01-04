//! Curation API Handlers
//!
//! Provides REST endpoints for medication curation management.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::routes::AppState;
use crate::repository::MedicationMasterModel;
use pharma_db::repo::normalize_arabic_text;

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestionQuery {
    pub name: String,
    pub limit: Option<i64>,
}

/// Master medication DTO for frontend
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterDto {
    pub id: Uuid,
    pub name: String,
    pub canonical_name_ar: Option<String>,
    pub active_ingredient: Option<String>,
    pub strength: Option<String>,
    pub manufacturer: Option<String>,
}

impl From<&MedicationMasterModel> for MasterDto {
    fn from(m: &MedicationMasterModel) -> Self {
        Self {
            id: m.id,
            name: m.canonical_name.clone(),
            canonical_name_ar: m.canonical_name_ar.clone(),
            active_ingredient: m.active_ingredient.clone(),
            strength: m.strength.clone(),
            manufacturer: m.manufacturer.clone(),
        }
    }
}

/// Suggestion with nested master object (matches frontend schema)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterSuggestion {
    pub master: MasterDto,
    pub score: f32,
    pub method: String, // "semantic" or "fuzzy"
}

/// Alias DTO for frontend
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasDto {
    pub id: Uuid,
    pub alias_name: String,
    pub master_medication_id: Option<Uuid>,
    pub curation_status: String,
    pub occurrence_count: i32,
    pub first_seen_at: Option<String>,
    pub ai_suggestion_confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMasterRequest {
    pub name: Option<String>,
    pub name_ar: Option<String>,
    pub active_ingredient: Option<String>,
    pub strength: Option<String>,
    pub manufacturer: Option<String>,
    pub alias_id: Option<Uuid>, // Optional: link to existing alias after creation
    pub alias_name: Option<String>, // Optional: create new alias with this name
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveAliasPathRequest {
    pub master_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasListResponse {
    pub aliases: Vec<AliasDto>,
    pub total: i64,
}

/// Curation stats matching frontend schema
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationStatsDto {
    pub total_aliases: i64,
    pub pending_count: i64,
    pub approved_count: i64,
    pub rejected_count: i64,
    pub curation_percentage: f64,
}

/// Response for create master with link
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMasterResponse {
    pub success: bool,
    pub master: MasterDto,
}

// =============================================================================
// Handlers
// =============================================================================

/// Get overall curation statistics
pub async fn get_curation_stats<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<CurationStatsDto>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    let stats = state
        .medication_alias_repo
        .get_stats()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = stats.total_aliases;
    let pending = stats.pending_aliases;
    let rejected = stats.rejected_aliases;
    let approved = total - pending - rejected;

    let percentage = if total > 0 {
        ((total - pending) as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(CurationStatsDto {
        total_aliases: total,
        pending_count: pending,
        approved_count: approved,
        rejected_count: rejected,
        curation_percentage: percentage,
    }))
}

/// List pending or all medication aliases
pub async fn list_aliases<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<AliasListResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    let limit = pagination.limit.unwrap_or(50);
    let offset = pagination.offset.unwrap_or(0);
    let status = pagination.status.as_deref().unwrap_or("Pending");

    let aliases = if status == "All" {
        state
            .medication_alias_repo
            .get_all(limit, offset)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        state
            .medication_alias_repo
            .get_pending(limit, offset)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let total = if status == "All" {
        state
            .medication_alias_repo
            .count_all()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        state
            .medication_alias_repo
            .count_pending()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let alias_dtos: Vec<AliasDto> = aliases
        .iter()
        .map(|a| AliasDto {
            id: a.id,
            alias_name: a.alias_name.clone(),
            master_medication_id: a.master_medication_id,
            curation_status: format!("{:?}", a.curation_status),
            occurrence_count: a.occurrence_count,
            first_seen_at: Some(a.first_seen_at.to_rfc3339()),
            ai_suggestion_confidence: a.ai_suggestion_confidence,
        })
        .collect();

    Ok(Json(AliasListResponse {
        aliases: alias_dtos,
        total,
    }))
}

/// Create a new master medication record and optionally link to alias
pub async fn create_master<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<CreateMasterRequest>,
) -> Result<Json<CreateMasterResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    tracing::info!(">>> create_master called with: {:?}", req);

    let name = req.name.clone().or(req.name_ar.clone()).ok_or((
        StatusCode::BAD_REQUEST,
        "At least one name (English or Arabic) is required".to_string(),
    ))?;

    tracing::info!(">>> create_master: using name: {}", name);

    // Check for duplicates - normalize and search for existing masters
    let normalized_name = normalize_arabic_text(&name);

    // Check exact match on canonical_name
    if let Some(ref en_name) = req.name
        && let Ok(Some(existing)) = state.medication_master_repo.find_by_name(en_name).await
    {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "Master medication '{}' already exists with id: {}",
                en_name, existing.id
            ),
        ));
    }

    // Check semantic similarity - if very high match exists, reject as duplicate
    if let Ok(embedding) = state.ai_client.embed(&normalized_name).await
        && let Ok(similar) = state
            .medication_master_repo
            .search_semantic(&embedding, 1)
            .await
        && let Some((existing, score)) = similar.first()
        && *score > 0.95
    {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "Similar master medication '{}' already exists ({}% match). Use existing master id: {}",
                existing.canonical_name,
                (score * 100.0) as i32,
                existing.id
            ),
        ));
    }

    // Generate embedding for the new master
    let embedding = state.ai_client.embed(&name).await.map_err(|e| {
        tracing::error!(">>> create_master: AI embed error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("AI error: {}", e),
        )
    })?;

    tracing::info!(">>> create_master: embedding generated");

    let mut master = MedicationMasterModel::new(req.name.clone().unwrap_or_else(|| name.clone()));
    master.canonical_name_ar = req.name_ar.clone();
    master.active_ingredient = req.active_ingredient;
    master.strength = req.strength;
    master.manufacturer = req.manufacturer;
    master.embedding = Some(pgvector::Vector::from(embedding));

    let saved = state
        .medication_master_repo
        .save(&master)
        .await
        .map_err(|e| {
            tracing::error!(">>> create_master: save error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    tracing::info!(">>> create_master: master saved with id: {}", saved.id);

    // If alias_id is provided, link the alias to the new master
    if let Some(alias_id) = req.alias_id {
        tracing::info!(">>> create_master: linking alias_id: {}", alias_id);
        if let Some(mut alias) = state
            .medication_alias_repo
            .get_by_id(alias_id)
            .await
            .map_err(|e| {
                tracing::error!(">>> create_master: get alias error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?
        {
            alias.approve(saved.id, "system".to_string());
            state
                .medication_alias_repo
                .save(&alias)
                .await
                .map_err(|e| {
                    tracing::error!(">>> create_master: save alias error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                })?;
            tracing::info!(">>> create_master: alias linked successfully");
        } else {
            tracing::warn!(">>> create_master: alias not found: {}", alias_id);
        }
    } else if let Some(alias_name) = req.alias_name.as_ref() {
        // Create a new alias if alias_name is provided but no alias_id
        tracing::info!(
            ">>> create_master: creating new alias for name: '{}' (len={})",
            alias_name,
            alias_name.len()
        );
        use crate::repository::MedicationAliasModel;

        let mut new_alias = MedicationAliasModel::new(alias_name.clone());
        new_alias.approve(saved.id, "system".to_string());
        tracing::info!(
            ">>> create_master: alias model created with alias_name: '{}'",
            new_alias.alias_name
        );

        if let Err(e) = state.medication_alias_repo.save(&new_alias).await {
            tracing::warn!(">>> create_master: failed to create alias: {}", e);
            // Don't fail the whole operation if alias creation fails
        } else {
            tracing::info!(
                ">>> create_master: new alias created and linked with id: {}",
                new_alias.id
            );
        }
    } else {
        tracing::info!(">>> create_master: no alias_id or alias_name provided");
    }

    tracing::info!(">>> create_master: returning success");
    Ok(Json(CreateMasterResponse {
        success: true,
        master: MasterDto::from(&saved),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMasterRequest {
    pub name: Option<String>,
    pub name_ar: Option<String>,
    pub active_ingredient: Option<String>,
    pub strength: Option<String>,
    pub manufacturer: Option<String>,
}

/// Update a master medication record
/// PUT /api/curation/master/{id}
/// Regenerates embedding if canonical names change
pub async fn update_master<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(master_id): Path<Uuid>,
    Json(req): Json<UpdateMasterRequest>,
) -> Result<Json<CreateMasterResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    tracing::info!(">>> update_master called for id: {}", master_id);

    // Get existing master
    let mut master = state
        .medication_master_repo
        .get_by_id(master_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            "Master medication not found".to_string(),
        ))?;

    let old_name = master.canonical_name.clone();
    let old_name_ar = master.canonical_name_ar.clone();

    // Apply updates
    if let Some(name) = req.name {
        master.canonical_name = name;
    }
    if let Some(name_ar) = req.name_ar {
        master.canonical_name_ar = Some(name_ar);
    }
    if let Some(active_ingredient) = req.active_ingredient {
        master.active_ingredient = Some(active_ingredient);
    }
    if let Some(strength) = req.strength {
        master.strength = Some(strength);
    }
    if let Some(manufacturer) = req.manufacturer {
        master.manufacturer = Some(manufacturer);
    }

    // Regenerate embedding if canonical names changed
    let names_changed =
        master.canonical_name != old_name || master.canonical_name_ar != old_name_ar;

    if names_changed {
        tracing::info!(
            ">>> update_master: names changed, regenerating embedding (old: '{}' -> new: '{}')",
            old_name,
            master.canonical_name
        );

        // Create embedding text from both names
        let embed_text = if let Some(ref ar_name) = master.canonical_name_ar {
            format!("{} {}", master.canonical_name, ar_name)
        } else {
            master.canonical_name.clone()
        };

        let embedding = state.ai_client.embed(&embed_text).await.map_err(|e| {
            tracing::error!(">>> update_master: AI embed error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("AI error: {}", e),
            )
        })?;

        master.embedding = Some(pgvector::Vector::from(embedding));
        tracing::info!(">>> update_master: new embedding generated");
    }

    // Save updated master
    let saved = state
        .medication_master_repo
        .save(&master)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(">>> update_master: saved successfully");

    Ok(Json(CreateMasterResponse {
        success: true,
        master: MasterDto::from(&saved),
    }))
}

/// Approve an alias and map it to a master medication (path-based)
pub async fn approve_alias<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(alias_id): Path<Uuid>,
    Json(req): Json<ApproveAliasPathRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    if let Some(mut alias) = state
        .medication_alias_repo
        .get_by_id(alias_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        alias.approve(req.master_id, "system".to_string());
        state
            .medication_alias_repo
            .save(&alias)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        return Err((StatusCode::NOT_FOUND, "Alias not found".to_string()));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkAliasRequest {
    pub master_id: Uuid,
    pub alias_name: String,
}

/// Create a new alias and link it to an existing master medication
/// POST /api/curation/link
pub async fn link_alias_to_master<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<LinkAliasRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    use crate::repository::MedicationAliasModel;

    // Check if master exists
    let master = state
        .medication_master_repo
        .get_by_id(req.master_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            "Master medication not found".to_string(),
        ))?;

    tracing::info!(
        ">>> link_alias_to_master: linking '{}' to master '{}'",
        req.alias_name,
        master.canonical_name
    );

    // Check if alias already exists for this name
    if let Some(mut existing_alias) = state
        .medication_alias_repo
        .get_by_name(&req.alias_name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        // Update existing alias
        existing_alias.approve(req.master_id, "system".to_string());
        state
            .medication_alias_repo
            .save(&existing_alias)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        tracing::info!(
            ">>> link_alias_to_master: updated existing alias {}",
            existing_alias.id
        );
    } else {
        // Create new alias
        let mut new_alias = MedicationAliasModel::new(req.alias_name.clone());
        new_alias.approve(req.master_id, "system".to_string());
        state
            .medication_alias_repo
            .save(&new_alias)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        tracing::info!(
            ">>> link_alias_to_master: created new alias {}",
            new_alias.id
        );
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkApproveRequest {
    pub alias_ids: Vec<Uuid>,
    pub master_id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkApproveResponse {
    pub success: bool,
    pub approved_count: usize,
    pub failed_count: usize,
}

/// Bulk approve multiple aliases by linking them to a master medication
/// POST /api/curation/aliases/bulk-approve
pub async fn bulk_approve_aliases<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<BulkApproveRequest>,
) -> Result<Json<BulkApproveResponse>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    tracing::info!(
        ">>> bulk_approve_aliases: approving {} aliases to master {}",
        req.alias_ids.len(),
        req.master_id
    );

    // Verify master exists
    state
        .medication_master_repo
        .get_by_id(req.master_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            "Master medication not found".to_string(),
        ))?;

    let mut approved_count = 0;
    let mut failed_count = 0;

    for alias_id in req.alias_ids {
        match state.medication_alias_repo.get_by_id(alias_id).await {
            Ok(Some(mut alias)) => {
                alias.approve(req.master_id, "system:bulk".to_string());
                match state.medication_alias_repo.save(&alias).await {
                    Ok(_) => {
                        approved_count += 1;
                        tracing::info!(">>> bulk_approve: approved alias {}", alias_id);
                    }
                    Err(e) => {
                        failed_count += 1;
                        tracing::warn!(
                            ">>> bulk_approve: failed to save alias {}: {}",
                            alias_id,
                            e
                        );
                    }
                }
            }
            Ok(None) => {
                failed_count += 1;
                tracing::warn!(">>> bulk_approve: alias {} not found", alias_id);
            }
            Err(e) => {
                failed_count += 1;
                tracing::warn!(">>> bulk_approve: failed to get alias {}: {}", alias_id, e);
            }
        }
    }

    Ok(Json(BulkApproveResponse {
        success: failed_count == 0,
        approved_count,
        failed_count,
    }))
}

/// Get AI-driven suggestions for a medication name
pub async fn get_suggestions<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Query(query): Query<SuggestionQuery>,
) -> Result<Json<Vec<MasterSuggestion>>, (StatusCode, String)>
where
    RQ: crate::repository::ReviewQueueRepository + 'static,
    A: crate::repository::AuditLogRepository + 'static,
    MM: crate::repository::MedicationMasterRepository + 'static,
{
    let limit = query.limit.unwrap_or(5);
    let mut suggestions = Vec::new();

    // Normalize the query name (convert Arabic-Indic numerals to Western)
    let normalized_name = normalize_arabic_text(&query.name);
    tracing::info!(
        ">>> get_suggestions: query='{}' normalized='{}'",
        query.name,
        normalized_name
    );

    // 1. Semantic Search (AI)
    match state.ai_client.embed(&normalized_name).await {
        Ok(embedding) => {
            tracing::info!(">>> get_suggestions: embedding generated, searching...");
            match state
                .medication_master_repo
                .search_semantic(&embedding, limit)
                .await
            {
                Ok(semantic_matches) => {
                    tracing::info!(
                        ">>> get_suggestions: semantic search found {} matches",
                        semantic_matches.len()
                    );
                    for (model, score) in semantic_matches {
                        tracing::info!(
                            ">>> get_suggestions: semantic match: '{}' score={}",
                            model.canonical_name,
                            score
                        );
                        suggestions.push(MasterSuggestion {
                            master: MasterDto::from(&model),
                            score,
                            method: "semantic".to_string(),
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(">>> get_suggestions: semantic search failed: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::warn!(">>> get_suggestions: embedding failed: {}", e);
        }
    }

    // 2. Trigram Similarity Search (better fuzzy matching)
    match state
        .medication_master_repo
        .search_fuzzy(&normalized_name, limit, 0.3) // 30% minimum similarity
        .await
    {
        Ok(trigram_matches) => {
            tracing::info!(
                ">>> get_suggestions: trigram search found {} matches",
                trigram_matches.len()
            );
            for (model, score) in trigram_matches {
                // Avoid duplicates from semantic search
                if suggestions.iter().any(|s| s.master.id == model.id) {
                    continue;
                }
                tracing::info!(
                    ">>> get_suggestions: trigram match: '{}' score={}",
                    model.canonical_name,
                    score
                );
                suggestions.push(MasterSuggestion {
                    master: MasterDto::from(&model),
                    score,
                    method: "trigram".to_string(),
                });
            }
        }
        Err(e) => {
            tracing::warn!(">>> get_suggestions: trigram search failed: {}", e);
        }
    }

    // 3. LIKE Search (Substring fallback)
    match state
        .medication_master_repo
        .search(&normalized_name, limit)
        .await
    {
        Ok(fuzzy_matches) => {
            tracing::info!(
                ">>> get_suggestions: LIKE search found {} matches",
                fuzzy_matches.len()
            );
            for model in fuzzy_matches {
                // Avoid duplicates from semantic/trigram search
                if suggestions.iter().any(|s| s.master.id == model.id) {
                    continue;
                }
                suggestions.push(MasterSuggestion {
                    master: MasterDto::from(&model),
                    score: 0.6, // Lower confidence for LIKE matches
                    method: "substring".to_string(),
                });
            }
        }
        Err(e) => {
            tracing::warn!(">>> get_suggestions: LIKE search failed: {}", e);
        }
    }

    // Sort by score
    suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    suggestions.truncate(limit as usize);

    tracing::info!(
        ">>> get_suggestions: returning {} suggestions",
        suggestions.len()
    );
    Ok(Json(suggestions))
}
