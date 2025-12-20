//! Groups API Handlers
//!
//! REST endpoints for group management (CRUD operations)

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::api::routes::AppState;
use crate::domain::Group;
use crate::repository::{
    AuditLogRepository, FeedbackRecordRepository, GroupRepository, MatchRepository,
    OfferRepository, RequestRepository, ReviewQueueRepository,
};

/// Request body for creating/updating a group
#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub jid: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub monitored: bool,
}

/// Request body for updating group monitoring status
#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub monitored: Option<bool>,
}

/// Response for group operations
#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub success: bool,
    pub group: Option<Group>,
    pub error: Option<String>,
}

/// Response for list operations
#[derive(Debug, Serialize)]
pub struct GroupListResponse {
    pub success: bool,
    pub groups: Vec<Group>,
    pub total: usize,
}

/// GET /api/groups - List all groups
pub async fn get_groups<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
) -> Result<Json<GroupListResponse>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let groups = state
        .group_repo
        .get_all()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total = groups.len();

    Ok(Json(GroupListResponse {
        success: true,
        groups,
        total,
    }))
}

/// GET /api/groups/:jid - Get a specific group
pub async fn get_group<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Path(jid): Path<String>,
) -> Result<Json<GroupResponse>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let group = state
        .group_repo
        .get_by_jid(&jid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match group {
        Some(g) => Ok(Json(GroupResponse {
            success: true,
            group: Some(g),
            error: None,
        })),
        None => Err((StatusCode::NOT_FOUND, "Group not found".to_string())),
    }
}

/// POST /api/groups - Create a new group
pub async fn create_group<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Json(req): Json<CreateGroupRequest>,
) -> Result<(StatusCode, Json<GroupResponse>), (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    // Check if group already exists
    if let Ok(Some(_)) = state.group_repo.get_by_jid(&req.jid).await {
        return Err((StatusCode::CONFLICT, "Group already exists".to_string()));
    }

    let mut group = Group::new(req.jid, req.name);
    group.description = req.description;
    group.monitored = req.monitored;

    state
        .group_repo
        .save(&group)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(GroupResponse {
            success: true,
            group: Some(group),
            error: None,
        }),
    ))
}

/// PUT /api/groups/:jid - Update a group
pub async fn update_group<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Path(jid): Path<String>,
    Json(req): Json<UpdateGroupRequest>,
) -> Result<Json<GroupResponse>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let existing = state
        .group_repo
        .get_by_jid(&jid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Group not found".to_string()))?;

    let mut updated = existing;

    if let Some(name) = req.name {
        updated.name = name;
    }
    if let Some(desc) = req.description {
        updated.description = Some(desc);
    }
    if let Some(monitored) = req.monitored {
        updated.monitored = monitored;
    }

    state
        .group_repo
        .save(&updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(GroupResponse {
        success: true,
        group: Some(updated),
        error: None,
    }))
}

/// DELETE /api/groups/:jid - Delete a group
pub async fn delete_group<O, R, M, G, F, RQ, A>(
    State(state): State<AppState<O, R, M, G, F, RQ, A>>,
    Path(jid): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    let deleted = state
        .group_repo
        .delete(&jid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if deleted {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Group deleted"
        })))
    } else {
        Err((StatusCode::NOT_FOUND, "Group not found".to_string()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_group_request_deserialize() {
        let json = r#"{"jid": "123@g.us", "name": "Test Group", "monitored": true}"#;
        let req: CreateGroupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jid, "123@g.us");
        assert_eq!(req.name, "Test Group");
        assert!(req.monitored);
    }

    #[test]
    fn test_update_group_request_optional_fields() {
        let json = r#"{"monitored": false}"#;
        let req: UpdateGroupRequest = serde_json::from_str(json).unwrap();
        assert!(req.name.is_none());
        assert!(req.description.is_none());
        assert_eq!(req.monitored, Some(false));
    }

    #[test]
    fn test_group_list_response_serialize() {
        let response = GroupListResponse {
            success: true,
            groups: vec![],
            total: 0,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("groups"));
    }
}
