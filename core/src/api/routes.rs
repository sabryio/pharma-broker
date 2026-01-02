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
use tower_http::cors::{Any, CorsLayer};

use super::{
    audit_records, audit_trail, calibration, confidence, curation, diagnostics, embedding_cache,
    groups, handlers, match_filter, match_reviews, matching, participants, reclassify, reparse,
    review_queue, uncertainty, weights,
};
use crate::ai::PharmaParser;
use crate::matching::{AliasLearner, MatchingEngine};
use crate::repository::{
    AuditLogRepository, FeedbackRepository, GroupRepository, MatchQueueRepository, MatchRepository,
    MedicationAliasRepository, MedicationMappingRepository, MedicationMasterRepository,
    OfferRepository, ParticipantRepository, RawMessageRepository, RequestRepository,
    ReviewQueueRepository,
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
    pub feedback_repo: Arc<dyn FeedbackRepository + Send + Sync>,
    pub participant_repo: Arc<dyn ParticipantRepository + Send + Sync>,
    pub raw_message_repo: Arc<dyn RawMessageRepository + Send + Sync>,
    pub review_queue_repo: Arc<RQ>,
    pub audit_log_repo: Arc<A>,
    pub medication_mapping_repo: Arc<MM>,
    pub medication_master_repo: Arc<dyn MedicationMasterRepository + Send + Sync>,
    pub medication_alias_repo: Arc<dyn MedicationAliasRepository + Send + Sync>,
    pub match_queue_repo: Arc<dyn MatchQueueRepository + Send + Sync>,
    pub matching_engine: Option<Arc<MatchingEngine>>,
    pub ai_client: Arc<PharmaParser>,
    pub alias_learner: Arc<AliasLearner>,
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
            participant_repo: self.participant_repo.clone(),
            raw_message_repo: self.raw_message_repo.clone(),
            review_queue_repo: self.review_queue_repo.clone(),
            audit_log_repo: self.audit_log_repo.clone(),
            medication_mapping_repo: self.medication_mapping_repo.clone(),
            medication_master_repo: self.medication_master_repo.clone(),
            medication_alias_repo: self.medication_alias_repo.clone(),
            match_queue_repo: self.match_queue_repo.clone(),
            matching_engine: self.matching_engine.clone(),
            ai_client: self.ai_client.clone(),
            alias_learner: self.alias_learner.clone(),
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
            "/api/matching/calibration",
            get(calibration::get_calibration::<RQ, A, MM>),
        )
        .route(
            "/api/matches/{id}/confirm",
            post(handlers::confirm_match::<RQ, A, MM>),
        )
        .route(
            "/api/matches/{id}/reject",
            post(handlers::reject_match::<RQ, A, MM>),
        )
        // Curation
        .route(
            "/api/curation/stats",
            get(curation::get_curation_stats::<RQ, A, MM>),
        )
        .route(
            "/api/curation/aliases",
            get(curation::list_aliases::<RQ, A, MM>),
        )
        .route(
            "/api/curation/master",
            post(curation::create_master::<RQ, A, MM>),
        )
        .route(
            "/api/curation/master/{id}",
            put(curation::update_master::<RQ, A, MM>),
        )
        .route(
            "/api/curation/aliases/{alias_id}/approve",
            put(curation::approve_alias::<RQ, A, MM>),
        )
        .route(
            "/api/curation/link",
            post(curation::link_alias_to_master::<RQ, A, MM>),
        )
        .route(
            "/api/curation/aliases/bulk-approve",
            post(curation::bulk_approve_aliases::<RQ, A, MM>),
        )
        .route(
            "/api/curation/suggestions",
            get(curation::get_suggestions::<RQ, A, MM>),
        )
        // Reclassification (Offer <-> Request)
        .route(
            "/api/reclassify",
            post(reclassify::reclassify_item::<RQ, A, MM>),
        )
        .route(
            "/api/match/rematch",
            post(matching::rematch_item::<RQ, A, MM>),
        )
        .route(
            "/api/items/{item_type}/{id}",
            get(reclassify::get_item::<RQ, A, MM>),
        )
        // Re-parse with AI
        .route("/api/reparse", post(reparse::reparse_item::<RQ, A, MM>))
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
        // Review Queue (AI Parsing)
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
        // Match Reviews (Offer-Request Matches)
        .route(
            "/api/match-reviews",
            get(match_reviews::list_match_reviews::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/stats",
            get(match_reviews::get_match_review_stats::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/{id}",
            get(match_reviews::get_match_review::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/{id}/status",
            put(match_reviews::update_match_review_status::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/{id}/re-audit",
            post(match_reviews::re_audit_match::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/{id}/recalculate",
            post(match_reviews::recalculate_confidence::<RQ, A, MM>),
        )
        .route(
            "/api/match-reviews/bulk",
            post(match_reviews::bulk_update_match_reviews::<RQ, A, MM>),
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
        // Match Filter Management
        .route(
            "/api/match-filter",
            get(match_filter::get_match_filter::<RQ, A, MM>),
        )
        .route(
            "/api/match-filter",
            put(match_filter::update_match_filter::<RQ, A, MM>),
        )
        .route(
            "/api/match-filter/stale",
            post(match_filter::toggle_stale_filter::<RQ, A, MM>),
        )
        .route(
            "/api/match-filter/same-sender",
            post(match_filter::toggle_same_sender::<RQ, A, MM>),
        )
        .route(
            "/api/match-filter/stats/reset",
            post(match_filter::reset_stats::<RQ, A, MM>),
        )
        // Embedding Cache Management
        .route(
            "/api/embedding-cache",
            get(embedding_cache::get_cache_stats::<RQ, A, MM>),
        )
        .route(
            "/api/embedding-cache/refresh",
            post(embedding_cache::refresh_cache::<RQ, A, MM>),
        )
        .route(
            "/api/embedding-cache/clear",
            post(embedding_cache::clear_cache::<RQ, A, MM>),
        )
        .route(
            "/api/embedding-cache/lookup/{term}",
            get(embedding_cache::lookup_term::<RQ, A, MM>),
        )
        .route(
            "/api/embedding-cache/synonyms",
            post(embedding_cache::check_synonyms::<RQ, A, MM>),
        )
        .route(
            "/api/embedding-cache/embedding/{term}",
            get(embedding_cache::get_embedding::<RQ, A, MM>),
        )
        // Audit Trail Management
        .route(
            "/api/audit-trail",
            get(audit_trail::get_audit_trail::<RQ, A, MM>),
        )
        .route(
            "/api/audit-trail",
            put(audit_trail::update_audit_trail::<RQ, A, MM>),
        )
        .route(
            "/api/audit-trail/enable",
            post(audit_trail::toggle_audit_trail::<RQ, A, MM>),
        )
        .route(
            "/api/audit-trail/match/{match_id}",
            get(audit_trail::get_match_history::<RQ, A, MM>),
        )
        .route(
            "/api/audit-trail/recent",
            get(audit_trail::get_recent_actions::<RQ, A, MM>),
        )
        // Database Diagnostics
        .route(
            "/api/diagnostics/health",
            get(diagnostics::get_health::<RQ, A, MM>),
        )
        .route(
            "/api/diagnostics/tables",
            get(diagnostics::get_table_stats::<RQ, A, MM>),
        )
        .route(
            "/api/diagnostics/indexes",
            get(diagnostics::get_index_stats::<RQ, A, MM>),
        )
        .route(
            "/api/diagnostics/queries",
            get(diagnostics::analyze_queries::<RQ, A, MM>),
        )
        // Audit Records (Match Debug/Replay)
        .route(
            "/api/audit-records",
            get(audit_records::list_audit_records::<RQ, A, MM>),
        )
        .route(
            "/api/audit-records/status",
            get(audit_records::get_audit_recorder_status::<RQ, A, MM>),
        )
        .route(
            "/api/audit-records/session/{session_id}",
            get(audit_records::get_session_records::<RQ, A, MM>),
        )
        .route(
            "/api/audit-records/{match_id}",
            get(audit_records::get_audit_record::<RQ, A, MM>),
        )
        .route(
            "/api/audit-records/{match_id}/review",
            put(audit_records::update_audit_review::<RQ, A, MM>),
        )
        // Uncertainty Estimation
        .route(
            "/api/uncertainty/status",
            get(uncertainty::get_uncertainty_status::<RQ, A, MM>),
        )
        .route(
            "/api/uncertainty/estimate",
            post(uncertainty::estimate_uncertainty::<RQ, A, MM>),
        )
        .route(
            "/api/uncertainty/batch",
            post(uncertainty::batch_estimate_uncertainty::<RQ, A, MM>),
        )
        .route(
            "/api/uncertainty/match/{match_id}",
            get(uncertainty::estimate_match_uncertainty::<RQ, A, MM>),
        )
        // Participants
        .route(
            "/api/participants/{id}/stats",
            get(participants::get_participant_stats::<RQ, A, MM>),
        )
        .route(
            "/api/participants/by-jid/{jid}",
            get(participants::get_participant_by_jid::<RQ, A, MM>),
        )
        // Match Review Notes
        .route(
            "/api/match-reviews/{id}/notes",
            put(match_reviews::update_match_notes::<RQ, A, MM>),
        )
        // WebSocket
        .route("/ws", get(ws::ws_handler::<RQ, A, MM>))
        .with_state(state)
        // CORS: Allow all origins for development
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
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
