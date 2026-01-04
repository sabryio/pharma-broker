//! Parameter structs for parsing module
//!
//! These structs consolidate multiple function arguments into descriptive,
//! self-documenting parameter objects following Rust best practices.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::ai::PharmaParser;
use crate::repository::{
    AuditLogRepository, GroupRepository, MatchQueueRepository, MedicationMasterRepository,
    OfferRepository, ParticipantRepository, RawMessageRepository, RequestRepository,
    ReviewQueueRepository,
};
use crate::ws::WsEvent;

use super::config::{BatchConfig, MultiPassConfig};

// =============================================================================
// BatchProcessor Configuration
// =============================================================================

/// Configuration for the BatchProcessor
///
/// Groups all configuration-related parameters together.
#[derive(Clone, Default)]
pub struct BatchProcessorConfig {
    /// Batch processing configuration (size, timeout, etc.)
    pub batch: BatchConfig,
    /// Multi-pass parsing configuration
    pub multi_pass: MultiPassConfig,
}

impl BatchProcessorConfig {
    /// Create a new configuration
    pub fn new(batch: BatchConfig, multi_pass: MultiPassConfig) -> Self {
        Self { batch, multi_pass }
    }

    /// Create with default multi-pass config
    pub fn with_batch(batch: BatchConfig) -> Self {
        Self {
            batch,
            multi_pass: MultiPassConfig::default(),
        }
    }
}

// =============================================================================
// BatchProcessor Repositories
// =============================================================================

/// Repository dependencies for the BatchProcessor
///
/// Groups all repository dependencies together for cleaner constructor signatures.
pub struct BatchProcessorRepositories {
    /// Raw message repository for reading/updating messages
    pub raw_message: Arc<dyn RawMessageRepository>,
    /// Offer repository for creating offers
    pub offer: Arc<dyn OfferRepository>,
    /// Request repository for creating requests
    pub request: Arc<dyn RequestRepository>,
    /// Medication mapping repository for RAG context
    pub medication_master: Arc<dyn MedicationMasterRepository>,
    /// Review queue repository for low-confidence items
    pub review_queue: Arc<dyn ReviewQueueRepository>,
    /// Group repository for reading group details
    pub group: Arc<dyn GroupRepository>,
    /// Participant repository for reading participant details
    pub participant: Arc<dyn ParticipantRepository>,
    /// Audit log repository for tracking actions
    pub audit_log: Arc<dyn AuditLogRepository>,
    /// Match queue repository for enqueueing new items
    pub match_queue: Arc<dyn MatchQueueRepository>,
}

impl BatchProcessorRepositories {
    /// Create a new repository bundle
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        raw_message: Arc<dyn RawMessageRepository>,
        offer: Arc<dyn OfferRepository>,
        request: Arc<dyn RequestRepository>,
        medication_master: Arc<dyn MedicationMasterRepository>,
        review_queue: Arc<dyn ReviewQueueRepository>,
        group: Arc<dyn GroupRepository>,
        participant: Arc<dyn ParticipantRepository>,
        audit_log: Arc<dyn AuditLogRepository>,
        match_queue: Arc<dyn MatchQueueRepository>,
    ) -> Self {
        Self {
            raw_message,
            offer,
            request,
            medication_master,
            review_queue,
            group,
            participant,
            audit_log,
            match_queue,
        }
    }
}

// =============================================================================
// BatchProcessor External Dependencies
// =============================================================================

/// External dependencies for the BatchProcessor
///
/// Groups non-repository dependencies together.
pub struct BatchProcessorDeps {
    /// AI client for parsing messages
    pub ai_client: Arc<PharmaParser>,
    /// WebSocket broadcast sender for real-time updates
    pub ws_tx: broadcast::Sender<WsEvent>,
}

impl BatchProcessorDeps {
    /// Create new external dependencies
    pub fn new(ai_client: Arc<PharmaParser>, ws_tx: broadcast::Sender<WsEvent>) -> Self {
        Self { ai_client, ws_tx }
    }
}

// =============================================================================
// Builder Pattern for BatchProcessor
// =============================================================================

/// Builder for constructing a BatchProcessor with a fluent API
pub struct BatchProcessorBuilder {
    config: Option<BatchProcessorConfig>,
    repos: Option<BatchProcessorRepositories>,
    deps: Option<BatchProcessorDeps>,
}

impl BatchProcessorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: None,
            repos: None,
            deps: None,
        }
    }

    /// Set the configuration
    pub fn config(mut self, config: BatchProcessorConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the batch config only (uses default multi-pass)
    pub fn batch_config(mut self, batch: BatchConfig) -> Self {
        self.config = Some(BatchProcessorConfig::with_batch(batch));
        self
    }

    /// Set the repositories
    pub fn repositories(mut self, repos: BatchProcessorRepositories) -> Self {
        self.repos = Some(repos);
        self
    }

    /// Set the external dependencies
    pub fn dependencies(mut self, deps: BatchProcessorDeps) -> Self {
        self.deps = Some(deps);
        self
    }

    /// Build the BatchProcessor
    ///
    /// # Panics
    /// Panics if required fields are not set.
    pub fn build(self) -> super::processor::BatchProcessor {
        let config = self.config.unwrap_or_default();
        let repos = self.repos.expect("repositories are required");
        let deps = self.deps.expect("dependencies are required");

        super::processor::BatchProcessor::new(config, repos, deps)
    }

    /// Try to build the BatchProcessor, returning an error if fields are missing
    pub fn try_build(self) -> Result<super::processor::BatchProcessor, &'static str> {
        let config = self.config.unwrap_or_default();
        let repos = self.repos.ok_or("repositories are required")?;
        let deps = self.deps.ok_or("dependencies are required")?;

        Ok(super::processor::BatchProcessor::new(config, repos, deps))
    }
}

impl Default for BatchProcessorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_processor_config_default() {
        let config = BatchProcessorConfig::default();
        assert!(config.batch.batch_size > 0);
    }

    #[test]
    fn test_batch_processor_config_with_batch() {
        let batch = BatchConfig {
            batch_size: 50,
            ..Default::default()
        };
        let config = BatchProcessorConfig::with_batch(batch);
        assert_eq!(config.batch.batch_size, 50);
    }
}
