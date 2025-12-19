//! gRPC Server Implementation
//!
//! Handles messages from the Go WhatsApp bridge

use std::net::SocketAddr;
use std::sync::Arc;

use tonic::{Request, Response, Status};

use super::pharma::{
    HealthRequest, HealthResponse, ProcessResponse, RawMessage, StatsRequest, StatsResponse,
    pharma_core_server::{PharmaCore, PharmaCoreServer},
};
use crate::repository::{OfferRepository, RequestRepository};

/// The gRPC service implementation
pub struct PharmaCoreService<O, R>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    start_time: std::time::Instant,
}

impl<O, R> PharmaCoreService<O, R>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
{
    pub fn new(offer_repo: Arc<O>, request_repo: Arc<R>) -> Self {
        Self {
            offer_repo,
            request_repo,
            start_time: std::time::Instant::now(),
        }
    }
}

#[tonic::async_trait]
impl<O, R> PharmaCore for PharmaCoreService<O, R>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
{
    /// Process an incoming WhatsApp message
    async fn process_message(
        &self,
        request: Request<RawMessage>,
    ) -> Result<Response<ProcessResponse>, Status> {
        let msg = request.into_inner();

        tracing::info!(
            id = %msg.id,
            group = %msg.group_jid,
            sender = %msg.sender_phone,
            content_len = msg.content.len(),
            "📨 Received message from Go bridge"
        );

        // TODO: Integrate with AI parsing pipeline
        // TODO: Create offers/requests based on parsed content
        // TODO: Trigger matching engine

        Ok(Response::new(ProcessResponse {
            success: true,
            message_id: msg.id,
            error: None,
        }))
    }

    /// Get current statistics
    async fn get_stats(
        &self,
        _request: Request<StatsRequest>,
    ) -> Result<Response<StatsResponse>, Status> {
        let active_offers = self.offer_repo.count_active().await.unwrap_or(0);
        let active_requests = self.request_repo.count_active().await.unwrap_or(0);

        Ok(Response::new(StatsResponse {
            active_offers,
            active_requests,
            pending_matches: 0,
            confirmed_today: 0,
            processed_today: 0,
            avg_match_score: 0.0,
        }))
    }

    /// Health check
    async fn health_check(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse {
            healthy: true,
            version: "0.1.0".to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs() as i64,
        }))
    }
}

/// Start the gRPC server on the specified address
pub async fn start_grpc_server<O, R>(
    addr: SocketAddr,
    service: PharmaCoreService<O, R>,
) -> Result<(), tonic::transport::Error>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
{
    tracing::info!("🔌 gRPC server starting on {}", addr);

    tonic::transport::Server::builder()
        .add_service(PharmaCoreServer::new(service))
        .serve(addr)
        .await
}
