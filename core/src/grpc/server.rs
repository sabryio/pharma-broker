//! gRPC Server Implementation
//!
//! Handles messages from the Go WhatsApp bridge

use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use super::pharma::{
    HealthRequest, HealthResponse, ProcessResponse, RawMessage as ProtoRawMessage, StatsRequest,
    StatsResponse,
    pharma_core_server::{PharmaCore, PharmaCoreServer},
};
use crate::domain::RawMessage;
use crate::repository::{OfferRepository, RawMessageRepository, RequestRepository};

/// The gRPC service implementation
pub struct PharmaCoreService<O, R, M>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    pub raw_message_repo: Arc<M>,
    start_time: std::time::Instant,
}

impl<O, R, M> PharmaCoreService<O, R, M>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
{
    pub fn new(offer_repo: Arc<O>, request_repo: Arc<R>, raw_message_repo: Arc<M>) -> Self {
        Self {
            offer_repo,
            request_repo,
            raw_message_repo,
            start_time: std::time::Instant::now(),
        }
    }
}

/// Convert proto RawMessage to domain RawMessage
fn proto_to_domain(proto: &ProtoRawMessage) -> RawMessage {
    // Convert timestamp from Unix seconds to DateTime
    let timestamp = DateTime::from_timestamp(proto.timestamp, 0).unwrap_or_else(Utc::now);

    RawMessage {
        id: if proto.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            proto.id.clone()
        },
        external_id: proto.external_id.clone(),
        group_jid: proto.group_jid.clone(),
        group_name: proto.group_name.clone(),
        sender_jid: proto.sender_jid.clone(),
        sender_phone: proto.sender_phone.clone(),
        sender_name: proto.sender_name.clone(),
        content: proto.content.clone(),
        timestamp,
        processed_at: None,
        error: None,
        reply_to_id: proto.reply_to_id.clone(),
        reply_to_content: proto.reply_to_content.clone(),
        reply_to_sender: proto.reply_to_sender.clone(),
    }
}

#[tonic::async_trait]
impl<O, R, M> PharmaCore for PharmaCoreService<O, R, M>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
{
    /// Process an incoming WhatsApp message
    async fn process_message(
        &self,
        request: Request<ProtoRawMessage>,
    ) -> Result<Response<ProcessResponse>, Status> {
        let proto_msg = request.into_inner();

        tracing::info!(
            id = %proto_msg.id,
            group = %proto_msg.group_jid,
            sender = %proto_msg.sender_phone,
            content_len = proto_msg.content.len(),
            "📨 Received message from Go bridge"
        );

        // Convert proto to domain entity
        let raw_message = proto_to_domain(&proto_msg);
        let message_id = raw_message.id.clone();

        // Save to database
        if let Err(e) = self.raw_message_repo.save(&raw_message).await {
            tracing::error!(error = %e, id = %message_id, "Failed to save raw message");
            return Ok(Response::new(ProcessResponse {
                success: false,
                message_id,
                error: Some(format!("Database error: {}", e)),
            }));
        }

        tracing::info!(id = %message_id, "✅ Message saved to database");

        // TODO: Integrate with AI parsing pipeline
        // TODO: Create offers/requests based on parsed content
        // TODO: Trigger matching engine

        Ok(Response::new(ProcessResponse {
            success: true,
            message_id,
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
pub async fn start_grpc_server<O, R, M>(
    addr: SocketAddr,
    service: PharmaCoreService<O, R, M>,
) -> Result<(), tonic::transport::Error>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
{
    tracing::info!("🔌 gRPC server starting on {}", addr);

    tonic::transport::Server::builder()
        .add_service(PharmaCoreServer::new(service))
        .serve(addr)
        .await
}
