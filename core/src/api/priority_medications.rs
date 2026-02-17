//! Priority Medications API Handlers
//!
//! REST endpoints for priority medication management (CRUD operations)

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};
use pharma_db::entity::priority_medication::{Model as PriorityMedication, PriorityLevel};

/// Request body for creating a priority medication
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePriorityRequest {
    pub medication_name: String,
    #[serde(default)]
    pub medication_name_ar: Option<String>,
    pub priority_level: PriorityLevel,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub active_from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub active_until: Option<DateTime<Utc>>,
}

/// Request body for updating a priority medication
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePriorityRequest {
    #[serde(default)]
    pub medication_name: Option<String>,
    #[serde(default)]
    pub medication_name_ar: Option<String>,
    #[serde(default)]
    pub priority_level: Option<PriorityLevel>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub active_from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub active_until: Option<DateTime<Utc>>,
}

/// Response for priority medication operations
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityResponse {
    pub success: bool,
    pub priority: Option<PriorityMedication>,
    pub error: Option<String>,
}

/// Response for list operations
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityListResponse {
    pub success: bool,
    pub priorities: Vec<PriorityMedication>,
    pub total: usize,
}

/// Response for priority check
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityCheckResponse {
    pub is_priority: bool,
    pub priority_score: i32,
    pub medication: Option<PriorityMedication>,
}

/// GET /api/priority-medications - List all priority medications
pub async fn list_priorities<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<PriorityListResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let priorities = state
        .priority_medication_repo
        .get_all(1000, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = priorities.len();

    Ok(Json(PriorityListResponse {
        success: true,
        priorities,
        total,
    }))
}

/// GET /api/priority-medications/active - List active priority medications
pub async fn list_active_priorities<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
) -> Result<Json<PriorityListResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let priorities = state
        .priority_medication_repo
        .get_all_active()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = priorities.len();

    Ok(Json(PriorityListResponse {
        success: true,
        priorities,
        total,
    }))
}

/// GET /api/priority-medications/:id - Get a specific priority medication
pub async fn get_priority<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<PriorityResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let priority = state
        .priority_medication_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match priority {
        Some(p) => Ok(Json(PriorityResponse {
            success: true,
            priority: Some(p),
            error: None,
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            "Priority medication not found".to_string(),
        )),
    }
}

/// GET /api/priority-medications/check/:medication - Check if medication is priority
pub async fn check_priority<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(medication): Path<String>,
) -> Result<Json<PriorityCheckResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let priority_score = state
        .priority_medication_repo
        .get_priority_for_medication(&medication)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(0);

    let medication_data = if priority_score > 0 {
        state
            .priority_medication_repo
            .get_by_medication_name(&medication)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        None
    };

    Ok(Json(PriorityCheckResponse {
        is_priority: priority_score > 0,
        priority_score,
        medication: medication_data,
    }))
}

/// POST /api/priority-medications - Create a new priority medication
pub async fn create_priority<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Json(req): Json<CreatePriorityRequest>,
) -> Result<(StatusCode, Json<PriorityResponse>), (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Check if priority medication already exists
    if let Ok(Some(_)) = state
        .priority_medication_repo
        .get_by_medication_name(&req.medication_name)
        .await
    {
        return Err((
            StatusCode::CONFLICT,
            "Priority medication already exists".to_string(),
        ));
    }

    let now = Utc::now();
    let priority = PriorityMedication {
        id: Uuid::new_v4(),
        medication_name: req.medication_name,
        medication_name_ar: req.medication_name_ar,
        priority_level: req.priority_level,
        reason: req.reason,
        active: req.active,
        active_from: req.active_from.unwrap_or(now),
        active_until: req.active_until,
        created_by: None, // TODO: Add user authentication
        created_at: now,
        updated_at: now,
    };

    state
        .priority_medication_repo
        .save(&priority)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clear priority detector cache if available
    if let Some(detector) = &state.priority_detector {
        detector.clear_cache().await;
    }

    Ok((
        StatusCode::CREATED,
        Json(PriorityResponse {
            success: true,
            priority: Some(priority),
            error: None,
        }),
    ))
}

/// PUT /api/priority-medications/:id - Update a priority medication
pub async fn update_priority<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePriorityRequest>,
) -> Result<Json<PriorityResponse>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let existing = state
        .priority_medication_repo
        .get_by_id(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            StatusCode::NOT_FOUND,
            "Priority medication not found".to_string(),
        ))?;

    let mut updated = existing;

    if let Some(name) = req.medication_name {
        updated.medication_name = name;
    }
    if let Some(name_ar) = req.medication_name_ar {
        updated.medication_name_ar = Some(name_ar);
    }
    if let Some(level) = req.priority_level {
        updated.priority_level = level;
    }
    if let Some(reason) = req.reason {
        updated.reason = Some(reason);
    }
    if let Some(active) = req.active {
        updated.active = active;
    }
    if let Some(from) = req.active_from {
        updated.active_from = from;
    }
    if let Some(until) = req.active_until {
        updated.active_until = Some(until);
    }

    updated.updated_at = Utc::now();

    state
        .priority_medication_repo
        .save(&updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clear priority detector cache if available
    if let Some(detector) = &state.priority_detector {
        detector.clear_cache().await;
    }

    Ok(Json(PriorityResponse {
        success: true,
        priority: Some(updated),
        error: None,
    }))
}

/// DELETE /api/priority-medications/:id - Delete a priority medication
pub async fn delete_priority<RQ, A, MM>(
    State(state): State<AppState<RQ, A, MM>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    let deleted = state
        .priority_medication_repo
        .delete(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        // Clear priority detector cache if available
        if let Some(detector) = &state.priority_detector {
            detector.clear_cache().await;
        }

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Priority medication deleted"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "Priority medication not found".to_string(),
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_priority_request_deserialize() {
        let json = r#"{
            "medicationName": "Insulin",
            "medicationNameAr": "انسولين",
            "priorityLevel": "CRITICAL",
            "reason": "Life-saving medication",
            "active": true
        }"#;
        let req: CreatePriorityRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.medication_name, "Insulin");
        assert_eq!(req.medication_name_ar, Some("انسولين".to_string()));
        assert_eq!(req.priority_level, PriorityLevel::Critical);
        assert!(req.active);
    }

    #[test]
    fn test_update_priority_request_optional_fields() {
        let json = r#"{"active": false, "priorityLevel": "HIGH"}"#;
        let req: UpdatePriorityRequest = serde_json::from_str(json).unwrap();
        assert!(req.medication_name.is_none());
        assert_eq!(req.active, Some(false));
        assert_eq!(req.priority_level, Some(PriorityLevel::High));
    }

    #[test]
    fn test_priority_check_response_serialize() {
        let response = PriorityCheckResponse {
            is_priority: true,
            priority_score: 10,
            medication: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("isPriority"));
        assert!(json.contains("priorityScore"));
    }
}
