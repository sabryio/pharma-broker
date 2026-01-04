//! Persistent Audit Recorder
//!
//! Wraps the in-memory AuditRecorder with database persistence.
//! Implements buffer flush mechanism with periodic and capacity-based triggers.
//!
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::matching::audit_recorder::{AuditRecorderConfig, MatchAuditRecord};
use crate::repository::{MatchAuditRecordModel, MatchAuditRecordRepository};

/// Configuration for persistent audit recorder
#[derive(Debug, Clone)]
pub struct PersistentAuditConfig {
    /// Base audit recorder config
    pub base: AuditRecorderConfig,
    /// Flush interval in seconds
    pub flush_interval_secs: u64,
    /// Maximum buffer size before forced flush
    pub max_buffer_size: usize,
    /// Number of retry attempts for failed flushes
    pub max_retry_attempts: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for PersistentAuditConfig {
    fn default() -> Self {
        Self {
            base: AuditRecorderConfig::default(),
            flush_interval_secs: 30,
            max_buffer_size: 100,
            max_retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl PersistentAuditConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            base: AuditRecorderConfig::from_env(),
            flush_interval_secs: std::env::var("AUDIT_FLUSH_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            max_buffer_size: std::env::var("AUDIT_MAX_BUFFER_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            max_retry_attempts: std::env::var("AUDIT_MAX_RETRY_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            retry_delay_ms: std::env::var("AUDIT_RETRY_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
        }
    }
}

/// Statistics for persistent audit recorder
#[derive(Debug, Default)]
pub struct PersistentAuditStats {
    pub records_created: AtomicU64,
    pub records_persisted: AtomicU64,
    pub records_failed: AtomicU64,
    pub flush_count: AtomicU64,
    pub flush_errors: AtomicU64,
    pub retry_count: AtomicU64,
}

impl PersistentAuditStats {
    pub fn snapshot(&self) -> PersistentAuditStatsSnapshot {
        PersistentAuditStatsSnapshot {
            records_created: self.records_created.load(Ordering::Relaxed),
            records_persisted: self.records_persisted.load(Ordering::Relaxed),
            records_failed: self.records_failed.load(Ordering::Relaxed),
            flush_count: self.flush_count.load(Ordering::Relaxed),
            flush_errors: self.flush_errors.load(Ordering::Relaxed),
            retry_count: self.retry_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PersistentAuditStatsSnapshot {
    pub records_created: u64,
    pub records_persisted: u64,
    pub records_failed: u64,
    pub flush_count: u64,
    pub flush_errors: u64,
    pub retry_count: u64,
}

/// Persistent audit recorder with database backing
pub struct PersistentAuditRecorder<R: MatchAuditRecordRepository> {
    config: PersistentAuditConfig,
    buffer: Arc<RwLock<VecDeque<MatchAuditRecord>>>,
    repository: Arc<R>,
    stats: Arc<PersistentAuditStats>,
    shutdown: Arc<AtomicBool>,
    flush_tx: Option<mpsc::Sender<()>>,
}

impl<R: MatchAuditRecordRepository + 'static> PersistentAuditRecorder<R> {
    /// Create a new persistent audit recorder
    pub fn new(config: PersistentAuditConfig, repository: Arc<R>) -> Self {
        Self {
            config,
            buffer: Arc::new(RwLock::new(VecDeque::new())),
            repository,
            stats: Arc::new(PersistentAuditStats::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            flush_tx: None,
        }
    }

    /// Start the background flush task
    pub fn start_flush_task(&mut self) -> tokio::task::JoinHandle<()> {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        self.flush_tx = Some(tx);

        let buffer = self.buffer.clone();
        let repository = self.repository.clone();
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_secs(config.flush_interval_secs));

            loop {
                tokio::select! {
                    _ = flush_interval.tick() => {
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        Self::flush_buffer_internal(&buffer, &repository, &stats, &config).await;
                    }
                    _ = rx.recv() => {
                        // Manual flush triggered
                        Self::flush_buffer_internal(&buffer, &repository, &stats, &config).await;
                    }
                }
            }

            // Final flush on shutdown
            info!("Performing final flush before shutdown");
            Self::flush_buffer_internal(&buffer, &repository, &stats, &config).await;
        })
    }

    /// Record a match audit
    pub async fn record(&self, record: MatchAuditRecord) -> bool {
        if !self.config.base.enabled {
            return false;
        }

        // Check score threshold
        if let Some(min_score) = self.config.base.min_score_threshold
            && record.final_score < min_score
        {
            return false;
        }

        // Check sample rate
        if self.config.base.sample_rate < 1.0 {
            use rand::Rng;
            if rand::rng().random::<f64>() >= self.config.base.sample_rate {
                return false;
            }
        }

        let mut buffer = self.buffer.write().await;

        // Check if buffer is at capacity - trigger flush
        if buffer.len() >= self.config.max_buffer_size {
            // Drop the write lock before flushing
            drop(buffer);

            // Trigger flush
            if let Some(tx) = &self.flush_tx {
                let _ = tx.try_send(());
            }

            // Re-acquire lock
            buffer = self.buffer.write().await;
        }

        buffer.push_back(record);
        self.stats.records_created.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Get recent records from buffer
    pub async fn get_recent(&self, limit: usize) -> Vec<MatchAuditRecord> {
        let buffer = self.buffer.read().await;
        buffer.iter().rev().take(limit).cloned().collect()
    }

    /// Get record by match ID from buffer
    pub async fn get_by_match_id_from_buffer(&self, match_id: Uuid) -> Option<MatchAuditRecord> {
        let buffer = self.buffer.read().await;
        buffer.iter().find(|r| r.match_id == match_id).cloned()
    }

    /// Get records by session ID from buffer
    pub async fn get_by_session_from_buffer(&self, session_id: &str) -> Vec<MatchAuditRecord> {
        let buffer = self.buffer.read().await;
        buffer
            .iter()
            .filter(|r| r.session_id.as_deref() == Some(session_id))
            .cloned()
            .collect()
    }

    /// List audit records from both buffer and database
    pub async fn list_audit_records(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MatchAuditRecord>, pharma_db::Error> {
        // Get from buffer first
        let buffer_records: Vec<MatchAuditRecord> = {
            let buffer = self.buffer.read().await;
            buffer.iter().rev().cloned().collect()
        };

        // If we have enough from buffer, return those
        if buffer_records.len() >= limit + offset {
            return Ok(buffer_records
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect());
        }

        // Query database for additional records
        let db_records = self.repository.list_recent(limit, offset).await?;

        // Convert DB models to domain records and merge
        let mut result: Vec<MatchAuditRecord> = buffer_records
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        // Add DB records that aren't already in buffer
        let buffer_ids: std::collections::HashSet<Uuid> =
            result.iter().map(|r| r.match_id).collect();

        for db_record in db_records {
            if !buffer_ids.contains(&db_record.match_id) && result.len() < limit {
                result.push(Self::model_to_record(db_record));
            }
        }

        Ok(result)
    }

    /// Get record by match ID from both buffer and database
    pub async fn get_by_match_id(
        &self,
        match_id: Uuid,
    ) -> Result<Option<MatchAuditRecord>, pharma_db::Error> {
        // Check buffer first
        if let Some(record) = self.get_by_match_id_from_buffer(match_id).await {
            return Ok(Some(record));
        }

        // Query database
        let db_record = self.repository.get_by_match_id(match_id).await?;
        Ok(db_record.map(Self::model_to_record))
    }

    /// Get records by session ID from both buffer and database
    pub async fn get_by_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<MatchAuditRecord>, pharma_db::Error> {
        // Get from buffer
        let mut records = self.get_by_session_from_buffer(session_id).await;

        // Query database
        let db_records = self.repository.get_by_session(session_id).await?;

        // Merge, avoiding duplicates
        let buffer_ids: std::collections::HashSet<Uuid> =
            records.iter().map(|r| r.match_id).collect();

        for db_record in db_records {
            if !buffer_ids.contains(&db_record.match_id) {
                records.push(Self::model_to_record(db_record));
            }
        }

        Ok(records)
    }

    /// Manually trigger a flush
    pub async fn flush(&self) {
        if let Some(tx) = &self.flush_tx {
            let _ = tx.send(()).await;
        }
    }

    /// Shutdown the recorder
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Get statistics
    pub fn stats(&self) -> PersistentAuditStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get buffer size
    pub async fn buffer_len(&self) -> usize {
        self.buffer.read().await.len()
    }

    /// Internal flush implementation
    async fn flush_buffer_internal(
        buffer: &Arc<RwLock<VecDeque<MatchAuditRecord>>>,
        repository: &Arc<R>,
        stats: &Arc<PersistentAuditStats>,
        config: &PersistentAuditConfig,
    ) {
        // Drain buffer
        let records: Vec<MatchAuditRecord> = {
            let mut buf = buffer.write().await;
            buf.drain(..).collect()
        };

        if records.is_empty() {
            return;
        }

        debug!("Flushing {} audit records to database", records.len());
        stats.flush_count.fetch_add(1, Ordering::Relaxed);

        let mut failed_records = Vec::new();

        for record in records {
            let model = Self::record_to_model(&record);

            let mut attempts = 0;
            let mut success = false;

            while attempts < config.max_retry_attempts && !success {
                match repository.insert(&model).await {
                    Ok(_) => {
                        stats.records_persisted.fetch_add(1, Ordering::Relaxed);
                        success = true;
                    }
                    Err(e) => {
                        attempts += 1;
                        stats.retry_count.fetch_add(1, Ordering::Relaxed);
                        warn!(
                            "Failed to persist audit record {} (attempt {}/{}): {}",
                            record.id, attempts, config.max_retry_attempts, e
                        );

                        if attempts < config.max_retry_attempts {
                            tokio::time::sleep(Duration::from_millis(config.retry_delay_ms)).await;
                        }
                    }
                }
            }

            if !success {
                error!(
                    "Failed to persist audit record {} after {} attempts",
                    record.id, config.max_retry_attempts
                );
                stats.records_failed.fetch_add(1, Ordering::Relaxed);
                stats.flush_errors.fetch_add(1, Ordering::Relaxed);
                failed_records.push(record);
            }
        }

        // Re-add failed records to buffer for retry
        if !failed_records.is_empty() {
            let mut buf = buffer.write().await;
            for record in failed_records {
                buf.push_front(record);
            }
        }
    }

    /// Convert domain record to database model
    fn record_to_model(record: &MatchAuditRecord) -> MatchAuditRecordModel {
        MatchAuditRecordModel {
            id: record.id,
            match_id: record.match_id,
            offer_id: record.offer_id,
            request_id: record.request_id,
            pipeline_version: record.pipeline_version.clone(),
            offer_snapshot: record.offer_snapshot.clone(),
            request_snapshot: record.request_snapshot.clone(),
            weights_snapshot: record.weights_snapshot.clone(),
            config_snapshot: record.config_snapshot.clone(),
            score_breakdown: record.score_breakdown.clone(),
            final_score: record.final_score,
            pipeline_stages: serde_json::to_value(&record.pipeline_stages).unwrap_or_default(),
            ai_involved: record.ai_involved,
            ai_model: record.ai_record.as_ref().map(|r| r.model.clone()),
            ai_response: record.ai_record.as_ref().map(|r| r.response.clone()),
            ai_latency_ms: record.ai_record.as_ref().map(|r| r.latency_ms as i32),
            resolution_stage: record.resolution_stage.clone(),
            resolution_details: record
                .resolution_details
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok()),
            total_latency_ms: record.total_latency_ms as i32,
            created_at: record.created_at,
            review_status: record.review_status.clone(),
            reviewed_by: record.reviewed_by,
            reviewed_at: record.reviewed_at,
            review_notes: record.review_notes.clone(),
            session_id: record.session_id.clone(),
            client_metadata: record
                .client_metadata
                .as_ref()
                .and_then(|m| serde_json::to_value(m).ok()),
        }
    }

    /// Convert database model to domain record
    fn model_to_record(model: MatchAuditRecordModel) -> MatchAuditRecord {
        use crate::matching::audit_recorder::{
            AIInvolvementRecord, ClientMetadata, ResolutionDetails,
        };

        let ai_record = if model.ai_involved {
            model.ai_model.map(|model_name| AIInvolvementRecord {
                model: model_name,
                prompt_tokens: None,
                completion_tokens: None,
                latency_ms: model.ai_latency_ms.unwrap_or(0) as u64,
                response: model.ai_response.unwrap_or(serde_json::Value::Null),
            })
        } else {
            None
        };

        let resolution_details: Option<ResolutionDetails> = model
            .resolution_details
            .and_then(|v| serde_json::from_value(v).ok());

        let client_metadata: Option<ClientMetadata> = model
            .client_metadata
            .and_then(|v| serde_json::from_value(v).ok());

        let pipeline_stages = serde_json::from_value(model.pipeline_stages).unwrap_or_default();

        MatchAuditRecord {
            id: model.id,
            match_id: model.match_id,
            offer_id: model.offer_id,
            request_id: model.request_id,
            pipeline_version: model.pipeline_version,
            offer_snapshot: model.offer_snapshot,
            request_snapshot: model.request_snapshot,
            weights_snapshot: model.weights_snapshot,
            config_snapshot: model.config_snapshot,
            score_breakdown: model.score_breakdown,
            final_score: model.final_score,
            pipeline_stages,
            ai_involved: model.ai_involved,
            ai_record,
            resolution_stage: model.resolution_stage,
            resolution_details,
            total_latency_ms: model.total_latency_ms as u64,
            created_at: model.created_at,
            review_status: model.review_status,
            reviewed_by: model.reviewed_by,
            reviewed_at: model.reviewed_at,
            review_notes: model.review_notes,
            session_id: model.session_id,
            client_metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests are in the property test file: tests/persistence_properties.rs
}
