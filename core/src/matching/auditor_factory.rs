//! Auditor Factory for Multi-Model Configuration
//!
//! Creates and configures AI auditors for different use cases.
//! Supports single-model, multi-model consensus, and fallback configurations.

use std::sync::Arc;

use ai_client::{Client as AIClient, ClientConfig};

use crate::matching::consensus_auditor::{ConsensusAuditor, ConsensusConfig};
use crate::matching::reviewer::AIReviewer;

/// Model configuration for auditing
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Model identifier (e.g., "ai/qwen3-vl:latest")
    pub model: String,
    /// Optional custom base URL (uses default if None)
    pub base_url: Option<String>,
    /// Optional API key override
    pub api_key: Option<String>,
    /// Weight for this model in consensus (1.0 = normal)
    pub weight: f64,
    /// Whether this model is enabled
    pub enabled: bool,
}

impl ModelConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: None,
            api_key: None,
            weight: 1.0,
            enabled: true,
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Auditor type to create
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditorType {
    /// Single model reviewer (fastest, least reliable)
    Single,
    /// Multi-model consensus (slower, more reliable)
    Consensus,
    /// Hybrid: single for low-stakes, consensus for high-stakes
    Hybrid,
}

/// Factory configuration
#[derive(Debug, Clone)]
pub struct AuditorFactoryConfig {
    /// Default auditor type
    pub default_type: AuditorType,
    /// Models to use for auditing
    pub models: Vec<ModelConfig>,
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    /// Score threshold for hybrid mode (above this uses consensus)
    pub hybrid_threshold: f64,
}

impl Default for AuditorFactoryConfig {
    fn default() -> Self {
        Self {
            default_type: AuditorType::Single,
            models: vec![ModelConfig::new("ai/qwen3-vl:latest")],
            consensus: ConsensusConfig::default(),
            hybrid_threshold: 0.70,
        }
    }
}

impl AuditorFactoryConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        let default_type = match std::env::var("AUDITOR_TYPE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "consensus" => AuditorType::Consensus,
            "hybrid" => AuditorType::Hybrid,
            _ => AuditorType::Single,
        };

        let models: Vec<ModelConfig> = std::env::var("AUDITOR_MODELS")
            .unwrap_or_else(|_| "ai/qwen3-vl:latest".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(ModelConfig::new)
            .collect();

        let hybrid_threshold = std::env::var("AUDITOR_HYBRID_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.70);

        Self {
            default_type,
            models,
            consensus: ConsensusConfig::from_env(),
            hybrid_threshold,
        }
    }

    /// Configuration for production (consensus with multiple models)
    pub fn production() -> Self {
        Self {
            default_type: AuditorType::Consensus,
            models: vec![
                ModelConfig::new("ai/qwen3-vl:latest"),
                ModelConfig::new("ai/gemma3:latest"),
                ModelConfig::new("ai/ministral3:latest"),
            ],
            consensus: ConsensusConfig::default(),
            hybrid_threshold: 0.70,
        }
    }

    /// Configuration for development (single model, fast)
    pub fn development() -> Self {
        Self {
            default_type: AuditorType::Single,
            models: vec![ModelConfig::new("ai/qwen3-vl:latest")],
            consensus: ConsensusConfig::default(),
            hybrid_threshold: 0.70,
        }
    }
}

/// Factory for creating auditors
pub struct AuditorFactory {
    config: AuditorFactoryConfig,
    base_config: ClientConfig,
}

impl AuditorFactory {
    /// Create a new factory with the given configuration
    pub fn new(config: AuditorFactoryConfig) -> Self {
        Self {
            config,
            base_config: ClientConfig::from_env(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(AuditorFactoryConfig::from_env())
    }

    /// Create a single-model reviewer
    pub fn create_single_reviewer(&self) -> AIReviewer {
        let model_config = self
            .config
            .models
            .iter()
            .find(|m| m.enabled)
            .cloned()
            .unwrap_or_else(|| ModelConfig::new("ai/qwen3-vl:latest"));

        let client = self.create_client(&model_config);
        AIReviewer::new(Arc::new(client))
    }

    /// Create a consensus auditor with all configured models
    pub fn create_consensus_auditor(&self) -> ConsensusAuditor {
        let clients: Vec<(String, Arc<AIClient>)> = self
            .config
            .models
            .iter()
            .filter(|m| m.enabled)
            .map(|m| {
                let client = self.create_client(m);
                (m.model.clone(), Arc::new(client))
            })
            .collect();

        ConsensusAuditor::new(self.config.consensus.clone(), clients)
    }

    /// Create a hybrid auditor (returns both single and consensus)
    pub fn create_hybrid_auditors(&self) -> (AIReviewer, ConsensusAuditor) {
        (
            self.create_single_reviewer(),
            self.create_consensus_auditor(),
        )
    }

    /// Create an AI client for a model configuration
    fn create_client(&self, model_config: &ModelConfig) -> AIClient {
        let mut config = self.base_config.clone();
        config.model = model_config.model.clone();

        if let Some(ref url) = model_config.base_url {
            config.base_url = url.clone();
        }

        if let Some(ref key) = model_config.api_key {
            config.api_key = Some(key.clone());
        }

        AIClient::new(config)
    }

    /// Get the default auditor type
    pub fn default_type(&self) -> AuditorType {
        self.config.default_type
    }

    /// Get the hybrid threshold
    pub fn hybrid_threshold(&self) -> f64 {
        self.config.hybrid_threshold
    }

    /// Get the number of enabled models
    pub fn enabled_model_count(&self) -> usize {
        self.config.models.iter().filter(|m| m.enabled).count()
    }

    /// Get current configuration
    pub fn config(&self) -> &AuditorFactoryConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: AuditorFactoryConfig) {
        self.config = config;
    }
}

/// Hybrid auditor that uses single or consensus based on score
pub struct HybridAuditor {
    single: AIReviewer,
    consensus: ConsensusAuditor,
    threshold: f64,
}

impl HybridAuditor {
    /// Create a new hybrid auditor
    pub fn new(single: AIReviewer, consensus: ConsensusAuditor, threshold: f64) -> Self {
        Self {
            single,
            consensus,
            threshold,
        }
    }

    /// Create from factory
    pub fn from_factory(factory: &AuditorFactory) -> Self {
        let (single, consensus) = factory.create_hybrid_auditors();
        Self::new(single, consensus, factory.hybrid_threshold())
    }

    /// Audit a match, using consensus for high-stakes decisions
    pub async fn audit_match(
        &self,
        offer: &crate::domain::Offer,
        request: &crate::domain::Request,
        score: f64,
        reasoning: &str,
    ) -> crate::matching::consensus_auditor::ConsensusResult {
        if score >= self.threshold {
            // High score - use consensus for verification
            self.consensus
                .audit_match(offer, request, score, reasoning)
                .await
        } else {
            // Low score - use single model (faster)
            match self
                .single
                .audit_match(offer, request, score, reasoning)
                .await
            {
                Ok(result) => crate::matching::consensus_auditor::ConsensusResult {
                    status: result.status,
                    confidence: result.confidence as f64,
                    agreement_ratio: 1.0,
                    agreeing_models: 1,
                    total_models: 1,
                    explanation: result.explanation,
                    consensus_reached: true,
                    model_details: vec![],
                },
                Err(e) => crate::matching::consensus_auditor::ConsensusResult {
                    status: crate::matching::reviewer::ReviewStatus::Flagged,
                    confidence: 0.0,
                    agreement_ratio: 0.0,
                    agreeing_models: 0,
                    total_models: 1,
                    explanation: format!("Single model failed: {}", e),
                    consensus_reached: false,
                    model_details: vec![],
                },
            }
        }
    }

    /// Get the threshold for consensus usage
    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Set the threshold for consensus usage
    pub fn set_threshold(&mut self, threshold: f64) {
        self.threshold = threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_builder() {
        let config = ModelConfig::new("test-model")
            .with_base_url("http://localhost:8080")
            .with_api_key("secret")
            .with_weight(1.5);

        assert_eq!(config.model, "test-model");
        assert_eq!(config.base_url, Some("http://localhost:8080".to_string()));
        assert_eq!(config.api_key, Some("secret".to_string()));
        assert!((config.weight - 1.5).abs() < 0.01);
        assert!(config.enabled);
    }

    #[test]
    fn test_model_config_disabled() {
        let config = ModelConfig::new("test-model").disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_default_factory_config() {
        let config = AuditorFactoryConfig::default();
        assert_eq!(config.default_type, AuditorType::Single);
        assert_eq!(config.models.len(), 1);
        assert!((config.hybrid_threshold - 0.70).abs() < 0.01);
    }

    #[test]
    fn test_production_config() {
        let config = AuditorFactoryConfig::production();
        assert_eq!(config.default_type, AuditorType::Consensus);
        assert_eq!(config.models.len(), 3);
    }

    #[test]
    fn test_development_config() {
        let config = AuditorFactoryConfig::development();
        assert_eq!(config.default_type, AuditorType::Single);
        assert_eq!(config.models.len(), 1);
    }

    #[test]
    fn test_factory_enabled_model_count() {
        let mut config = AuditorFactoryConfig::default();
        config.models = vec![
            ModelConfig::new("model1"),
            ModelConfig::new("model2").disabled(),
            ModelConfig::new("model3"),
        ];

        let factory = AuditorFactory::new(config);
        assert_eq!(factory.enabled_model_count(), 2);
    }

    #[test]
    fn test_auditor_type_equality() {
        assert_eq!(AuditorType::Single, AuditorType::Single);
        assert_ne!(AuditorType::Single, AuditorType::Consensus);
        assert_ne!(AuditorType::Consensus, AuditorType::Hybrid);
    }
}
