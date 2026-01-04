//! Database Diagnostics API
//!
//! HTTP endpoints for database health monitoring and performance analysis.

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use super::routes::AppState;
use crate::repository::{AuditLogRepository, MedicationMasterRepository, ReviewQueueRepository};

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub connection_count: i64,
    pub max_connections: i64,
    pub database_size: String,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Serialize)]
pub struct TableStatsResponse {
    pub tables: Vec<TableStat>,
}

#[derive(Debug, Serialize)]
pub struct TableStat {
    pub table_name: String,
    pub row_count: i64,
    pub dead_tuples: i64,
    pub table_size: String,
    pub total_size: String,
    pub needs_vacuum: bool,
}

#[derive(Debug, Serialize)]
pub struct IndexStatsResponse {
    pub indexes: Vec<IndexStat>,
    pub unused_indexes: Vec<IndexStat>,
}

#[derive(Debug, Serialize)]
pub struct IndexStat {
    pub index_name: String,
    pub table_name: String,
    pub index_size: String,
    pub index_scans: i64,
}

#[derive(Debug, Serialize)]
pub struct QueryAnalysisResponse {
    pub queries: Vec<QueryAnalysis>,
}

#[derive(Debug, Serialize)]
pub struct QueryAnalysis {
    pub name: String,
    pub uses_index: bool,
    pub uses_seq_scan: bool,
    pub execution_time_ms: Option<f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeQueryRequest {
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct AnalyzeQueryResponse {
    pub plan: String,
    pub uses_index: bool,
    pub uses_seq_scan: bool,
    pub execution_time_ms: Option<f64>,
    pub warnings: Vec<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/diagnostics/health
/// Returns database health overview
pub async fn get_health<RQ, A, MM>(
    State(_state): State<AppState<RQ, A, MM>>,
) -> Json<serde_json::Value>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    // Note: This endpoint requires direct database access
    // In a real implementation, you'd inject the DatabaseConnection
    // For now, return a placeholder response
    Json(serde_json::json!({
        "status": "ok",
        "message": "Use the db-health CLI tool for full diagnostics",
        "cli_command": "cargo run --bin db_health"
    }))
}

/// GET /api/diagnostics/tables
/// Returns table statistics
pub async fn get_table_stats<RQ, A, MM>(
    State(_state): State<AppState<RQ, A, MM>>,
) -> Json<serde_json::Value>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    Json(serde_json::json!({
        "status": "ok",
        "message": "Use the db-health CLI tool for table statistics",
        "cli_command": "cargo run --bin db_health -- --tables"
    }))
}

/// GET /api/diagnostics/indexes
/// Returns index statistics
pub async fn get_index_stats<RQ, A, MM>(
    State(_state): State<AppState<RQ, A, MM>>,
) -> Json<serde_json::Value>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    Json(serde_json::json!({
        "status": "ok",
        "message": "Use the db-health CLI tool for index statistics",
        "cli_command": "cargo run --bin db_health -- --indexes"
    }))
}

/// GET /api/diagnostics/queries
/// Analyzes critical queries
pub async fn analyze_queries<RQ, A, MM>(
    State(_state): State<AppState<RQ, A, MM>>,
) -> Json<serde_json::Value>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMasterRepository + 'static,
{
    Json(serde_json::json!({
        "status": "ok",
        "message": "Use the db-health CLI tool for query analysis",
        "cli_command": "cargo run --bin db_health -- --analyze"
    }))
}
