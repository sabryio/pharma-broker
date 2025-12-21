//! Batch processor implementation
//!
//! Accumulates messages and processes them in batches for efficiency.
//! Ported from legacy/parsing/processor.go

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
    AuditLogRepository, MatchQueueRepository, MedicationMappingRepository, OfferRepository,
    RawMessageRepository, RequestRepository, ReviewQueueRepository,
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
    medication_mapping_repo: Arc<dyn MedicationMappingRepository>,
    review_queue_repo: Arc<dyn ReviewQueueRepository>,
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
    /// Create a new batch processor
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: BatchConfig,
        multi_pass_config: MultiPassConfig,
        ai_client: Arc<PharmaParser>,
        raw_message_repo: Arc<dyn RawMessageRepository>,
        offer_repo: Arc<dyn OfferRepository>,
        request_repo: Arc<dyn RequestRepository>,
        medication_mapping_repo: Arc<dyn MedicationMappingRepository>,
        review_queue_repo: Arc<dyn ReviewQueueRepository>,
        audit_log_repo: Arc<dyn AuditLogRepository>,
        match_queue_repo: Arc<dyn MatchQueueRepository>,
        ws_tx: broadcast::Sender<WsEvent>,
    ) -> Self {
        let (input_tx, input_rx) = mpsc::channel(config.channel_buffer);

        Self {
            config,
            multi_pass_config,
            ai_client,
            raw_message_repo,
            offer_repo,
            request_repo,
            medication_mapping_repo,
            review_queue_repo,
            audit_log_repo,
            match_queue_repo,
            ws_tx,
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
                .medication_mapping_repo
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
                    Some(&msg.sender_name),
                    Some(&msg.group_name),
                    msg.reply_to_content.as_deref(),
                    mapping_slice,
                )
                .await;

            match parse_result {
                Ok(items) => {
                    results.push(ParseJobResult::success(msg.id.clone(), items, pass));
                }
                Err(e) => {
                    error!(error = %e, msg_id = %msg.id, "AI parsing failed");
                    results.push(ParseJobResult::error(msg.id.clone(), e.to_string()));
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
                .mark_processed(&msg.id, Some(error))
                .await;
            return;
        }

        // Handle no items
        if result.items.is_empty() {
            info!(msg_id = %msg.id, "No items extracted");
            let _ = self.raw_message_repo.mark_processed(&msg.id, None).await;
            return;
        }

        // Calculate average confidence
        let avg_confidence = self.calculate_avg_confidence(&result.items);

        // Check if needs Pass 2 retry
        if result.pass == ParsePass::Strict && self.multi_pass_config.needs_pass2(avg_confidence) {
            info!(
                msg_id = %msg.id,
                avg_confidence,
                threshold = self.multi_pass_config.strict_min_confidence,
                "Low confidence, would retry with Pass 2 (not implemented)"
            );
            // TODO: Implement pass 2 with relaxed prompts
            let mut stats = self.stats.write().await;
            stats.pass2_retries += 1;
        }

        // Check if needs review queue
        if self.multi_pass_config.needs_review(avg_confidence) {
            let review_item = ReviewQueueItem::for_low_confidence(
                msg.id.clone(),
                serde_json::to_value(&result.items).unwrap_or_default(),
                avg_confidence,
            );
            if let Err(e) = self.review_queue_repo.save(&review_item).await {
                error!(error = %e, "Failed to queue for review");
            }
        }

        // Batch generate embeddings for all items
        let medications: Vec<String> = result.items.iter().map(|i| i.medication.clone()).collect();
        let embeddings = if !medications.is_empty() {
            match self.ai_client.embed_batch(&medications).await {
                Ok(embs) => embs,
                Err(e) => {
                    warn!(error = %e, "Failed to batch generate embeddings, falling back to None");
                    vec![vec![]; medications.len()]
                }
            }
        } else {
            vec![]
        };

        // Create offers and requests with pre-generated embeddings
        for (item, embedding) in result.items.into_iter().zip(embeddings.into_iter()) {
            let emb = if embedding.is_empty() {
                None
            } else {
                Some(embedding)
            };
            self.create_entity_from_item_with_embedding(msg, &item, emb)
                .await;
        }

        // Mark as processed
        let _ = self.raw_message_repo.mark_processed(&msg.id, None).await;
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
                    id: uuid::Uuid::new_v4().to_string(),
                    raw_message_id: msg.id.clone(),
                    medication: item.medication.clone(),
                    medication_raw: item.medication.clone(),
                    quantity: item.quantity,
                    unit: item.unit.clone(),
                    price: item.price,
                    currency: Some("EGP".to_string()),
                    expiry_date: None,
                    batch_number: None,
                    source_phone: msg.sender_phone.clone(),
                    source_name: msg.sender_name.clone(),
                    source_group: msg.group_jid.clone(),
                    group_name: msg.group_name.clone(),
                    notes: item.notes.clone(),
                    raw_message: msg.content.clone(),
                    status: ItemStatus::Active,
                    content_embedding: embedding.clone(),
                    urgent: item.urgent,
                    urgency_level: UrgencyLevel::from_bool(item.urgent),
                    expiry_info: item.expiry.clone(),
                    ai_confidence: item.ai_confidence,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                if let Err(e) = self.offer_repo.save(&offer).await {
                    error!(error = %e, offer_id = %offer.id, "Failed to save offer");
                } else {
                    info!(offer_id = %offer.id, medication = %offer.medication, "✅ Offer created");
                    let _ = self.ws_tx.send(WsEvent::NewOffer(offer.clone()));

                    // Enqueue for matching
                    let _ = self.match_queue_repo.enqueue(&offer.id, 0).await;

                    // Audit log
                    let audit = AuditLog::system(
                        AuditAction::OfferCreated,
                        EntityType::Offer,
                        offer.id.clone(),
                    );
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
                    id: uuid::Uuid::new_v4().to_string(),
                    raw_message_id: msg.id.clone(),
                    medication: item.medication.clone(),
                    medication_raw: item.medication.clone(),
                    quantity: item.quantity,
                    unit: item.unit.clone(),
                    max_price: item.max_price,
                    currency: Some("EGP".to_string()),
                    source_phone: msg.sender_phone.clone(),
                    source_name: msg.sender_name.clone(),
                    source_group: msg.group_jid.clone(),
                    group_name: msg.group_name.clone(),
                    notes: item.notes.clone(),
                    raw_message: msg.content.clone(),
                    urgent: item.urgent,
                    urgency_level: UrgencyLevel::from_bool(item.urgent),
                    expiry_requirement: item.expiry.clone(),
                    ai_confidence: item.ai_confidence,
                    status: ItemStatus::Active,
                    content_embedding: embedding,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                if let Err(e) = self.request_repo.save(&request).await {
                    error!(error = %e, request_id = %request.id, "Failed to save request");
                } else {
                    info!(request_id = %request.id, medication = %request.medication, "✅ Request created");
                    let _ = self.ws_tx.send(WsEvent::NewRequest(request.clone()));

                    // Enqueue for matching
                    let _ = self.match_queue_repo.enqueue(&request.id, 0).await;

                    // Audit log
                    let audit = AuditLog::system(
                        AuditAction::RequestCreated,
                        EntityType::Request,
                        request.id.clone(),
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
