//! gRPC Server Implementation
//!
//! Handles messages from the Go WhatsApp bridge

use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use pgvector::Vector as PgVector;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::{ai::Intent, metrics, ws::WsEvent};

use super::pharma::{
    GroupInfo as ProtoGroupInfo, HealthRequest, HealthResponse, MonitoredGroupsRequest,
    MonitoredGroupsResponse, ProcessResponse, RawMessage as ProtoRawMessage, StatsRequest,
    StatsResponse, SyncGroupsRequest, SyncGroupsResponse,
    pharma_core_server::{PharmaCore, PharmaCoreServer},
};
use crate::ai::PharmaParser;
use crate::ai::UrgencyLevel as AiUrgencyLevel;
use crate::domain::{
    AuditAction, AuditLog, EntityType, Group, ItemStatus, Offer, RawMessage,
    Request as RequestEntity, ReviewQueueItem, UrgencyLevel,
};
use crate::matching::AutoActionHandler;
use crate::matching::MatchingEngine;
use crate::matching::MedicationResolver;
use crate::repository::{
    AuditLogRepository, FeedbackRepository, FindDuplicateParams, GroupRepository,
    MatchQueueRepository, MatchRepository, MedicationMasterRepository, OfferRepository,
    ParticipantRepository, RawMessageRepository, RequestRepository, ReviewQueueRepository,
    SemanticDuplicateParams,
};

/// The gRPC service implementation
pub struct PharmaCoreService<O, R, M, G, F, RQ, A, MQ, P>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
    P: ParticipantRepository + 'static,
{
    pub offer_repo: Arc<O>,
    pub request_repo: Arc<R>,
    pub raw_message_repo: Arc<M>,
    pub group_repo: Arc<G>,
    pub participant_repo: Arc<P>,
    pub feedback_repo: Arc<F>,
    pub review_queue_repo: Arc<RQ>,
    pub audit_log_repo: Arc<A>,
    pub match_queue_repo: Arc<MQ>,
    pub medication_master_repo: Arc<dyn MedicationMasterRepository + Send + Sync>,
    pub match_repo: Arc<dyn MatchRepository + Send + Sync>,
    pub ai_client: Arc<PharmaParser>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub matching_engine: Arc<MatchingEngine>,
    pub medication_resolver: Option<Arc<MedicationResolver>>,
    pub auto_action: AutoActionHandler,
    start_time: std::time::Instant,
}

impl<O, R, M, G, F, RQ, A, MQ, P> PharmaCoreService<O, R, M, G, F, RQ, A, MQ, P>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
    P: ParticipantRepository + 'static,
{
    /// Create a new service from structured parameter objects
    pub fn new(
        repos: super::params::GrpcRepositories<O, R, M, G, F, RQ, A, MQ, P>,
        deps: super::params::GrpcDependencies,
    ) -> Self {
        Self {
            offer_repo: repos.offer,
            request_repo: repos.request,
            raw_message_repo: repos.raw_message,
            group_repo: repos.group,
            participant_repo: repos.participant,
            feedback_repo: repos.feedback,
            review_queue_repo: repos.review_queue,
            audit_log_repo: repos.audit_log,
            match_queue_repo: repos.match_queue,
            medication_master_repo: repos.medication_master,
            match_repo: repos.match_repo,
            ai_client: deps.ai_client,
            ws_tx: deps.ws_tx,
            matching_engine: deps.matching_engine,
            medication_resolver: deps.medication_resolver,
            auto_action: AutoActionHandler::from_env(),
            start_time: std::time::Instant::now(),
        }
    }
}

/// Convert proto RawMessage to domain RawMessage
fn proto_to_domain(proto: &ProtoRawMessage, participant_id: Uuid, group_id: Uuid) -> RawMessage {
    let timestamp = DateTime::from_timestamp(proto.timestamp, 0).unwrap_or_else(Utc::now);

    RawMessage {
        id: if proto.id.is_empty() {
            Uuid::new_v4()
        } else {
            // Try to parse as UUID, fallback to new UUID if invalid
            Uuid::parse_str(&proto.id).unwrap_or_else(|_| Uuid::new_v4())
        },
        external_id: if proto.external_id.is_empty() {
            None
        } else {
            Some(proto.external_id.clone())
        },
        participant_id,
        group_id,
        content: proto.content.clone(),
        timestamp,
        processed_at: None,
        error: None,
        reply_to_id: proto.reply_to_id.clone(),
        reply_to_content: proto.reply_to_content.clone(),
        reply_to_sender: proto.reply_to_sender.clone(),
        created_at: Utc::now(),
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
impl<O, R, M, G, F, RQ, A, MQ, P> PharmaCore for PharmaCoreService<O, R, M, G, F, RQ, A, MQ, P>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
    P: ParticipantRepository + 'static,
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

        // Step 1: Resolve Group
        let group = match self.group_repo.get_by_jid(&proto_msg.group_jid).await {
            Ok(Some(g)) => g,
            Ok(None) => {
                // Auto-sync/create group if missing
                let new_group = Group {
                    id: Uuid::new_v4(),
                    jid: proto_msg.group_jid.clone(),
                    name: proto_msg.group_name.clone(),
                    description: None,
                    monitoring: true, // Default to monitored for new groups discovered via bridge
                    parsing: true,
                    added_at: Utc::now(),
                    last_message: Some(Utc::now()),
                    message_count: 0,
                    member_count: 0,
                };
                if let Err(e) = self.group_repo.save(&new_group).await {
                    tracing::error!(error = %e, group = %proto_msg.group_jid, "Failed to auto-create group");
                    return Ok(Response::new(ProcessResponse {
                        success: false,
                        message_id: proto_msg.id.clone(),
                        error: Some(format!("Group lookup/creation failed: {}", e)),
                    }));
                }
                new_group
            }
            Err(e) => {
                tracing::error!(error = %e, group = %proto_msg.group_jid, "Group lookup error");
                return Ok(Response::new(ProcessResponse {
                    success: false,
                    message_id: proto_msg.id.clone(),
                    error: Some(format!("Database error: {}", e)),
                }));
            }
        };

        if !group.monitoring {
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

        // Step 2: Resolve Participant
        let participant = match self
            .participant_repo
            .get_by_jid(&proto_msg.sender_jid)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => {
                let p = crate::domain::Participant {
                    id: Uuid::new_v4(),
                    jid: proto_msg.sender_jid.clone(),
                    phone: proto_msg.sender_phone.clone(),
                    push_name: if proto_msg.sender_name.is_empty() {
                        None
                    } else {
                        Some(proto_msg.sender_name.clone())
                    },
                    display_name: None,
                    label: None,
                    notes: None,
                    is_blocked: false,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                if let Err(e) = self.participant_repo.save(&p).await {
                    tracing::error!(error = %e, sender = %proto_msg.sender_jid, "Failed to create participant");
                    return Ok(Response::new(ProcessResponse {
                        success: false,
                        message_id: proto_msg.id.clone(),
                        error: Some(format!("Participant creation failed: {}", e)),
                    }));
                }
                p
            }
            Err(e) => {
                tracing::error!(error = %e, sender = %proto_msg.sender_jid, "Participant lookup error");
                return Ok(Response::new(ProcessResponse {
                    success: false,
                    message_id: proto_msg.id.clone(),
                    error: Some(format!("Database error: {}", e)),
                }));
            }
        };

        // Step 3: Link Participant to Group
        if let Err(e) = self
            .participant_repo
            .add_to_group(participant.id, group.id)
            .await
        {
            tracing::warn!(error = %e, participant = %participant.id, group = %group.id, "Failed to link participant to group");
        }

        // Step 4: Convert proto to domain entity
        let raw_message = proto_to_domain(&proto_msg, participant.id, group.id);
        let message_id = raw_message.id;

        // Step 5: Save to database (handle duplicates gracefully)
        match self.raw_message_repo.save(&raw_message).await {
            Ok(_) => {
                // New message saved successfully
            }
            Err(e) => {
                // Check if this is a duplicate key error
                let error_str = e.to_string();
                if error_str.contains("duplicate key") || error_str.contains("unique constraint") {
                    tracing::info!(
                        id = %message_id,
                        "⏭️ Message already exists, skipping (duplicate)"
                    );
                    return Ok(Response::new(ProcessResponse {
                        success: true,
                        message_id: message_id.to_string(),
                        error: None,
                    }));
                }
                // Other database errors
                tracing::error!(error = %e, id = %message_id, "Failed to save raw message");
                return Ok(Response::new(ProcessResponse {
                    success: false,
                    message_id: message_id.to_string(),
                    error: Some(format!("Database error: {}", e)),
                }));
            }
        }

        // Record message received metric
        metrics::record_message_received(&group.jid, "saved");
        tracing::info!(id = %message_id, "✅ Message saved to database");
        let group_jid = group.jid.clone();

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
        let msg_id = message_id;
        let content = raw_message.content.clone();
        let sender_name = participant.push_name.clone();
        let group_name = group.name.clone();
        let reply_to = raw_message.reply_to_content.clone();
        let ws_tx = self.ws_tx.clone();
        let _match_repo = self.match_repo.clone();
        let _matching_engine = self.matching_engine.clone();
        let review_queue_repo = self.review_queue_repo.clone();
        let audit_log_repo = self.audit_log_repo.clone();
        let auto_action = self.auto_action.clone();
        let match_queue_repo = self.match_queue_repo.clone();
        let medication_master_repo = self.medication_master_repo.clone();
        let medication_resolver = self.medication_resolver.clone();

        tokio::spawn(async move {
            tracing::info!(id = %msg_id, "🤖 Starting AI parsing (background)");

            // Step 5a: Fetch medication mappings (RAG-Lite)
            let mappings_vec: Vec<String> =
                match medication_master_repo.find_relevant(&content, 5).await {
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
                    sender_name.as_deref(),
                    &group_name,
                    reply_to.as_deref(),
                    mappings_opt,
                )
                .await
            {
                Ok(items) => {
                    metrics::record_ai_parse("success");
                    items
                }
                Err(e) => {
                    metrics::record_ai_parse("error");
                    tracing::error!(error = %e, id = %msg_id, "AI parsing failed");
                    let _ = raw_message_repo
                        .mark_processed(msg_id, Some(&e.to_string()))
                        .await;
                    return;
                }
            };

            if parsed_items.is_empty() {
                tracing::debug!(id = %msg_id, "🎯 AI parsing complete (no items found)");
            } else {
                tracing::info!(
                    id = %msg_id,
                    items_count = parsed_items.len(),
                    "🎯 AI parsing complete"
                );
            }

            // Create Offer/Request entities from parsed items
            let mut offers_created = 0;
            let mut requests_created = 0;
            let mut items_queued = 0;
            let mut new_request_ids: Vec<_> = Vec::new();

            // Pre-filter items that will be accepted (need embeddings)
            let accepted_items: Vec<_> = parsed_items
                .iter()
                .filter(|item| {
                    matches!(
                        auto_action.determine_parse_action(item.ai_confidence),
                        crate::matching::ParseAction::Accept
                    )
                })
                .collect();

            // Batch generate embeddings for accepted items
            // IMPORTANT: Strip dosage from medication names before embedding to prevent false positives
            // e.g., "Kozentex 150" and "Gonapure 150" would have similar embeddings due to "150"
            let medications_normalized: Vec<String> = accepted_items
                .iter()
                .map(|item| crate::matching::arabic::normalize_for_matching(&item.medication))
                .collect();
            let medications_original: Vec<String> = accepted_items
                .iter()
                .map(|item| item.medication.clone())
                .collect();

            let embeddings_map: std::collections::HashMap<String, Vec<f32>> =
                if !medications_normalized.is_empty() {
                    match ai_client.embed_batch(&medications_normalized).await {
                        Ok(embs) => medications_original.into_iter().zip(embs).collect(),
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to batch generate embeddings");
                            std::collections::HashMap::new()
                        }
                    }
                } else {
                    std::collections::HashMap::new()
                };

            for item in parsed_items {
                // Task 3.3: Determine action based on AI confidence
                let parse_action = auto_action.determine_parse_action(item.ai_confidence);

                match parse_action {
                    crate::matching::ParseAction::Accept => {
                        // Get pre-generated embedding
                        let content_embedding = embeddings_map.get(&item.medication).cloned();

                        // Dynamic medication resolution
                        let (master_medication_id, medication_curated) =
                            if let Some(resolver) = &medication_resolver {
                                let resolution = if let Some(emb) = &content_embedding {
                                    resolver.resolve_with_embedding(&item.medication, emb).await
                                } else {
                                    resolver.resolve(&item.medication).await
                                };

                                if resolution.master_medication_id.is_some() {
                                    tracing::info!(
                                        medication = %item.medication,
                                        master_id = ?resolution.master_medication_id,
                                        method = %resolution.method,
                                        confidence = %resolution.confidence,
                                        auto_approved = %resolution.auto_approved,
                                        "🔗 Dynamic medication resolution"
                                    );
                                }

                                (resolution.master_medication_id, resolution.auto_approved)
                            } else {
                                (None, false)
                            };

                        if item.item_type == Intent::Offer {
                            let offer = Offer {
                                id: Uuid::new_v4(),
                                raw_message_id: msg_id,
                                participant_id: participant.id,
                                group_id: group.id,
                                medication: item.medication.clone(),
                                medication_raw: item.medication_raw.clone(),
                                quantity: Decimal::from_f64(item.quantity),
                                unit: item.unit.clone(),
                                price: Decimal::from_f64(item.price),
                                currency: Some("EGP".to_string()),
                                expiry_date: None,
                                batch_number: None,
                                notes: item.notes.clone(),
                                status: ItemStatus::Active,
                                content_embedding: content_embedding.clone().map(PgVector::from),
                                urgency_level: convert_urgency_level(item.urgency_level),
                                expiry_info: item.expiry.clone(),
                                ai_confidence: item.ai_confidence,
                                master_medication_id, // Dynamic resolution
                                medication_curated,
                                confirmed_match_count: 0,
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            };

                            // Check for duplicates (Exact then Semantic)
                            let mut offer = offer;
                            let mut is_duplicate = false;

                            // 1. Exact match check
                            if let Ok(Some(existing)) = offer_repo
                                .find_recent_duplicate(FindDuplicateParams::new(
                                    participant.id,
                                    &offer.medication,
                                    chrono::Duration::minutes(10),
                                ))
                                .await
                            {
                                tracing::info!(id = %offer.id, existing = %existing.id, "Duplicate offer detected (exact)");
                                is_duplicate = true;
                            }
                            // 2. Semantic match check
                            else if let Some(emb) = &offer.content_embedding
                                && let Ok(semantic_dups) = offer_repo
                                    .find_semantic_duplicates(SemanticDuplicateParams::new(
                                        emb.as_slice(),
                                        0.95,
                                        chrono::Duration::minutes(10),
                                    ))
                                    .await
                            {
                                // Filter by same sender
                                if let Some(existing) = semantic_dups
                                    .iter()
                                    .find(|o| o.participant_id == participant.id)
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
                                metrics::record_offer_created();
                                offers_created += 1;
                                let _ = ws_tx.send(WsEvent::NewOffer(offer.clone()));

                                // Task 5.3: Audit log Offer creation
                                let audit_log = AuditLog::system(
                                    AuditAction::OfferCreated,
                                    EntityType::Offer,
                                    offer.id,
                                )
                                .with_details(serde_json::json!({ "message_id": msg_id }));
                                let _ = audit_log_repo.save(&audit_log).await;
                            }
                        } else if item.item_type == Intent::Request {
                            let request = RequestEntity {
                                id: Uuid::new_v4(),
                                raw_message_id: msg_id,
                                participant_id: participant.id,
                                group_id: group.id,
                                medication: item.medication.clone(),
                                medication_raw: item.medication_raw.clone(),
                                quantity: Decimal::from_f64(item.quantity),
                                unit: item.unit.clone(),
                                max_price: Decimal::from_f64(item.max_price),
                                currency: Some("EGP".to_string()),
                                urgency_level: convert_urgency_level(item.urgency_level),
                                expiry_requirement: item.expiry.clone(),
                                ai_confidence: item.ai_confidence,
                                notes: item.notes.clone(),
                                status: ItemStatus::Active,
                                content_embedding: content_embedding.clone().map(PgVector::from),
                                master_medication_id, // Dynamic resolution (reuse from above)
                                medication_curated,
                                confirmed_match_count: 0,
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            };

                            // Check for duplicates (Exact then Semantic)
                            let mut request = request;
                            let mut is_duplicate = false;

                            // 1. Exact match check
                            if let Ok(Some(existing)) = request_repo
                                .find_recent_duplicate(FindDuplicateParams::new(
                                    participant.id,
                                    &request.medication,
                                    chrono::Duration::minutes(10),
                                ))
                                .await
                            {
                                tracing::info!(id = %request.id, existing = %existing.id, "Duplicate request detected (exact)");
                                is_duplicate = true;
                            }
                            // 2. Semantic match check
                            else if let Some(emb) = &request.content_embedding
                                && let Ok(semantic_dups) = request_repo
                                    .find_semantic_duplicates(SemanticDuplicateParams::new(
                                        emb.as_slice(),
                                        0.95,
                                        chrono::Duration::minutes(10),
                                    ))
                                    .await
                            {
                                // Filter by same sender
                                if let Some(existing) = semantic_dups
                                    .iter()
                                    .find(|r| r.participant_id == participant.id)
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
                                    is_duplicate = is_duplicate,
                                    "❓ Request created"
                                );
                                metrics::record_request_created();
                                requests_created += 1;
                                // Only enqueue non-duplicate requests for matching
                                if !is_duplicate {
                                    new_request_ids.push(request.id);
                                }
                                let _ = ws_tx.send(WsEvent::NewRequest(request.clone()));

                                // Task 5.3: Audit log Request creation
                                let audit_log = AuditLog::system(
                                    AuditAction::RequestCreated,
                                    EntityType::Request,
                                    request.id,
                                )
                                .with_details(serde_json::json!({ "message_id": msg_id }));
                                let _ = audit_log_repo.save(&audit_log).await;
                            }
                        }
                    }
                    crate::matching::ParseAction::QueueForReview => {
                        // Task 3.3: Queue for human review
                        let review_item = ReviewQueueItem::for_low_confidence(
                            msg_id,
                            serde_json::to_value(&item).unwrap_or(serde_json::Value::Null),
                            item.ai_confidence,
                            "low_confidence",
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
                            metrics::record_ai_parse("queued_review");
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
            let _ = raw_message_repo.mark_processed(msg_id, None).await;

            if offers_created == 0 && requests_created == 0 && items_queued == 0 {
                tracing::debug!(id = %msg_id, "✅ Background processing complete (no items created)");
            } else {
                tracing::info!(
                    id = %msg_id,
                    offers = offers_created,
                    requests = requests_created,
                    queued = items_queued,
                    "✅ Background processing complete"
                );
            }

            // Trigger matching engine for newly created requests only
            for request_id in new_request_ids {
                if let Err(e) = match_queue_repo.enqueue(request_id, 0).await {
                    tracing::error!(error = %e, request_id = %request_id, "Failed to enqueue request for matching");
                } else {
                    tracing::debug!(request_id = %request_id, "Queued request for matching");
                }
            }
        });

        Ok(Response::new(ProcessResponse {
            success: true,
            message_id: message_id.to_string(),
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

    /// Sync groups from WhatsApp Bridge to Core
    async fn sync_groups(
        &self,
        request: Request<SyncGroupsRequest>,
    ) -> Result<Response<SyncGroupsResponse>, Status> {
        let req = request.into_inner();
        let mut added: i32 = 0;
        let mut updated: i32 = 0;

        tracing::info!(count = req.groups.len(), "📱 Syncing groups from Bridge");

        for proto_group in req.groups {
            let ProtoGroupInfo {
                jid,
                name,
                description,
                member_count,
            } = proto_group;

            // Check if group exists
            let existing = self.group_repo.get_by_jid(&jid).await.ok().flatten();

            let group = Group {
                id: existing.as_ref().map(|e| e.id).unwrap_or_else(Uuid::new_v4),
                jid: jid.clone(),
                name: name.clone(),
                description: if description.is_empty() {
                    None
                } else {
                    Some(description.clone())
                },
                // Preserve monitoring status for existing groups, default to false for new
                monitoring: existing.as_ref().map(|e| e.monitoring).unwrap_or(false),
                parsing: true,
                added_at: existing
                    .as_ref()
                    .map(|e| e.added_at)
                    .unwrap_or_else(Utc::now),
                last_message: existing.as_ref().and_then(|e| e.last_message),
                message_count: existing.as_ref().map(|e| e.message_count).unwrap_or(0),
                member_count,
            };

            if existing.is_some() {
                updated += 1;
            } else {
                added += 1;
            }

            if let Err(e) = self.group_repo.save(&group).await {
                tracing::warn!(error = %e, jid = %jid, "Failed to save group");
            }
        }

        tracing::info!(added, updated, "✅ Groups synced from Bridge");

        Ok(Response::new(SyncGroupsResponse {
            success: true,
            added,
            updated,
            error: String::new(),
        }))
    }
}

/// Start the gRPC server on the specified address
pub async fn start_grpc_server<O, R, M, G, F, RQ, A, MQ, P, S>(
    addr: SocketAddr,
    service: PharmaCoreService<O, R, M, G, F, RQ, A, MQ, P>,
    shutdown: S,
) -> std::result::Result<(), tonic::transport::Error>
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: RawMessageRepository + 'static,
    G: GroupRepository + 'static,
    F: FeedbackRepository + 'static,
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MQ: MatchQueueRepository + 'static,
    P: ParticipantRepository + 'static,
    S: std::future::Future<Output = ()> + Send + 'static,
{
    tracing::info!("🔌 gRPC server starting on {}", addr);

    tonic::transport::Server::builder()
        .add_service(PharmaCoreServer::new(service))
        .serve_with_shutdown(addr, shutdown)
        .await
}
