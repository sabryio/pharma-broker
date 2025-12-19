//! API Routes
//!
//! Defines the axum router with all REST endpoints

use axum::{
    Router,
    response::IntoResponse,
    routing::{get, post},
};
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::Arc;

use super::handlers;
use crate::matching::Scorer;
use crate::repository::{MatchRepository, OfferRepository, RequestRepository};

/// Application state shared across handlers
pub struct AppState<O, R, M>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    pub match_repo: Arc<M>,
    pub scorer: Arc<Scorer>,
    pub metrics_handle: Option<PrometheusHandle>,
}

impl<O, R, M> Clone for AppState<O, R, M>
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    fn clone(&self) -> Self {
        Self {
            offer_repo: self.offer_repo.clone(),
            request_repo: self.request_repo.clone(),
            match_repo: self.match_repo.clone(),
            scorer: self.scorer.clone(),
            metrics_handle: self.metrics_handle.clone(),
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
/// - GET  /health             - Health check
/// - GET  /metrics            - Prometheus metrics
pub fn create_router<O, R, M>(state: AppState<O, R, M>) -> Router
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: MatchRepository + 'static,
{
    Router::new()
        // Health checks (Kubernetes probes)
        .route("/health", get(handlers::health_check))
        .route("/health/ready", get(handlers::health_ready::<O, R, M>))
        .route("/health/live", get(handlers::health_live))
        // Prometheus metrics
        .route("/metrics", get(metrics_handler::<O, R, M>))
        // Offers
        .route("/api/offers", get(handlers::get_offers::<O, R, M>))
        // Requests
        .route("/api/requests", get(handlers::get_requests::<O, R, M>))
        // Matches
        .route("/api/matches", get(handlers::get_matches::<O, R, M>))
        .route(
            "/api/matches/{id}/confirm",
            post(handlers::confirm_match::<O, R, M>),
        )
        .route(
            "/api/matches/{id}/reject",
            post(handlers::reject_match::<O, R, M>),
        )
        // Stats
        .route("/api/stats", get(handlers::get_stats::<O, R, M>))
        .with_state(state)
}

/// Handler for /metrics endpoint - returns Prometheus format
async fn metrics_handler<O, R, M>(
    axum::extract::State(state): axum::extract::State<AppState<O, R, M>>,
) -> impl IntoResponse
where
    O: OfferRepository,
    R: RequestRepository,
    M: MatchRepository,
{
    if let Some(handle) = &state.metrics_handle {
        handle.render()
    } else {
        "# Metrics not initialized\n".to_string()
    }
}
