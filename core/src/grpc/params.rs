//! Parameter structs for gRPC service construction
//!
//! Provides structured parameter types to avoid too_many_arguments warnings
//! while maintaining a clean API.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::ai::PharmaParser;
use crate::matching::MatchingEngine;
use crate::repository::{
    AuditLogRepository, FeedbackRepository, GroupRepository, MatchQueueRepository, MatchRepository,
    MedicationMappingRepository, OfferRepository, RawMessageRepository, RequestRepository,
    ReviewQueueRepository,
};
use crate::ws::WsEvent;

/// Repositories required by the gRPC service
pub struct GrpcRepositories<O, R, M, G, F, RQ, A, MQ>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
{
    pub offer: Arc<O>,
    pub request: Arc<R>,
    pub raw_message: Arc<M>,
    pub group: Arc<G>,
    pub feedback: Arc<F>,
    pub review_queue: Arc<RQ>,
    pub audit_log: Arc<A>,
    pub match_queue: Arc<MQ>,
    pub medication_mapping: Arc<dyn MedicationMappingRepository + Send + Sync>,
    pub match_repo: Arc<dyn MatchRepository + Send + Sync>,
}

/// Dependencies required by the gRPC service
pub struct GrpcDependencies {
    pub ai_client: Arc<PharmaParser>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub matching_engine: Arc<MatchingEngine>,
}

impl GrpcDependencies {
    pub fn new(
        ai_client: Arc<PharmaParser>,
        ws_tx: broadcast::Sender<WsEvent>,
        matching_engine: Arc<MatchingEngine>,
    ) -> Self {
        Self {
            ai_client,
            ws_tx,
            matching_engine,
        }
    }
}
