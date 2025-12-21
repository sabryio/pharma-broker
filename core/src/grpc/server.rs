//! gRPC Server Implementation
//!
//! Handles messages from the Go WhatsApp bridge

use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::{ai::Intent, ws::WsEvent};

use super::pharma::{
    HealthRequest, HealthResponse, MonitoredGroupsRequest, MonitoredGroupsResponse,
    ProcessResponse, RawMessage as ProtoRawMessage, StatsRequest, StatsResponse,
    pharma_core_server::{PharmaCore, PharmaCoreServer},
};
use crate::ai::PharmaParser;
use crate::ai::UrgencyLevel as AiUrgencyLevel;
use crate::domain::{
    AuditAction, AuditLog, EntityType, ItemStatus, Offer, RawMessage, Request as RequestEntity,
    ReviewQueueItem, UrgencyLevel,
};
use crate::matching::AutoActionHandler;
use crate::matching::MatchingEngine;
use crate::repository::{
    AuditLogRepository, FeedbackRecordRepository, GroupRepository, MatchQueueRepository,
    MatchRepository, MedicationMappingRepository, OfferRepository, RawMessageRepository,
    RequestRepository, ReviewQueueRepository,
};

/// The gRPC service implementation
pub struct PharmaCoreService<O, R, M, G, F, RQ, A, MQ>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRecordRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    pub raw_message_repo: Arc<M>,
    pub group_repo: Arc<G>,
    pub feedback_repo: Arc<F>,
    pub review_queue_repo: Arc<RQ>,
    pub audit_log_repo: Arc<A>,
    pub match_queue_repo: Arc<MQ>,
    pub medication_mapping_repo: Arc<dyn MedicationMappingRepository + Send + Sync>,
    pub match_repo: Arc<dyn MatchRepository + Send + Sync>,
    pub ai_client: Arc<PharmaParser>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub matching_engine: Arc<MatchingEngine>,
    pub auto_action: AutoActionHandler,
    start_time: std::time::Instant,
}

impl<O, R, M, G, F, RQ, A, MQ> PharmaCoreService<O, R, M, G, F, RQ, A, MQ>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRecordRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        offer_repo: Arc<O>,
        request_repo: Arc<R>,
        raw_message_repo: Arc<M>,
        group_repo: Arc<G>,
        feedback_repo: Arc<F>,
        review_queue_repo: Arc<RQ>,
        audit_log_repo: Arc<A>,
        match_queue_repo: Arc<MQ>,
        medication_mapping_repo: Arc<dyn MedicationMappingRepository + Send + Sync>,
        match_repo: Arc<dyn MatchRepository + Send + Sync>,
        ai_client: Arc<PharmaParser>,
        ws_tx: broadcast::Sender<WsEvent>,
        matching_engine: Arc<MatchingEngine>,
    ) -> Self {
        Self {
            offer_repo,
            request_repo,
            raw_message_repo,
            group_repo,
            feedback_repo,
            review_queue_repo,
            audit_log_repo,
            match_queue_repo,
            medication_mapping_repo,
            match_repo,
            ai_client,
            ws_tx,
            matching_engine,
            auto_action: AutoActionHandler::from_env(),
            start_time: std::time::Instant::now(),
        }
    }
}

/// Convert proto RawMessage to domain RawMessage
fn proto_to_domain(proto: &ProtoRawMessage) -> RawMessage {
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

/// Convert AI UrgencyLevel to domain UrgencyLevel
fn convert_urgency_level(ai_level: AiUrgencyLevel) -> UrgencyLevel {
    match ai_level {
        AiUrgencyLevel::Normal => UrgencyLevel::Normal,
        AiUrgencyLevel::Soon => UrgencyLevel::Soon,
        AiUrgencyLevel::Urgent => UrgencyLevel::Urgent,
        AiUrgencyLevel::Critical => UrgencyLevel::Critical,
    }
}

#[tonic::async_trait]
impl<O, R, M, G, F, RQ, A, MQ> PharmaCore for PharmaCoreService<O, R, M, G, F, RQ, A, MQ>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRecordRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
{
    /// Process an incoming WhatsApp message
    async fn process_message(
        &self,
        request: Request<ProtoRawMessage>,
    ) -> Result<Response<ProcessResponse>, Status> {
        // Extract trace ID from metadata or generate new one
        let trace_id = request
            .metadata()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let proto_msg = request.into_inner();

        tracing::info!(
            trace_id = %trace_id,
            id = %proto_msg.id,
            group = %proto_msg.group_jid,
            sender = %proto_msg.sender_phone,
            content_len = proto_msg.content.len(),
            "📨 Received message from Go bridge"
        );

        // Step 1: Check if group is monitored
        let is_monitored = match self.group_repo.is_monitored(&proto_msg.group_jid).await {
            Ok(monitored) => monitored,
            Err(e) => {
                tracing::warn!(error = %e, group = %proto_msg.group_jid, "Failed to check group monitoring, allowing by default");
                true
            }
        };

        if !is_monitored {
            tracing::info!(
                group = %proto_msg.group_jid,
                "⏭️ Group not monitored, skipping message"
            );
            return Ok(Response::new(ProcessResponse {
                success: true,
                message_id: proto_msg.id.clone(),
                error: Some("Group not monitored".to_string()),
            }));
        }

        // Step 2: Convert proto to domain entity
        let raw_message = proto_to_domain(&proto_msg);
        let message_id = raw_message.id.clone();
        let group_jid = raw_message.group_jid.clone();

        // Step 3: Save to database
        if let Err(e) = self.raw_message_repo.save(&raw_message).await {
            tracing::error!(error = %e, id = %message_id, "Failed to save raw message");
            return Ok(Response::new(ProcessResponse {
                success: false,
                message_id,
                error: Some(format!("Database error: {}", e)),
            }));
        }

        tracing::info!(id = %message_id, "✅ Message saved to database");

        // Step 4: Update group stats asynchronously
        let group_repo = self.group_repo.clone();
        let group_jid_clone = group_jid.clone();
        tokio::spawn(async move {
            if let Err(e) = group_repo.update_last_message(&group_jid_clone).await {
                tracing::debug!(error = %e, group = %group_jid_clone, "Failed to update last message");
            }
            if let Err(e) = group_repo.increment_message_count(&group_jid_clone).await {
                tracing::debug!(error = %e, group = %group_jid_clone, "Failed to increment message count");
            }
        });

        // Step 5: Spawn background AI parsing task
        let ai_client = self.ai_client.clone();
        let offer_repo = self.offer_repo.clone();
        let request_repo = self.request_repo.clone();
        let raw_message_repo = self.raw_message_repo.clone();
        let msg_id = message_id.clone();
        let content = raw_message.content.clone();
        let sender_name = raw_message.sender_name.clone();
        let group_name = raw_message.group_name.clone();
        let sender_phone = raw_message.sender_phone.clone();
        let reply_to = raw_message.reply_to_content.clone();
        let ws_tx = self.ws_tx.clone();
        let _match_repo = self.match_repo.clone();
        let _matching_engine = self.matching_engine.clone();
        let review_queue_repo = self.review_queue_repo.clone();
        let audit_log_repo = self.audit_log_repo.clone();
        let auto_action = self.auto_action.clone();
        let match_queue_repo = self.match_queue_repo.clone();
        let medication_mapping_repo = self.medication_mapping_repo.clone();

        tokio::spawn(async move {
            tracing::info!(id = %msg_id, "🤖 Starting AI parsing (background)");

            // Step 5a: Fetch medication mappings (RAG-Lite)
            let mappings_vec: Vec<String> =
                match medication_mapping_repo.find_relevant(&content, 5).await {
                    Ok(m) => m.into_iter().map(|map| map.to_prompt_context()).collect(),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to fetch medication mappings");
                        Vec::new()
                    }
                };
            let mappings_opt = if mappings_vec.is_empty() {
                None
            } else {
                Some(mappings_vec.as_slice())
            };

            // Call AI gateway
            let parsed_items = match ai_client
                .parse(
                    &content,
                    Some(&sender_name),
                    Some(&group_name),
                    reply_to.as_deref(),
                    mappings_opt,
                )
                .await
            {
                Ok(items) => items,
                Err(e) => {
                    tracing::error!(error = %e, id = %msg_id, "AI parsing failed");
                    let _ = raw_message_repo
                        .mark_processed(&msg_id, Some(&e.to_string()))
                        .await;
                    return;
                }
            };

            tracing::info!(
                id = %msg_id,
                items_count = parsed_items.len(),
                "🎯 AI parsing complete"
            );

            // Create Offer/Request entities from parsed items
            let mut offers_created = 0;
            let mut requests_created = 0;
            let mut items_queued = 0;

            for item in parsed_items {
                // Task 3.3: Determine action based on AI confidence
                let parse_action = auto_action.determine_parse_action(item.ai_confidence);

                match parse_action {
                    crate::matching::ParseAction::Accept => {
                        // Generate embedding for the medication name
                        let content_embedding = match ai_client.embed(&item.medication).await {
                            Ok(emb) => Some(emb),
                            Err(e) => {
                                tracing::warn!(error = %e, medication = %item.medication, "Failed to generate embedding");
                                None
                            }
                        };

                        if item.item_type == Intent::Offer {
                            let offer = Offer {
                                id: Uuid::new_v4().to_string(),
                                raw_message_id: msg_id.clone(),
                                source_phone: sender_phone.clone(),
                                source_name: sender_name.clone(),
                                source_group: group_jid.clone(),
                                group_name: group_name.clone(),
                                medication: item.medication.clone(),
                                medication_raw: item.medication_raw.clone(),
                                quantity: item.quantity,
                                unit: item.unit.clone(),
                                price: item.price,
                                currency: Some("EGP".to_string()),
                                expiry_date: None,
                                batch_number: None,
                                notes: item.notes.clone(),
                                raw_message: content.clone(),
                                status: ItemStatus::Active,
                                content_embedding: content_embedding.clone(),
                                urgent: item.urgent,
                                urgency_level: convert_urgency_level(item.urgency_level),
                                expiry_info: item.expiry.clone(),
                                ai_confidence: item.ai_confidence,
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            };

                            // Check for duplicates (Exact then Semantic)
                            let mut offer = offer;
                            let mut is_duplicate = false;

                            // 1. Exact match check
                            if let Ok(Some(existing)) = offer_repo
                                .find_recent_duplicate(
                                    &offer.source_phone,
                                    &offer.medication,
                                    chrono::Duration::minutes(10),
                                )
                                .await
                            {
                                tracing::info!(id = %offer.id, existing = %existing.id, "Duplicate offer detected (exact)");
                                is_duplicate = true;
                            }
                            // 2. Semantic match check
                            else if let Some(emb) = &offer.content_embedding
                                && let Ok(semantic_dups) = offer_repo
                                    .find_semantic_duplicates(
                                        emb,
                                        0.95,
                                        chrono::Duration::minutes(10),
                                    )
                                    .await
                            {
                                // Filter by same sender
                                if let Some(existing) = semantic_dups
                                    .iter()
                                    .find(|o| o.source_phone == offer.source_phone)
                                {
                                    tracing::info!(id = %offer.id, existing = %existing.id, "Duplicate offer detected (semantic)");
                                    is_duplicate = true;
                                }
                            }

                            if is_duplicate {
                                offer.status = ItemStatus::Duplicate;
                            }

                            if let Err(e) = offer_repo.save(&offer).await {
                                tracing::error!(error = %e, offer_id = %offer.id, "Failed to save offer");
                            } else {
                                tracing::info!(
                                    offer_id = %offer.id,
                                    medication = %offer.medication,
                                    "💊 Offer created"
                                );
                                offers_created += 1;
                                let _ = ws_tx.send(WsEvent::NewOffer(offer.clone()));

                                // Task 5.3: Audit log Offer creation
                                let audit_log = AuditLog::system(
                                    AuditAction::OfferCreated,
                                    EntityType::Offer,
                                    offer.id.clone(),
                                )
                                .with_details(serde_json::json!({ "message_id": msg_id }));
                                let _ = audit_log_repo.save(&audit_log).await;
                            }
                        } else if item.item_type == Intent::Request {
                            let request = RequestEntity {
                                id: Uuid::new_v4().to_string(),
                                raw_message_id: msg_id.clone(),
                                source_phone: sender_phone.clone(),
                                source_name: sender_name.clone(),
                                source_group: group_jid.clone(),
                                group_name: group_name.clone(),
                                medication: item.medication.clone(),
                                medication_raw: item.medication_raw.clone(),
                                quantity: item.quantity,
                                unit: item.unit.clone(),
                                max_price: item.max_price,
                                currency: Some("EGP".to_string()),
                                urgent: item.urgent,
                                urgency_level: convert_urgency_level(item.urgency_level),
                                expiry_requirement: item.expiry.clone(),
                                ai_confidence: item.ai_confidence,
                                notes: item.notes.clone(),
                                raw_message: content.clone(),
                                status: ItemStatus::Active,
                                content_embedding: content_embedding.clone(),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            };

                            // Check for duplicates (Exact then Semantic)
                            let mut request = request;
                            let mut is_duplicate = false;

                            // 1. Exact match check
                            if let Ok(Some(existing)) = request_repo
                                .find_recent_duplicate(
                                    &request.source_phone,
                                    &request.medication,
                                    chrono::Duration::minutes(10),
                                )
                                .await
                            {
                                tracing::info!(id = %request.id, existing = %existing.id, "Duplicate request detected (exact)");
                                is_duplicate = true;
                            }
                            // 2. Semantic match check
                            else if let Some(emb) = &request.content_embedding
                                && let Ok(semantic_dups) = request_repo
                                    .find_semantic_duplicates(
                                        emb,
                                        0.95,
                                        chrono::Duration::minutes(10),
                                    )
                                    .await
                            {
                                // Filter by same sender
                                if let Some(existing) = semantic_dups
                                    .iter()
                                    .find(|r| r.source_phone == request.source_phone)
                                {
                                    tracing::info!(id = %request.id, existing = %existing.id, "Duplicate request detected (semantic)");
                                    is_duplicate = true;
                                }
                            }

                            if is_duplicate {
                                request.status = ItemStatus::Duplicate;
                            }

                            if let Err(e) = request_repo.save(&request).await {
                                tracing::error!(error = %e, request_id = %request.id, "Failed to save request");
                            } else {
                                tracing::info!(
                                    request_id = %request.id,
                                    medication = %request.medication,
                                    "❓ Request created"
                                );
                                requests_created += 1;
                                let _ = ws_tx.send(WsEvent::NewRequest(request.clone()));

                                // Task 5.3: Audit log Request creation
                                let audit_log = AuditLog::system(
                                    AuditAction::RequestCreated,
                                    EntityType::Request,
                                    request.id.clone(),
                                )
                                .with_details(serde_json::json!({ "message_id": msg_id }));
                                let _ = audit_log_repo.save(&audit_log).await;
                            }
                        }
                    }
                    crate::matching::ParseAction::QueueForReview => {
                        // Task 3.3: Queue for human review
                        let review_item = ReviewQueueItem::for_low_confidence(
                            msg_id.clone(),
                            serde_json::to_value(&item).unwrap_or(serde_json::Value::Null),
                            item.ai_confidence,
                        );

                        if let Err(e) = review_queue_repo.save(&review_item).await {
                            tracing::error!(error = %e, id = %msg_id, "Failed to save review queue item");
                        } else {
                            tracing::info!(
                                id = %msg_id,
                                medication = %item.medication,
                                confidence = %item.ai_confidence,
                                "📋 Item queued for human review (low confidence)"
                            );
                            items_queued += 1;
                            let _ = ws_tx.send(WsEvent::ReviewQueued(review_item.id)); // Notify clients

                            // Task 5.3: Audit log Review Queue entry
                            let audit_log = AuditLog::system(
                                AuditAction::ReviewQueued,
                                EntityType::ReviewQueue,
                                review_item.id.to_string(),
                            )
                            .with_details(serde_json::json!({
                                "message_id": msg_id,
                                "confidence": item.ai_confidence
                            }));
                            let _ = audit_log_repo.save(&audit_log).await;
                        }
                    }
                    crate::matching::ParseAction::Reject => {
                        tracing::info!(
                            id = %msg_id,
                            medication = %item.medication,
                            confidence = %item.ai_confidence,
                            "🚫 Item rejected (too low confidence)"
                        );
                    }
                }
            }

            // Mark message as processed
            let _ = raw_message_repo.mark_processed(&msg_id, None).await;

            tracing::info!(
                id = %msg_id,
                offers = offers_created,
                requests = requests_created,
                queued = items_queued,
                "✅ Background processing complete"
            );

            // Trigger matching engine for new requests
            // Trigger matching engine via queue for new requests
            if requests_created > 0
                && let Ok(recent_requests) = request_repo.get_active(10, 0).await
            {
                for request in recent_requests {
                    // Enqueue for matching (Priority 0 default)
                    if let Err(e) = match_queue_repo.enqueue(&request.id, 0).await {
                        tracing::error!(error = %e, request_id = %request.id, "Failed to enqueue request for matching");
                    } else {
                        tracing::info!(request_id = %request.id, "Queued request for matching");
                    }
                }
            }
        });

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

    /// Get list of monitored group JIDs
    async fn get_monitored_groups(
        &self,
        _request: Request<MonitoredGroupsRequest>,
    ) -> Result<Response<MonitoredGroupsResponse>, Status> {
        let groups =
            self.group_repo.get_monitored().await.map_err(|e| {
                Status::internal(format!("Failed to fetch monitored groups: {}", e))
            })?;

        let jids = groups.into_iter().map(|g| g.jid).collect();

        Ok(Response::new(MonitoredGroupsResponse { jids }))
    }
}

/// Start the gRPC server on the specified address
pub async fn start_grpc_server<O, R, M, G, F, RQ, A, MQ, S>(
    addr: SocketAddr,
    service: PharmaCoreService<O, R, M, G, F, RQ, A, MQ>,
    shutdown: S,
) -> std::result::Result<(), tonic::transport::Error>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRecordRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
    S: std::future::Future<Output = ()> + Send + 'static,
{
    tracing::info!("🔌 gRPC server starting on {}", addr);

    tonic::transport::Server::builder()
        .add_service(PharmaCoreServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await
}
