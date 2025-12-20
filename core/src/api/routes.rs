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

use super::{groups, handlers, review_queue, weights};
use crate::matching::MatchingEngineHandle;
use crate::repository::{
    AuditLogRepository, FeedbackRecordRepository, GroupRepository, MatchRepository,
    OfferRepository, RequestRepository, ReviewQueueRepository,
};
use crate::ws::{self, WsEvent};

/// Application state shared across handlers
pub struct AppState<O, R, M, G, F, RQ, A>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    pub match_repo: Arc<M>,
    pub group_repo: Arc<G>,
    pub feedback_repo: Arc<F>,
    pub review_queue_repo: Arc<RQ>,
    pub audit_log_repo: Arc<A>,
    pub matching_engine: Option<MatchingEngineHandle>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub metrics_handle: Option<PrometheusHandle>,
    pub active_connections: Arc<AtomicUsize>,
}

impl<O, R, M, G, F, RQ, A> Clone for AppState<O, R, M, G, F, RQ, A>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
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
            matching_engine: self.matching_engine.clone(),
            ws_tx: self.ws_tx.clone(),
            metrics_handle: self.metrics_handle.clone(),
            active_connections: self.active_connections.clone(),
        }
    }
}

/// Create the main API router
///
/// Endpoints ported from legacy/api/routes.go:
/// - GET  /api/offers         - List active offers
/// - GET  /api/requests       - List active requests
/// - GET  /api/matches        - List pending matches
/// - POST /api/matches/:id/confirm - Confirm a match
/// - POST /api/matches/:id/reject  - Reject a match
/// - GET  /api/stats          - Get dashboard stats
/// - GET  /api/groups         - List all groups
/// - POST /api/groups         - Create a group
/// - PUT  /api/groups/:jid    - Update a group
/// - DELETE /api/groups/:jid  - Delete a group
/// - GET  /health             - Health check
/// - GET  /metrics            - Prometheus metrics
/// - GET  /ws                 - WebSocket real-time updates
pub fn create_router<O, R, M, G, F, RQ, A>(state: AppState<O, R, M, G, F, RQ, A>) -> Router
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: MatchRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRecordRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
{
    Router::new()
        // Health checks (Kubernetes probes)
        .route("/health", get(handlers::health_check))
        .route(
            "/health/ready",
            get(handlers::health_ready::<O, R, M, G, F, RQ, A>),
        )
        .route("/health/live", get(handlers::health_live))
        // Prometheus metrics
        .route("/metrics", get(metrics_handler::<O, R, M, G, F, RQ, A>))
        // Offers
        .route(
            "/api/offers",
            get(handlers::get_offers::<O, R, M, G, F, RQ, A>),
        )
        // Requests
        .route(
            "/api/requests",
            get(handlers::get_requests::<O, R, M, G, F, RQ, A>),
        )
        // Matches
        .route(
            "/api/matches",
            get(handlers::get_matches::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/matches/{id}/confirm",
            post(handlers::confirm_match::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/matches/{id}/reject",
            post(handlers::reject_match::<O, R, M, G, F, RQ, A>),
        )
        // Stats
        .route(
            "/api/stats",
            get(handlers::get_stats::<O, R, M, G, F, RQ, A>),
        )
        // Groups (new CRUD endpoints)
        .route(
            "/api/groups",
            get(groups::get_groups::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/groups",
            post(groups::create_group::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/groups/{jid}",
            get(groups::get_group::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/groups/{jid}",
            put(groups::update_group::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/groups/{jid}",
            delete(groups::delete_group::<O, R, M, G, F, RQ, A>),
        )
        // Weights management
        .route(
            "/api/weights",
            get(weights::get_weights::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights",
            put(weights::update_weights::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/scheduler",
            get(weights::get_scheduler_status::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/influence",
            get(weights::get_influence::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/abtest",
            get(weights::list_ab_tests::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/abtest",
            post(weights::create_ab_test::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/abtest/{id}",
            get(weights::get_ab_test_result::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/weights/abtest/{id}",
            delete(weights::end_ab_test::<O, R, M, G, F, RQ, A>),
        )
        // Review Queue
        .route(
            "/api/review-queue",
            get(review_queue::get_review_queue::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/review-queue/stats",
            get(review_queue::get_review_stats::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/review-queue/{id}",
            get(review_queue::get_review_item::<O, R, M, G, F, RQ, A>),
        )
        .route(
            "/api/review-queue/{id}/review",
            post(review_queue::update_review_status::<O, R, M, G, F, RQ, A>),
        )
        // WebSocket
        .route("/ws", get(ws::ws_handler::<O, R, M, G, F, RQ, A>))
        .with_state(state)
}

/// Handler for /metrics endpoint - returns Prometheus format
async fn metrics_handler<O, R, M, G, F, RQ, A>(
    axum::extract::State(state): axum::extract::State<AppState<O, R, M, G, F, RQ, A>>,
) -> impl IntoResponse
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
    G: GroupRepository,
    F: FeedbackRecordRepository,
    RQ: ReviewQueueRepository,
    A: AuditLogRepository,
{
    if let Some(handle) = &state.metrics_handle {
        handle.render()
    } else {
        "# Metrics not initialized\n".to_string()
    }
}
