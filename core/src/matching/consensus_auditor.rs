//! Consensus Auditor for Multi-Model Match Verification
//!
//! Uses multiple AI models to audit matches and requires agreement
//! for high-confidence decisions. This reduces false positives by
//! ensuring multiple independent models agree on the match status.
//!
//! Features:
//! - Parallel multi-model execution
//! - Configurable agreement threshold
//! - Unanimous mode for critical decisions
//! - Detailed consensus statistics

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::future::join_all;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::{Offer, Request};
use crate::matching::reviewer::{ReviewResult, ReviewStatus};
use ai_client::{AIContext, Client as AIClient, ClientConfig};

/// Configuration for consensus auditing
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Minimum agreement ratio required (0.0 - 1.0)
    /// Default: 0.67 (2/3 majority)
    pub min_agreement: f64,
    /// Require unanimous agreement for approval
    /// Default: false
    pub require_unanimous: bool,
    /// Timeout for each model in seconds
    pub model_timeout_secs: u64,
    /// Whether consensus auditing is enabled
    pub enabled: bool,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            min_agreement: 0.67,
            require_unanimous: false,
            model_timeout_secs: 30,
            enabled: true,
        }
    }
}

impl ConsensusConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            min_agreement: std::env::var("CONSENSUS_MIN_AGREEMENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.67),
            require_unanimous: std::env::var("CONSENSUS_REQUIRE_UNANIMOUS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            model_timeout_secs: std::env::var("CONSENSUS_MODEL_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            enabled: std::env::var("CONSENSUS_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
        }
    }

    /// Strict configuration requiring unanimous agreement
    pub fn strict() -> Self {
        Self {
            min_agreement: 1.0,
            require_unanimous: true,
            model_timeout_secs: 30,
            enabled: true,
        }
    }

    /// Relaxed configuration with simple majority
    pub fn relaxed() -> Self {
        Self {
            min_agreement: 0.5,
            require_unanimous: false,
            model_timeout_secs: 30,
            enabled: true,
        }
    }
}

/// Result from a single model audit
#[derive(Debug, Clone)]
pub struct ModelAuditResult {
    /// Model identifier
    pub model_id: String,
    /// The review result from this model
    pub result: Option<ReviewResult>,
    /// Error message if the model failed
    pub error: Option<String>,
    /// Time taken in milliseconds
    pub duration_ms: u64,
}

/// Consensus result from multiple models
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsensusResult {
    /// Final consensus status
    pub status: ReviewStatus,
    /// Consensus confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Agreement ratio among models
    pub agreement_ratio: f64,
    /// Number of models that agreed
    pub agreeing_models: usize,
    /// Total number of models queried
    pub total_models: usize,
    /// Combined explanation from models
    pub explanation: String,
    /// Whether consensus was reached
    pub consensus_reached: bool,
    /// Individual model results (for debugging)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub model_details: Vec<ModelDetail>,
}

/// Detail from a single model (for debugging)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelDetail {
    pub model_id: String,
    pub status: Option<String>,
    pub confidence: Option<f32>,
    pub error: Option<String>,
}

/// Statistics for consensus auditing
#[derive(Debug, Default)]
pub struct ConsensusStats {
    pub total_audits: AtomicU64,
    pub consensus_reached: AtomicU64,
    pub unanimous_agreements: AtomicU64,
    pub model_failures: AtomicU64,
    pub approvals: AtomicU64,
    pub rejections: AtomicU64,
    pub flagged: AtomicU64,
}

impl ConsensusStats {
    pub fn snapshot(&self) -> ConsensusStatsSnapshot {
        ConsensusStatsSnapshot {
            total_audits: self.total_audits.load(Ordering::Relaxed),
            consensus_reached: self.consensus_reached.load(Ordering::Relaxed),
            unanimous_agreements: self.unanimous_agreements.load(Ordering::Relaxed),
            model_failures: self.model_failures.load(Ordering::Relaxed),
            approvals: self.approvals.load(Ordering::Relaxed),
            rejections: self.rejections.load(Ordering::Relaxed),
            flagged: self.flagged.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.total_audits.store(0, Ordering::Relaxed);
        self.consensus_reached.store(0, Ordering::Relaxed);
        self.unanimous_agreements.store(0, Ordering::Relaxed);
        self.model_failures.store(0, Ordering::Relaxed);
        self.approvals.store(0, Ordering::Relaxed);
        self.rejections.store(0, Ordering::Relaxed);
        self.flagged.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of consensus statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusStatsSnapshot {
    pub total_audits: u64,
    pub consensus_reached: u64,
    pub unanimous_agreements: u64,
    pub model_failures: u64,
    pub approvals: u64,
    pub rejections: u64,
    pub flagged: u64,
}

/// Multi-model consensus auditor
pub struct ConsensusAuditor {
    config: ConsensusConfig,
    clients: Vec<(String, Arc<AIClient>)>,
    stats: ConsensusStats,
}

impl ConsensusAuditor {
    /// Create a new consensus auditor with the given clients
    pub fn new(config: ConsensusConfig, clients: Vec<(String, Arc<AIClient>)>) -> Self {
        Self {
            config,
            clients,
            stats: ConsensusStats::default(),
        }
    }

    /// Create from environment variables
    /// Expects CONSENSUS_MODELS as comma-separated model names
    /// Each model uses the same base URL but different model identifier
    pub fn from_env() -> Self {
        let config = ConsensusConfig::from_env();

        let models: Vec<String> = std::env::var("CONSENSUS_MODELS")
            .unwrap_or_else(|_| "ai/qwen3-vl:latest".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let base_config = ClientConfig::from_env();
        let clients: Vec<(String, Arc<AIClient>)> = models
            .into_iter()
            .map(|model| {
                let mut model_config = base_config.clone();
                model_config.model = model.clone();
                (model, Arc::new(AIClient::new(model_config)))
            })
            .collect();

        Self::new(config, clients)
    }

    /// Audit a match using multiple models
    pub async fn audit_match(
        &self,
        offer: &Offer,
        request: &Request,
        score: f64,
        reasoning: &str,
    ) -> ConsensusResult {
        self.stats.total_audits.fetch_add(1, Ordering::Relaxed);

        if !self.config.enabled || self.clients.is_empty() {
            return ConsensusResult {
                status: ReviewStatus::Flagged,
                confidence: 0.0,
                agreement_ratio: 0.0,
                agreeing_models: 0,
                total_models: 0,
                explanation: "Consensus auditing disabled or no models configured".to_string(),
                consensus_reached: false,
                model_details: vec![],
            };
        }

        // Run all models in parallel
        let futures: Vec<_> = self
            .clients
            .iter()
            .map(|(model_id, client)| {
                let model_id = model_id.clone();
                let client = client.clone();
                let offer = offer.clone();
                let request = request.clone();
                let reasoning = reasoning.to_string();

                async move {
                    let start = std::time::Instant::now();
                    let result =
                        Self::audit_with_model(&client, &offer, &request, score, &reasoning).await;
                    let duration_ms = start.elapsed().as_millis() as u64;

                    match result {
                        Ok(review) => ModelAuditResult {
                            model_id,
                            result: Some(review),
                            error: None,
                            duration_ms,
                        },
                        Err(e) => ModelAuditResult {
                            model_id,
                            result: None,
                            error: Some(e.to_string()),
                            duration_ms,
                        },
                    }
                }
            })
            .collect();

        let results = join_all(futures).await;

        // Aggregate results
        self.aggregate_results(results)
    }

    /// Audit with a single model
    async fn audit_with_model(
        client: &AIClient,
        offer: &Offer,
        request: &Request,
        score: f64,
        _reasoning: &str,
    ) -> Result<ReviewResult, ai_client::Error> {
        let system_prompt = r#"You are a medication NAME MATCHER. Your ONLY job is to compare medication names.

RULES:
1. Compare ONLY the medication names provided
2. Do NOT provide any medical information, drug details, or therapeutic uses
3. Do NOT explain what the medications are or what they do
4. Focus ONLY on whether the names refer to the same product

COMPARISON CRITERIA:
- Are the names the same brand/product? (exact or transliteration match)
- Are the Arabic names (Raw) equivalent transliterations?
- Ignore dosage numbers when comparing names (note them as "ignored" in dosage comparison)

OUTPUT FORMAT:
You MUST provide a JSON response with detailed match analysis:
- status: "approved" (names match), "flagged" (uncertain), or "rejected" (different)
- confidence: 0.0 to 1.0
- explanation: Brief summary of name comparison result
- match_details: Structured analysis with:
  - brand_match: Compare brand/product names with match_type ("exact", "transliteration", "fuzzy", "no_match")
  - arabic_match: Compare Arabic/raw names with match_type
  - dosage: Compare dosages, set ignored=true if dosage differs but names match
  - differences: List key differences found (if any)
  - decision_reasons: List reasons for your decision

Keep explanations focused ONLY on name comparison."#;

        let user_prompt = format!(
            "Compare these medication NAMES only:\n\n\
            OFFER:\n  Brand Name: {}\n  Arabic/Raw: {}\n\n\
            REQUEST:\n  Brand Name: {}\n  Arabic/Raw: {}\n\n\
            Scoring engine score: {:.1}%\n\n\
            Provide detailed match analysis in the specified JSON format.",
            offer.medication,
            offer.medication_raw,
            request.medication,
            request.medication_raw,
            score * 100.0
        );

        client
            .generate_object_with_context::<ReviewResult>(
                system_prompt,
                &user_prompt,
                AIContext::Comparison,
            )
            .await
    }

    /// Aggregate results from multiple models
    fn aggregate_results(&self, results: Vec<ModelAuditResult>) -> ConsensusResult {
        let total_models = results.len();
        let mut model_details = Vec::with_capacity(total_models);

        // Count votes for each status
        let mut approved_count = 0;
        let mut rejected_count = 0;
        let mut flagged_count = 0;
        let mut failed_count = 0;
        let mut total_confidence = 0.0;
        let mut explanations = Vec::new();

        for result in &results {
            let detail = ModelDetail {
                model_id: result.model_id.clone(),
                status: result.result.as_ref().map(|r| format!("{:?}", r.status)),
                confidence: result.result.as_ref().map(|r| r.confidence),
                error: result.error.clone(),
            };
            model_details.push(detail);

            if let Some(ref review) = result.result {
                total_confidence += review.confidence as f64;
                explanations.push(format!("[{}] {}", result.model_id, review.explanation));

                match review.status {
                    ReviewStatus::Approved => approved_count += 1,
                    ReviewStatus::Rejected => rejected_count += 1,
                    ReviewStatus::Flagged => flagged_count += 1,
                }
            } else {
                failed_count += 1;
                self.stats.model_failures.fetch_add(1, Ordering::Relaxed);
            }
        }

        let successful_models = total_models - failed_count;
        if successful_models == 0 {
            return ConsensusResult {
                status: ReviewStatus::Flagged,
                confidence: 0.0,
                agreement_ratio: 0.0,
                agreeing_models: 0,
                total_models,
                explanation: "All models failed to respond".to_string(),
                consensus_reached: false,
                model_details,
            };
        }

        // Determine majority status
        let (majority_status, majority_count) =
            if approved_count >= rejected_count && approved_count >= flagged_count {
                (ReviewStatus::Approved, approved_count)
            } else if rejected_count >= approved_count && rejected_count >= flagged_count {
                (ReviewStatus::Rejected, rejected_count)
            } else {
                (ReviewStatus::Flagged, flagged_count)
            };

        let agreement_ratio = majority_count as f64 / successful_models as f64;
        let avg_confidence = total_confidence / successful_models as f64;

        // Check if consensus is reached
        let consensus_reached = if self.config.require_unanimous {
            majority_count == successful_models
        } else {
            agreement_ratio >= self.config.min_agreement
        };

        // Update stats
        if consensus_reached {
            self.stats.consensus_reached.fetch_add(1, Ordering::Relaxed);
            if majority_count == successful_models {
                self.stats
                    .unanimous_agreements
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        match majority_status {
            ReviewStatus::Approved => self.stats.approvals.fetch_add(1, Ordering::Relaxed),
            ReviewStatus::Rejected => self.stats.rejections.fetch_add(1, Ordering::Relaxed),
            ReviewStatus::Flagged => self.stats.flagged.fetch_add(1, Ordering::Relaxed),
        };

        // If consensus not reached, flag for human review
        let final_status = if consensus_reached {
            majority_status
        } else {
            ReviewStatus::Flagged
        };

        let explanation = if consensus_reached {
            format!(
                "Consensus reached ({}/{} models agree): {}",
                majority_count,
                successful_models,
                explanations.join("; ")
            )
        } else {
            format!(
                "No consensus ({}/{} models agree, need {:.0}%): {}",
                majority_count,
                successful_models,
                self.config.min_agreement * 100.0,
                explanations.join("; ")
            )
        };

        ConsensusResult {
            status: final_status,
            confidence: avg_confidence,
            agreement_ratio,
            agreeing_models: majority_count,
            total_models,
            explanation,
            consensus_reached,
            model_details,
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> ConsensusStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// Get current configuration
    pub fn config(&self) -> &ConsensusConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ConsensusConfig) {
        self.config = config;
    }

    /// Get number of configured models
    pub fn model_count(&self) -> usize {
        self.clients.len()
    }

    /// Check if consensus auditing is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.clients.is_empty()
    }

    /// Enable or disable consensus auditing
    pub fn enable(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConsensusConfig::default();
        assert!((config.min_agreement - 0.67).abs() < 0.01);
        assert!(!config.require_unanimous);
        assert!(config.enabled);
    }

    #[test]
    fn test_strict_config() {
        let config = ConsensusConfig::strict();
        assert!((config.min_agreement - 1.0).abs() < 0.01);
        assert!(config.require_unanimous);
    }

    #[test]
    fn test_relaxed_config() {
        let config = ConsensusConfig::relaxed();
        assert!((config.min_agreement - 0.5).abs() < 0.01);
        assert!(!config.require_unanimous);
    }

    #[test]
    fn test_stats_snapshot() {
        let stats = ConsensusStats::default();
        stats.total_audits.store(10, Ordering::Relaxed);
        stats.consensus_reached.store(8, Ordering::Relaxed);
        stats.approvals.store(5, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_audits, 10);
        assert_eq!(snapshot.consensus_reached, 8);
        assert_eq!(snapshot.approvals, 5);
    }

    #[test]
    fn test_stats_reset() {
        let stats = ConsensusStats::default();
        stats.total_audits.store(10, Ordering::Relaxed);
        stats.reset();
        assert_eq!(stats.total_audits.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_consensus_auditor_disabled() {
        let config = ConsensusConfig {
            enabled: false,
            ..Default::default()
        };
        let auditor = ConsensusAuditor::new(config, vec![]);
        assert!(!auditor.is_enabled());
        assert_eq!(auditor.model_count(), 0);
    }

    #[test]
    fn test_aggregate_all_approved() {
        let config = ConsensusConfig::default();
        let auditor = ConsensusAuditor::new(config, vec![]);

        let results = vec![
            ModelAuditResult {
                model_id: "model1".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.9,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 100,
            },
            ModelAuditResult {
                model_id: "model2".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.85,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 120,
            },
        ];

        let consensus = auditor.aggregate_results(results);
        assert_eq!(consensus.status, ReviewStatus::Approved);
        assert!((consensus.agreement_ratio - 1.0).abs() < 0.01);
        assert!(consensus.consensus_reached);
    }

    #[test]
    fn test_aggregate_mixed_no_consensus() {
        let config = ConsensusConfig {
            min_agreement: 0.67,
            ..Default::default()
        };
        let auditor = ConsensusAuditor::new(config, vec![]);

        let results = vec![
            ModelAuditResult {
                model_id: "model1".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.9,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 100,
            },
            ModelAuditResult {
                model_id: "model2".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Rejected,
                    confidence: 0.8,
                    explanation: "No match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 120,
            },
            ModelAuditResult {
                model_id: "model3".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Flagged,
                    confidence: 0.5,
                    explanation: "Uncertain".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 110,
            },
        ];

        let consensus = auditor.aggregate_results(results);
        // No majority, so flagged
        assert_eq!(consensus.status, ReviewStatus::Flagged);
        assert!(!consensus.consensus_reached);
    }

    #[test]
    fn test_aggregate_with_failures() {
        let config = ConsensusConfig::default();
        let auditor = ConsensusAuditor::new(config, vec![]);

        let results = vec![
            ModelAuditResult {
                model_id: "model1".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.9,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 100,
            },
            ModelAuditResult {
                model_id: "model2".to_string(),
                result: None,
                error: Some("Timeout".to_string()),
                duration_ms: 30000,
            },
        ];

        let consensus = auditor.aggregate_results(results);
        // Only 1 successful model, 100% agreement
        assert_eq!(consensus.status, ReviewStatus::Approved);
        assert!((consensus.agreement_ratio - 1.0).abs() < 0.01);
        assert_eq!(consensus.total_models, 2);
        assert_eq!(consensus.agreeing_models, 1);
    }

    #[test]
    fn test_aggregate_all_failed() {
        let config = ConsensusConfig::default();
        let auditor = ConsensusAuditor::new(config, vec![]);

        let results = vec![
            ModelAuditResult {
                model_id: "model1".to_string(),
                result: None,
                error: Some("Error 1".to_string()),
                duration_ms: 100,
            },
            ModelAuditResult {
                model_id: "model2".to_string(),
                result: None,
                error: Some("Error 2".to_string()),
                duration_ms: 100,
            },
        ];

        let consensus = auditor.aggregate_results(results);
        assert_eq!(consensus.status, ReviewStatus::Flagged);
        assert!(!consensus.consensus_reached);
        assert!(consensus.explanation.contains("All models failed"));
    }

    #[test]
    fn test_unanimous_required() {
        let config = ConsensusConfig {
            require_unanimous: true,
            min_agreement: 1.0,
            ..Default::default()
        };
        let auditor = ConsensusAuditor::new(config, vec![]);

        let results = vec![
            ModelAuditResult {
                model_id: "model1".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.9,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 100,
            },
            ModelAuditResult {
                model_id: "model2".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Approved,
                    confidence: 0.85,
                    explanation: "Match".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 120,
            },
            ModelAuditResult {
                model_id: "model3".to_string(),
                result: Some(ReviewResult {
                    status: ReviewStatus::Flagged,
                    confidence: 0.5,
                    explanation: "Uncertain".to_string(),
                    suggested_action: None,
                    match_details: None,
                }),
                error: None,
                duration_ms: 110,
            },
        ];

        let consensus = auditor.aggregate_results(results);
        // Not unanimous, so flagged
        assert_eq!(consensus.status, ReviewStatus::Flagged);
        assert!(!consensus.consensus_reached);
    }
}
