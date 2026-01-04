//! Match Audit Record entity
//!
//! Stores complete snapshots of all inputs and parameters for debugging
//! and reproducibility. Designed to integrate with frontend debug recordings.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Match audit record entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "match_audit_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Match identification
    pub match_id: Uuid,
    pub offer_id: Uuid,
    pub request_id: Uuid,

    /// Pipeline version for reproducibility
    pub pipeline_version: String,

    /// Complete snapshots (JSONB)
    pub offer_snapshot: Json,
    pub request_snapshot: Json,
    pub weights_snapshot: Json,
    pub config_snapshot: Option<Json>,

    /// Score breakdown
    pub score_breakdown: Json,
    pub final_score: f64,

    /// Pipeline execution trace
    pub pipeline_stages: Json,

    /// AI involvement
    pub ai_involved: bool,
    pub ai_model: Option<String>,
    pub ai_response: Option<Json>,
    pub ai_latency_ms: Option<i32>,

    /// Resolution path
    pub resolution_stage: String,
    pub resolution_details: Option<Json>,

    /// Timing
    pub total_latency_ms: i32,
    pub created_at: DateTimeUtc,

    /// Review outcome
    pub review_status: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTimeUtc>,
    pub review_notes: Option<String>,

    /// Session tracking for frontend debug recordings
    pub session_id: Option<String>,
    pub client_metadata: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
