//! Batch processor implementation
//!
//! Accumulates messages and processes them in batches for efficiency.
//! Ported from legacy/parsing/processor.go

use pgvector::Vector as PgVector;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc, watch};
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::ai::{ParsedItem, PharmaParser};
use crate::domain::{
    AuditAction, AuditLog, EntityType, ItemStatus, Offer, RawMessage, Request, ReviewQueueItem,
    UrgencyLevel,
};
use crate::repository::{
    AuditLogRepository, GroupRepository, MatchQueueRepository, MedicationMasterRepository,
    OfferRepository, ParticipantRepository, RawMessageRepository, RequestRepository,
    ReviewQueueRepository,
};
use crate::ws::WsEvent;

use super::config::{BatchConfig, MultiPassConfig, ParsePass};
use super::{BatchStats, ParseJob, ParseJobResult};

/// Batch processor for message parsing
///
/// Accumulates incoming messages and processes them in batches
/// for more efficient AI gateway calls.
pub struct BatchProcessor {
    // Configuration
    config: BatchConfig,
    multi_pass_config: MultiPassConfig,

    // Dependencies
    ai_client: Arc<PharmaParser>,
    raw_message_repo: Arc<dyn RawMessageRepository>,
    offer_repo: Arc<dyn OfferRepository>,
    request_repo: Arc<dyn RequestRepository>,
    medication_master_repo: Arc<dyn MedicationMasterRepository>,
    review_queue_repo: Arc<dyn ReviewQueueRepository>,
    group_repo: Arc<dyn GroupRepository>,
    participant_repo: Arc<dyn ParticipantRepository>,
    audit_log_repo: Arc<dyn AuditLogRepository>,
    match_queue_repo: Arc<dyn MatchQueueRepository>,

    // Communication
    ws_tx: broadcast::Sender<WsEvent>,
    input_tx: mpsc::Sender<ParseJob>,
    input_rx: RwLock<Option<mpsc::Receiver<ParseJob>>>,

    // Statistics
    stats: RwLock<BatchStats>,
}

impl BatchProcessor {
    /// Create a new batch processor from structured parameter objects
    pub fn new(
        config: super::params::BatchProcessorConfig,
        repos: super::params::BatchProcessorRepositories,
        deps: super::params::BatchProcessorDeps,
    ) -> Self {
        let (input_tx, input_rx) = mpsc::channel(config.batch.channel_buffer);

        Self {
            config: config.batch,
            multi_pass_config: config.multi_pass,
            ai_client: deps.ai_client,
            raw_message_repo: repos.raw_message,
            offer_repo: repos.offer,
            request_repo: repos.request,
            medication_master_repo: repos.medication_master,
            review_queue_repo: repos.review_queue,
            group_repo: repos.group,
            participant_repo: repos.participant,
            audit_log_repo: repos.audit_log,
            match_queue_repo: repos.match_queue,
            ws_tx: deps.ws_tx,
            input_tx,
            input_rx: RwLock::new(Some(input_rx)),
            stats: RwLock::new(BatchStats::default()),
        }
    }

    /// Get sender for submitting messages
    pub fn sender(&self) -> mpsc::Sender<ParseJob> {
        self.input_tx.clone()
    }

    /// Submit a message for batch processing
    pub async fn submit(
        &self,
        message: RawMessage,
    ) -> Result<(), mpsc::error::SendError<ParseJob>> {
        let job = ParseJob::new(message);
        self.input_tx.send(job).await
    }

    /// Get current statistics
    pub async fn stats(&self) -> BatchStats {
        self.stats.read().await.clone()
    }

    /// Run the batch processor loop
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        // Take ownership of receiver
        let mut input_rx = {
            let mut rx_lock = self.input_rx.write().await;
            rx_lock.take().expect("BatchProcessor can only be run once")
        };

        info!(
            batch_size = self.config.batch_size,
            timeout_secs = self.config.batch_timeout.as_secs(),
            "📦 Batch processor started"
        );

        let mut batch: Vec<ParseJob> = Vec::with_capacity(self.config.batch_size);
        let mut ticker = interval(self.config.batch_timeout);

        loop {
            tokio::select! {
                // Shutdown signal
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        // Process remaining batch before exit
                        if !batch.is_empty() {
                            info!(remaining = batch.len(), "Processing remaining batch before shutdown");
                            self.process_batch(&batch).await;
                        }
                        info!("👋 Batch processor stopped gracefully");
                        break;
                    }
                }

                // New message received
                Some(job) = input_rx.recv() => {
                    {
                        let mut stats = self.stats.write().await;
                        stats.messages_received += 1;
                    }

                    batch.push(job);

                    // Process when batch is full
                    if batch.len() >= self.config.batch_size {
                        self.process_batch(&batch).await;
                        batch.clear();
                        ticker.reset();
                    }
                }

                // Timeout - process partial batch
                _ = ticker.tick() => {
                    if !batch.is_empty() {
                        self.process_batch(&batch).await;
                        batch.clear();
                    }
                }
            }
        }
    }

    /// Process a batch of messages
    async fn process_batch(&self, batch: &[ParseJob]) {
        let batch_size = batch.len();
        info!(batch_size, "📦 Processing message batch");

        // Extract raw messages
        let messages: Vec<&RawMessage> = batch.iter().map(|j| &j.message).collect();

        // Step 1: Get medication mappings (RAG-Lite)
        let mut all_mappings = Vec::new();
        for msg in &messages {
            if let Ok(mappings) = self
                .medication_master_repo
                .find_relevant(&msg.content, 3)
                .await
            {
                for m in mappings {
                    all_mappings.push(m.to_prompt_context());
                }
            }
        }

        // Deduplicate mappings
        all_mappings.sort();
        all_mappings.dedup();

        // Step 2: Parse with AI (Pass 1 - Strict)
        let results = self
            .parse_batch_with_ai(&messages, &all_mappings, ParsePass::Strict)
            .await;

        // Step 3: Process results
        for (i, result) in results.into_iter().enumerate() {
            let msg = &messages[i];
            self.process_single_result(msg, result).await;
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.batches_processed += 1;
        }

        info!(batch_size, "✅ Batch processing complete");
    }

    /// Parse a batch with AI gateway
    async fn parse_batch_with_ai(
        &self,
        messages: &[&RawMessage],
        mappings: &[String],
        pass: ParsePass,
    ) -> Vec<ParseJobResult> {
        let mut results = Vec::with_capacity(messages.len());

        // For now, parse each message individually
        for msg in messages {
            // Fetch names for AI prompt
            let participant = self
                .participant_repo
                .get_by_id(msg.participant_id)
                .await
                .ok()
                .flatten();
            let group = self.group_repo.get_by_id(msg.group_id).await.ok().flatten();

            let sender_name = participant.as_ref().and_then(|p| p.push_name.clone());
            let group_name = group
                .as_ref()
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Pass medication mappings to parser for better context
            let mapping_slice = if mappings.is_empty() {
                None
            } else {
                Some(mappings)
            };

            let parse_result = self
                .ai_client
                .parse(
                    &msg.content,
                    sender_name.as_deref(),
                    &group_name,
                    msg.reply_to_content.as_deref(),
                    mapping_slice,
                )
                .await;

            match parse_result {
                Ok(items) => {
                    results.push(ParseJobResult::success(msg.id, items, pass));
                }
                Err(e) => {
                    error!(error = %e, msg_id = %msg.id, "AI parsing failed");
                    results.push(ParseJobResult::error(msg.id, e.to_string()));
                }
            }
        }

        results
    }

    /// Process a single parse result
    async fn process_single_result(&self, msg: &RawMessage, result: ParseJobResult) {
        // Handle error
        if let Some(error) = &result.error {
            warn!(msg_id = %msg.id, error = %error, "Parse error");
            let _ = self
                .raw_message_repo
                .mark_processed(msg.id, Some(error))
                .await;
            return;
        }

        // Handle no items
        if result.items.is_empty() {
            info!(msg_id = %msg.id, "No items extracted");
            let _ = self.raw_message_repo.mark_processed(msg.id, None).await;
            return;
        }

        // Calculate average confidence
        let avg_confidence = self.calculate_avg_confidence(&result.items);

        // Check if needs Pass 2 retry
        let final_result = if result.pass == ParsePass::Strict
            && self.multi_pass_config.needs_pass2(avg_confidence)
        {
            info!(
                msg_id = %msg.id,
                avg_confidence,
                threshold = self.multi_pass_config.strict_min_confidence,
                "Low confidence, retrying with Pass 2 (relaxed prompts)"
            );

            // Retry with relaxed prompts
            let pass2_results = self
                .parse_batch_with_ai(&[msg], &[], ParsePass::Relaxed)
                .await;

            if let Some(pass2_result) = pass2_results.into_iter().next() {
                let pass2_confidence = self.calculate_avg_confidence(&pass2_result.items);

                // Use Pass 2 result if it has better confidence or more items
                if pass2_confidence > avg_confidence
                    || (pass2_result.items.len() > result.items.len()
                        && pass2_confidence >= self.multi_pass_config.relaxed_min_confidence)
                {
                    info!(
                        msg_id = %msg.id,
                        pass1_confidence = avg_confidence,
                        pass2_confidence,
                        pass1_items = result.items.len(),
                        pass2_items = pass2_result.items.len(),
                        "Using Pass 2 results (better confidence or more items)"
                    );
                    let mut stats = self.stats.write().await;
                    stats.pass2_retries += 1;
                    pass2_result
                } else {
                    info!(
                        msg_id = %msg.id,
                        pass1_confidence = avg_confidence,
                        pass2_confidence,
                        "Keeping Pass 1 results (Pass 2 not better)"
                    );
                    result
                }
            } else {
                result
            }
        } else {
            result
        };

        let final_confidence = self.calculate_avg_confidence(&final_result.items);

        // Check if needs review queue
        if self.multi_pass_config.needs_review(final_confidence) {
            let review_item = ReviewQueueItem::for_low_confidence(
                msg.id,
                serde_json::to_value(&final_result.items).unwrap_or_default(),
                final_confidence,
                "low_confidence",
            );
            if let Err(e) = self.review_queue_repo.save(&review_item).await {
                error!(error = %e, "Failed to queue for review");
            }
        }

        // Batch generate embeddings for all items
        // IMPORTANT: Strip dosage from medication names before embedding to prevent false positives
        // e.g., "Kozentex 150" and "Gonapure 150" would have similar embeddings due to "150"
        // but they are completely different medications
        let medications_for_embedding: Vec<String> = final_result
            .items
            .iter()
            .map(|i| crate::matching::arabic::normalize_for_matching(&i.medication))
            .collect();
        let embeddings = if !medications_for_embedding.is_empty() {
            match self.ai_client.embed_batch(&medications_for_embedding).await {
                Ok(embs) => embs,
                Err(e) => {
                    warn!(error = %e, "Failed to batch generate embeddings, falling back to None");
                    vec![vec![]; medications_for_embedding.len()]
                }
            }
        } else {
            vec![]
        };

        // Create offers and requests with pre-generated embeddings
        for (item, embedding) in final_result.items.into_iter().zip(embeddings.into_iter()) {
            let emb = if embedding.is_empty() {
                None
            } else {
                Some(embedding)
            };
            self.create_entity_from_item_with_embedding(msg, &item, emb)
                .await;
        }

        // Mark as processed
        let _ = self.raw_message_repo.mark_processed(msg.id, None).await;
    }

    /// Create offer or request from parsed item with pre-generated embedding
    async fn create_entity_from_item_with_embedding(
        &self,
        msg: &RawMessage,
        item: &ParsedItem,
        embedding: Option<Vec<f32>>,
    ) {
        match item.item_type.as_str() {
            "OFFER" | "BOTH" => {
                let offer = Offer {
                    id: uuid::Uuid::new_v4(),
                    raw_message_id: msg.id,
                    medication: item.medication.clone(),
                    medication_raw: item.medication.clone(),
                    quantity: Decimal::from_f64(item.quantity),
                    unit: item.unit.clone(),
                    price: Decimal::from_f64(item.price),
                    currency: Some("EGP".to_string()),
                    expiry_date: None,
                    batch_number: None,
                    participant_id: msg.participant_id,
                    group_id: msg.group_id,
                    notes: item.notes.clone(),
                    status: ItemStatus::Active,
                    content_embedding: embedding.clone().map(PgVector::from),
                    urgency_level: UrgencyLevel::from_bool(item.urgent),
                    expiry_info: item.expiry.clone(),
                    ai_confidence: item.ai_confidence,
                    master_medication_id: None,
                    medication_curated: false,
                    confirmed_match_count: 0,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                if let Err(e) = self.offer_repo.save(&offer).await {
                    error!(error = %e, offer_id = %offer.id, "Failed to save offer");
                } else {
                    info!(offer_id = %offer.id, medication = %offer.medication, "✅ Offer created");
                    let _ = self.ws_tx.send(WsEvent::NewOffer(offer.clone()));

                    // Trigger re-matching: Since workers are request-centric,
                    // we enqueue active requests to match against this new offer.
                    if let Ok(active_requests) = self.request_repo.get_active(100, 0).await {
                        for r in active_requests {
                            let _ = self.match_queue_repo.enqueue(r.id, 0).await;
                        }
                    }

                    // Audit log
                    let audit =
                        AuditLog::system(AuditAction::OfferCreated, EntityType::Offer, offer.id);
                    let _ = self.audit_log_repo.save(&audit).await;

                    let mut stats = self.stats.write().await;
                    stats.items_extracted += 1;
                }
            }
            _ => {}
        }

        match item.item_type.as_str() {
            "REQUEST" | "BOTH" => {
                let request = Request {
                    id: uuid::Uuid::new_v4(),
                    raw_message_id: msg.id,
                    medication: item.medication.clone(),
                    medication_raw: item.medication.clone(),
                    quantity: Decimal::from_f64(item.quantity),
                    unit: item.unit.clone(),
                    max_price: Decimal::from_f64(item.max_price),
                    currency: Some("EGP".to_string()),
                    participant_id: msg.participant_id,
                    group_id: msg.group_id,
                    notes: item.notes.clone(),
                    urgency_level: UrgencyLevel::from_bool(item.urgent),
                    expiry_requirement: item.expiry.clone(),
                    ai_confidence: item.ai_confidence,
                    status: ItemStatus::Active,
                    content_embedding: embedding.map(PgVector::from),
                    master_medication_id: None,
                    medication_curated: false,
                    confirmed_match_count: 0,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                if let Err(e) = self.request_repo.save(&request).await {
                    error!(error = %e, request_id = %request.id, "Failed to save request");
                } else {
                    info!(request_id = %request.id, medication = %request.medication, "✅ Request created");
                    let _ = self.ws_tx.send(WsEvent::NewRequest(request.clone()));

                    // Enqueue for matching
                    let _ = self.match_queue_repo.enqueue(request.id, 0).await;

                    // Audit log
                    let audit = AuditLog::system(
                        AuditAction::RequestCreated,
                        EntityType::Request,
                        request.id,
                    );
                    let _ = self.audit_log_repo.save(&audit).await;

                    let mut stats = self.stats.write().await;
                    stats.items_extracted += 1;
                }
            }
            _ => {}
        }
    }

    /// Calculate average AI confidence across items
    fn calculate_avg_confidence(&self, items: &[ParsedItem]) -> f64 {
        if items.is_empty() {
            return 0.0;
        }
        let sum: f64 = items.iter().map(|i| i.ai_confidence).sum();
        sum / items.len() as f64
    }
}
