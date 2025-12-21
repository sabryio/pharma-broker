//! API Routes
//!
//! Defines the axum router with all REST endpoints

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use axum::{
    Router,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::sync::broadcast;

use super::{calibration, confidence, groups, handlers, review_queue, weights};
use crate::matching::MatchingEngine;
use crate::repository::{
    AuditLogRepository, FeedbackRecordRepository, GroupRepository, MatchRepository,
    MedicationMappingRepository, OfferRepository, RequestRepository, ReviewQueueRepository,
};
use crate::ws::{self, WsEvent};

/// Application state shared across handlers
pub struct AppState<RQ, A, MM>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    pub offer_repo: Arc<dyn OfferRepository + Send + Sync>,
    pub request_repo: Arc<dyn RequestRepository + Send + Sync>,
    pub match_repo: Arc<dyn MatchRepository + Send + Sync>,
    pub group_repo: Arc<dyn GroupRepository + Send + Sync>,
    pub feedback_repo: Arc<dyn FeedbackRecordRepository + Send + Sync>,
    pub review_queue_repo: Arc<RQ>,
    pub audit_log_repo: Arc<A>,
    pub medication_mapping_repo: Arc<MM>,
    pub matching_engine: Option<Arc<MatchingEngine>>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub metrics_handle: Option<PrometheusHandle>,
    pub active_connections: Arc<AtomicUsize>,
}

impl<RQ, A, MM> Clone for AppState<RQ, A, MM>
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    fn clone(&self) -> Self {
        Self {
            offer_repo: self.offer_repo.clone(),
            request_repo: self.request_repo.clone(),
            match_repo: self.match_repo.clone(),
            group_repo: self.group_repo.clone(),
            feedback_repo: self.feedback_repo.clone(),
            review_queue_repo: self.review_queue_repo.clone(),
            audit_log_repo: self.audit_log_repo.clone(),
            medication_mapping_repo: self.medication_mapping_repo.clone(),
            matching_engine: self.matching_engine.clone(),
            ws_tx: self.ws_tx.clone(),
            metrics_handle: self.metrics_handle.clone(),
            active_connections: self.active_connections.clone(),
        }
    }
}

/// Create the main API router
pub fn create_router<RQ, A, MM>(state: AppState<RQ, A, MM>) -> Router
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    Router::new()
        // Health checks
        .route("/health", get(handlers::health_check))
        .route("/health/ready", get(handlers::health_ready::<RQ, A, MM>))
        .route("/health/live", get(handlers::health_live))
        // Prometheus metrics
        .route("/metrics", get(metrics_handler::<RQ, A, MM>))
        // Offers
        .route("/api/offers", get(handlers::get_offers::<RQ, A, MM>))
        // Requests
        .route("/api/requests", get(handlers::get_requests::<RQ, A, MM>))
        // Matches
        .route("/api/matches", get(handlers::get_matches::<RQ, A, MM>))
        .route(
            "/api/matches/{id}/confirm",
            post(handlers::confirm_match::<RQ, A, MM>),
        )
        .route(
            "/api/matches/{id}/reject",
            post(handlers::reject_match::<RQ, A, MM>),
        )
        // Stats
        .route("/api/stats", get(handlers::get_stats::<RQ, A, MM>))
        // Groups
        .route("/api/groups", get(groups::list_groups::<RQ, A, MM>))
        .route("/api/groups", post(groups::create_group::<RQ, A, MM>))
        .route("/api/groups/{jid}", get(groups::get_group::<RQ, A, MM>))
        .route("/api/groups/{jid}", put(groups::update_group::<RQ, A, MM>))
        .route(
            "/api/groups/{jid}",
            delete(groups::delete_group::<RQ, A, MM>),
        )
        // Weights management
        .route("/api/weights", get(weights::get_weights::<RQ, A, MM>))
        .route("/api/weights", put(weights::update_weights::<RQ, A, MM>))
        .route(
            "/api/weights/scheduler",
            get(weights::get_scheduler_status::<RQ, A, MM>),
        )
        .route(
            "/api/weights/influence",
            get(weights::get_influence::<RQ, A, MM>),
        )
        // Review Queue
        .route(
            "/api/review-queue",
            get(review_queue::list_review_items::<RQ, A, MM>),
        )
        .route(
            "/api/review-queue/stats",
            get(review_queue::get_review_stats::<RQ, A, MM>),
        )
        .route(
            "/api/review-queue/{id}",
            get(review_queue::get_review_item::<RQ, A, MM>),
        )
        .route(
            "/api/review-queue/{id}/status",
            put(review_queue::update_review_status::<RQ, A, MM>),
        )
        // Confidence Management
        .route(
            "/api/confidence",
            get(confidence::get_confidence::<RQ, A, MM>),
        )
        .route(
            "/api/confidence",
            put(confidence::update_confidence::<RQ, A, MM>),
        )
        .route(
            "/api/confidence/thresholds",
            put(confidence::update_thresholds::<RQ, A, MM>),
        )
        .route(
            "/api/confidence/reset",
            post(confidence::reset_confidence::<RQ, A, MM>),
        )
        .route(
            "/api/confidence/adaptive",
            post(confidence::toggle_adaptive::<RQ, A, MM>),
        )
        .route(
            "/api/confidence/stats/reset",
            post(confidence::reset_stats::<RQ, A, MM>),
        )
        // Calibration Management
        .route(
            "/api/calibration",
            get(calibration::get_calibration::<RQ, A, MM>),
        )
        .route(
            "/api/calibration",
            put(calibration::update_calibration::<RQ, A, MM>),
        )
        .route(
            "/api/calibration/reset",
            post(calibration::reset_calibration::<RQ, A, MM>),
        )
        .route(
            "/api/calibration/enable",
            post(calibration::toggle_calibration::<RQ, A, MM>),
        )
        .route(
            "/api/calibration/smoothing",
            put(calibration::update_smoothing::<RQ, A, MM>),
        )
        .route(
            "/api/calibration/calibrate",
            post(calibration::calibrate_score::<RQ, A, MM>),
        )
        // WebSocket
        .route("/ws", get(ws::ws_handler::<RQ, A, MM>))
        .with_state(state)
}

/// Handler for /metrics endpoint
async fn metrics_handler<RQ, A, MM>(
    axum::extract::State(state): axum::extract::State<AppState<RQ, A, MM>>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    if let Some(handle) = &state.metrics_handle {
        handle.render()
    } else {
        "# Metrics not initialized\n".to_string()
    }
}
