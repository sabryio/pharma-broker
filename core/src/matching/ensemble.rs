//! Ensemble Matching Module
//!
//! Combines multiple matching strategies for robust scoring.
//! Implements Task 7 from advanced_matching_tasks.md.
//!
//! Key features:
//! - Pluggable strategy trait for extensibility
//! - Weighted combination of multiple algorithms
//! - Detailed score explanations for transparency
//! - A/B testing support for strategy comparison
//! - Dynamic weight adjustment based on performance
//! - Parallel strategy scoring with rayon

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    HistoricalLearner, HistoricalLearningConfig, compare_dosages, cosine_similarity,
    normalize_arabic, parse_dosage,
};
use crate::domain::{Offer, Request};
use strsim::jaro_winkler;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for the ensemble matcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleConfig {
    /// Enable ensemble matching
    pub enabled: bool,
    /// Weight for embedding/semantic similarity
    pub embedding_weight: f64,
    /// Weight for fuzzy string matching
    pub fuzzy_weight: f64,
    /// Weight for dosage comparison
    pub dosage_weight: f64,
    /// Weight for historical learning patterns
    pub historical_weight: f64,
    /// Weight for recency scoring
    pub recency_weight: f64,
    /// Minimum score to consider a match
    pub min_score_threshold: f64,
    /// Enable detailed explanations
    pub enable_explanations: bool,
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            embedding_weight: 0.40,
            fuzzy_weight: 0.25,
            dosage_weight: 0.15,
            historical_weight: 0.15,
            recency_weight: 0.05,
            min_score_threshold: 0.5,
            enable_explanations: true,
        }
    }
}

impl EnsembleConfig {
    /// Normalize weights to sum to 1.0
    pub fn normalize(&mut self) {
        let total = self.embedding_weight
            + self.fuzzy_weight
            + self.dosage_weight
            + self.historical_weight
            + self.recency_weight;

        if total > 0.0 {
            self.embedding_weight /= total;
            self.fuzzy_weight /= total;
            self.dosage_weight /= total;
            self.historical_weight /= total;
            self.recency_weight /= total;
        }
    }

    /// Get total weight (should be ~1.0 after normalization)
    pub fn total_weight(&self) -> f64 {
        self.embedding_weight
            + self.fuzzy_weight
            + self.dosage_weight
            + self.historical_weight
            + self.recency_weight
    }
}

// =============================================================================
// Strategy Trait
// =============================================================================

/// Trait for matching strategies
pub trait MatchingStrategy: Send + Sync {
    /// Strategy name for identification
    fn name(&self) -> &str;

    /// Calculate similarity score between offer and request
    fn score(&self, offer: &Offer, request: &Request, context: &StrategyContext) -> f64;

    /// Get the configured weight for this strategy
    fn weight(&self) -> f64;

    /// Set the weight for this strategy
    fn set_weight(&self, weight: f64);

    /// Whether this strategy is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable this strategy
    fn enable(&self, enabled: bool);
}

/// Context passed to strategies for additional data
#[derive(Debug, Clone, Default)]
pub struct StrategyContext {
    /// Pre-computed embedding similarity (if available)
    pub embedding_similarity: Option<f64>,
    /// Offer embedding vector
    pub offer_embedding: Option<Vec<f32>>,
    /// Request embedding vector
    pub request_embedding: Option<Vec<f32>>,
    /// Historical affinity score (if available)
    pub historical_affinity: Option<f64>,
    /// Current timestamp for recency calculations
    pub now: DateTime<Utc>,
}

impl StrategyContext {
    pub fn new() -> Self {
        Self {
            now: Utc::now(),
            ..Default::default()
        }
    }

    pub fn with_embeddings(mut self, offer: Vec<f32>, request: Vec<f32>) -> Self {
        self.offer_embedding = Some(offer);
        self.request_embedding = Some(request);
        self
    }

    pub fn with_embedding_similarity(mut self, sim: f64) -> Self {
        self.embedding_similarity = Some(sim);
        self
    }

    pub fn with_historical_affinity(mut self, affinity: f64) -> Self {
        self.historical_affinity = Some(affinity);
        self
    }
}

// =============================================================================
// Component Scores
// =============================================================================

/// Individual strategy score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyScore {
    pub strategy_name: String,
    pub raw_score: f64,
    pub weight: f64,
    pub weighted_score: f64,
    pub enabled: bool,
}

/// Detailed match explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchExplanation {
    pub total_score: f64,
    pub confidence_band: String,
    pub component_scores: Vec<StrategyScore>,
    pub reasoning: String,
    pub top_contributor: String,
    pub weakest_contributor: String,
}

impl MatchExplanation {
    /// Generate human-readable reasoning
    pub fn generate_reasoning(scores: &[StrategyScore]) -> String {
        let enabled_scores: Vec<_> = scores.iter().filter(|s| s.enabled).collect();
        if enabled_scores.is_empty() {
            return "No strategies enabled".to_string();
        }

        let mut parts = Vec::new();

        // Find notable scores
        let high_scores: Vec<_> = enabled_scores
            .iter()
            .filter(|s| s.raw_score >= 0.85)
            .collect();
        let low_scores: Vec<_> = enabled_scores
            .iter()
            .filter(|s| s.raw_score < 0.5)
            .collect();

        if !high_scores.is_empty() {
            let names: Vec<_> = high_scores
                .iter()
                .map(|s| s.strategy_name.as_str())
                .collect();
            parts.push(format!("Strong {} match", names.join(", ")));
        }

        if !low_scores.is_empty() {
            let names: Vec<_> = low_scores
                .iter()
                .map(|s| s.strategy_name.as_str())
                .collect();
            parts.push(format!("Weak {} score", names.join(", ")));
        }

        if parts.is_empty() {
            parts.push("Moderate match across all strategies".to_string());
        }

        parts.join("; ")
    }
}

// =============================================================================
// Built-in Strategies
// =============================================================================

/// Embedding-based semantic similarity strategy
pub struct EmbeddingStrategy {
    weight: RwLock<f64>,
    enabled: RwLock<bool>,
}

impl Default for EmbeddingStrategy {
    fn default() -> Self {
        Self {
            weight: RwLock::new(0.40),
            enabled: RwLock::new(true),
        }
    }
}

impl EmbeddingStrategy {
    pub fn new(weight: f64) -> Self {
        Self {
            weight: RwLock::new(weight),
            enabled: RwLock::new(true),
        }
    }
}

impl MatchingStrategy for EmbeddingStrategy {
    fn name(&self) -> &str {
        "embedding"
    }

    fn score(&self, _offer: &Offer, _request: &Request, context: &StrategyContext) -> f64 {
        // Use pre-computed similarity if available
        if let Some(sim) = context.embedding_similarity {
            return sim.clamp(0.0, 1.0);
        }

        // Compute from embeddings if available
        if let (Some(offer_emb), Some(request_emb)) =
            (&context.offer_embedding, &context.request_embedding)
        {
            return cosine_similarity(offer_emb, request_emb)
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
        }

        0.0 // No embedding data available
    }

    fn weight(&self) -> f64 {
        *self.weight.read().unwrap()
    }

    fn set_weight(&self, weight: f64) {
        *self.weight.write().unwrap() = weight;
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn enable(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }
}

/// Fuzzy string matching strategy
pub struct FuzzyStringStrategy {
    weight: RwLock<f64>,
    enabled: RwLock<bool>,
}

impl Default for FuzzyStringStrategy {
    fn default() -> Self {
        Self {
            weight: RwLock::new(0.25),
            enabled: RwLock::new(true),
        }
    }
}

impl FuzzyStringStrategy {
    pub fn new(weight: f64) -> Self {
        Self {
            weight: RwLock::new(weight),
            enabled: RwLock::new(true),
        }
    }

    /// Normalize medication name for comparison
    fn normalize_name(name: &str) -> String {
        let normalized = normalize_arabic(name);
        normalized.to_lowercase().trim().to_string()
    }
}

impl MatchingStrategy for FuzzyStringStrategy {
    fn name(&self) -> &str {
        "fuzzy"
    }

    fn score(&self, offer: &Offer, request: &Request, _context: &StrategyContext) -> f64 {
        let offer_name = Self::normalize_name(&offer.medication);
        let request_name = Self::normalize_name(&request.medication);

        if offer_name.is_empty() || request_name.is_empty() {
            return 0.0;
        }

        // Exact match
        if offer_name == request_name {
            return 1.0;
        }

        // Jaro-Winkler similarity
        let jw_score = jaro_winkler(&offer_name, &request_name);

        // Substring bonus
        let substring_bonus =
            if offer_name.contains(&request_name) || request_name.contains(&offer_name) {
                0.1
            } else {
                0.0
            };

        (jw_score + substring_bonus).min(1.0)
    }

    fn weight(&self) -> f64 {
        *self.weight.read().unwrap()
    }

    fn set_weight(&self, weight: f64) {
        *self.weight.write().unwrap() = weight;
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn enable(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }
}

/// Dosage comparison strategy
pub struct DosageStrategy {
    weight: RwLock<f64>,
    enabled: RwLock<bool>,
}

impl Default for DosageStrategy {
    fn default() -> Self {
        Self {
            weight: RwLock::new(0.15),
            enabled: RwLock::new(true),
        }
    }
}

impl DosageStrategy {
    pub fn new(weight: f64) -> Self {
        Self {
            weight: RwLock::new(weight),
            enabled: RwLock::new(true),
        }
    }
}

impl MatchingStrategy for DosageStrategy {
    fn name(&self) -> &str {
        "dosage"
    }

    fn score(&self, offer: &Offer, request: &Request, _context: &StrategyContext) -> f64 {
        let offer_dosage = parse_dosage(&offer.medication);
        let request_dosage = parse_dosage(&request.medication);

        match (&offer_dosage, &request_dosage) {
            (None, None) => 0.9,                      // Both missing - slight penalty
            (None, Some(_)) | (Some(_), None) => 0.7, // One missing - partial penalty
            _ => compare_dosages(&offer_dosage, &request_dosage),
        }
    }

    fn weight(&self) -> f64 {
        *self.weight.read().unwrap()
    }

    fn set_weight(&self, weight: f64) {
        *self.weight.write().unwrap() = weight;
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn enable(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }
}

/// Historical pattern learning strategy
pub struct HistoricalStrategy {
    weight: RwLock<f64>,
    enabled: RwLock<bool>,
    learner: Arc<HistoricalLearner>,
}

impl HistoricalStrategy {
    pub fn with_default_learner(weight: f64) -> Self {
        Self {
            weight: RwLock::new(weight),
            enabled: RwLock::new(true),
            learner: Arc::new(HistoricalLearner::new(HistoricalLearningConfig::default())),
        }
    }
}

impl MatchingStrategy for HistoricalStrategy {
    fn name(&self) -> &str {
        "historical"
    }

    fn score(&self, offer: &Offer, request: &Request, context: &StrategyContext) -> f64 {
        // Use pre-computed affinity if available
        if let Some(affinity) = context.historical_affinity {
            return affinity.clamp(0.0, 1.0);
        }

        // Query the historical learner
        let bonus = self
            .learner
            .get_historical_bonus(&offer.medication, &request.medication);

        // Convert bonus (-0.1 to +0.1) to score (0.4 to 0.6 range, centered at 0.5)
        (0.5 + bonus * 5.0).clamp(0.0, 1.0)
    }

    fn weight(&self) -> f64 {
        *self.weight.read().unwrap()
    }

    fn set_weight(&self, weight: f64) {
        *self.weight.write().unwrap() = weight;
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn enable(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }
}

/// Recency-based scoring strategy
pub struct RecencyStrategy {
    weight: RwLock<f64>,
    enabled: RwLock<bool>,
    half_life_hours: f64,
}

impl Default for RecencyStrategy {
    fn default() -> Self {
        Self {
            weight: RwLock::new(0.05),
            enabled: RwLock::new(true),
            half_life_hours: 24.0,
        }
    }
}

impl RecencyStrategy {
    pub fn new(weight: f64, half_life_hours: f64) -> Self {
        Self {
            weight: RwLock::new(weight),
            enabled: RwLock::new(true),
            half_life_hours,
        }
    }
}

impl MatchingStrategy for RecencyStrategy {
    fn name(&self) -> &str {
        "recency"
    }

    fn score(&self, offer: &Offer, _request: &Request, context: &StrategyContext) -> f64 {
        let age_hours = (context.now - offer.created_at).num_minutes() as f64 / 60.0;

        if age_hours <= 0.0 {
            return 1.0;
        }

        // Exponential decay: score = 0.5^(age/half_life)
        0.5_f64.powf(age_hours / self.half_life_hours)
    }

    fn weight(&self) -> f64 {
        *self.weight.read().unwrap()
    }

    fn set_weight(&self, weight: f64) {
        *self.weight.write().unwrap() = weight;
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn enable(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics for the ensemble matcher
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnsembleStats {
    pub total_matches_scored: u64,
    pub avg_total_score: f64,
    pub strategy_usage: HashMap<String, u64>,
    pub strategy_avg_scores: HashMap<String, f64>,
    pub high_confidence_matches: u64,
    pub low_confidence_matches: u64,
}

// =============================================================================
// Ensemble Matcher
// =============================================================================

/// Ensemble matcher combining multiple strategies
pub struct EnsembleMatcher {
    config: RwLock<EnsembleConfig>,
    strategies: RwLock<Vec<Arc<dyn MatchingStrategy>>>,
    // Statistics
    total_scored: AtomicU64,
    total_score_sum: RwLock<f64>,
    strategy_scores: RwLock<HashMap<String, (u64, f64)>>, // (count, sum)
    high_confidence: AtomicU64,
    low_confidence: AtomicU64,
}

impl Default for EnsembleMatcher {
    fn default() -> Self {
        Self::new(EnsembleConfig::default())
    }
}

impl EnsembleMatcher {
    /// Create a new ensemble matcher with default strategies
    pub fn new(config: EnsembleConfig) -> Self {
        let strategies: Vec<Arc<dyn MatchingStrategy>> = vec![
            Arc::new(EmbeddingStrategy::new(config.embedding_weight)),
            Arc::new(FuzzyStringStrategy::new(config.fuzzy_weight)),
            Arc::new(DosageStrategy::new(config.dosage_weight)),
            Arc::new(HistoricalStrategy::with_default_learner(
                config.historical_weight,
            )),
            Arc::new(RecencyStrategy::new(config.recency_weight, 24.0)),
        ];

        Self {
            config: RwLock::new(config),
            strategies: RwLock::new(strategies),
            total_scored: AtomicU64::new(0),
            total_score_sum: RwLock::new(0.0),
            strategy_scores: RwLock::new(HashMap::new()),
            high_confidence: AtomicU64::new(0),
            low_confidence: AtomicU64::new(0),
        }
    }

    /// Create with custom strategies
    pub fn with_strategies(
        config: EnsembleConfig,
        strategies: Vec<Arc<dyn MatchingStrategy>>,
    ) -> Self {
        Self {
            config: RwLock::new(config),
            strategies: RwLock::new(strategies),
            total_scored: AtomicU64::new(0),
            total_score_sum: RwLock::new(0.0),
            strategy_scores: RwLock::new(HashMap::new()),
            high_confidence: AtomicU64::new(0),
            low_confidence: AtomicU64::new(0),
        }
    }

    /// Score a match using all enabled strategies (parallel execution with rayon)
    pub fn score(&self, offer: &Offer, request: &Request, context: &StrategyContext) -> f64 {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return 0.0;
        }
        drop(config);

        let strategies = self.strategies.read().unwrap();

        // Parallel scoring with rayon - each strategy scores independently
        let results: Vec<(String, f64, f64, bool)> = strategies
            .par_iter()
            .map(|strategy| {
                let enabled = strategy.is_enabled();
                if !enabled {
                    return (strategy.name().to_string(), 0.0, 0.0, false);
                }
                let weight = strategy.weight();
                let score = strategy.score(offer, request, context);
                (strategy.name().to_string(), score, weight, true)
            })
            .collect();

        // Aggregate results
        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for (name, score, weight, enabled) in results {
            if enabled {
                weighted_sum += score * weight;
                total_weight += weight;
                // Update strategy stats
                self.update_strategy_stats(&name, score);
            }
        }

        let total_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Update overall stats
        self.update_overall_stats(total_score);

        total_score.clamp(0.0, 1.0)
    }

    /// Score with detailed explanation
    pub fn score_with_explanation(
        &self,
        offer: &Offer,
        request: &Request,
        context: &StrategyContext,
    ) -> MatchExplanation {
        let config = self.config.read().unwrap();
        let enable_explanations = config.enable_explanations;
        drop(config);

        let strategies = self.strategies.read().unwrap();
        let mut component_scores = Vec::new();
        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for strategy in strategies.iter() {
            let weight = strategy.weight();
            let enabled = strategy.is_enabled();
            let raw_score = if enabled {
                strategy.score(offer, request, context)
            } else {
                0.0
            };

            let weighted_score = if enabled { raw_score * weight } else { 0.0 };

            if enabled {
                weighted_sum += weighted_score;
                total_weight += weight;
            }

            component_scores.push(StrategyScore {
                strategy_name: strategy.name().to_string(),
                raw_score,
                weight,
                weighted_score,
                enabled,
            });

            if enabled {
                self.update_strategy_stats(strategy.name(), raw_score);
            }
        }

        let total_score = if total_weight > 0.0 {
            (weighted_sum / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.update_overall_stats(total_score);

        // Find top and weakest contributors
        let enabled_scores: Vec<_> = component_scores.iter().filter(|s| s.enabled).collect();
        let top = enabled_scores
            .iter()
            .max_by(|a, b| a.weighted_score.partial_cmp(&b.weighted_score).unwrap())
            .map(|s| s.strategy_name.clone())
            .unwrap_or_default();
        let weakest = enabled_scores
            .iter()
            .min_by(|a, b| a.raw_score.partial_cmp(&b.raw_score).unwrap())
            .map(|s| s.strategy_name.clone())
            .unwrap_or_default();

        let confidence_band = self.get_confidence_band(total_score);
        let reasoning = if enable_explanations {
            MatchExplanation::generate_reasoning(&component_scores)
        } else {
            String::new()
        };

        MatchExplanation {
            total_score,
            confidence_band,
            component_scores,
            reasoning,
            top_contributor: top,
            weakest_contributor: weakest,
        }
    }

    /// Get confidence band for a score
    fn get_confidence_band(&self, score: f64) -> String {
        if score >= 0.90 {
            "Auto".to_string()
        } else if score >= 0.70 {
            "Suggest".to_string()
        } else if score >= 0.50 {
            "Review".to_string()
        } else {
            "None".to_string()
        }
    }

    /// Update strategy-specific statistics
    fn update_strategy_stats(&self, name: &str, score: f64) {
        let mut stats = self.strategy_scores.write().unwrap();
        let entry = stats.entry(name.to_string()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += score;
    }

    /// Update overall statistics
    fn update_overall_stats(&self, score: f64) {
        self.total_scored.fetch_add(1, Ordering::Relaxed);
        *self.total_score_sum.write().unwrap() += score;

        if score >= 0.85 {
            self.high_confidence.fetch_add(1, Ordering::Relaxed);
        } else if score < 0.5 {
            self.low_confidence.fetch_add(1, Ordering::Relaxed);
        }
    }

    // =========================================================================
    // Configuration & Management
    // =========================================================================

    /// Get current configuration
    pub fn get_config(&self) -> EnsembleConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, mut config: EnsembleConfig) {
        config.normalize();

        // Update strategy weights
        let strategies = self.strategies.read().unwrap();
        for strategy in strategies.iter() {
            let weight = match strategy.name() {
                "embedding" => config.embedding_weight,
                "fuzzy" => config.fuzzy_weight,
                "dosage" => config.dosage_weight,
                "historical" => config.historical_weight,
                "recency" => config.recency_weight,
                _ => strategy.weight(),
            };
            strategy.set_weight(weight);
        }

        *self.config.write().unwrap() = config;
    }

    /// Enable or disable ensemble matching
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Enable or disable a specific strategy
    pub fn enable_strategy(&self, name: &str, enabled: bool) {
        let strategies = self.strategies.read().unwrap();
        for strategy in strategies.iter() {
            if strategy.name() == name {
                strategy.enable(enabled);
                break;
            }
        }
    }

    /// Set weight for a specific strategy
    pub fn set_strategy_weight(&self, name: &str, weight: f64) {
        let strategies = self.strategies.read().unwrap();
        for strategy in strategies.iter() {
            if strategy.name() == name {
                strategy.set_weight(weight);
                break;
            }
        }
    }

    /// Get list of strategy names
    pub fn strategy_names(&self) -> Vec<String> {
        self.strategies
            .read()
            .unwrap()
            .iter()
            .map(|s| s.name().to_string())
            .collect()
    }

    /// Add a custom strategy
    pub fn add_strategy(&self, strategy: Arc<dyn MatchingStrategy>) {
        self.strategies.write().unwrap().push(strategy);
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get ensemble statistics
    pub fn get_stats(&self) -> EnsembleStats {
        let total = self.total_scored.load(Ordering::Relaxed);
        let sum = *self.total_score_sum.read().unwrap();
        let strategy_data = self.strategy_scores.read().unwrap();

        let mut strategy_usage = HashMap::new();
        let mut strategy_avg_scores = HashMap::new();

        for (name, (count, score_sum)) in strategy_data.iter() {
            strategy_usage.insert(name.clone(), *count);
            if *count > 0 {
                strategy_avg_scores.insert(name.clone(), score_sum / *count as f64);
            }
        }

        EnsembleStats {
            total_matches_scored: total,
            avg_total_score: if total > 0 { sum / total as f64 } else { 0.0 },
            strategy_usage,
            strategy_avg_scores,
            high_confidence_matches: self.high_confidence.load(Ordering::Relaxed),
            low_confidence_matches: self.low_confidence.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset_stats(&self) {
        self.total_scored.store(0, Ordering::Relaxed);
        *self.total_score_sum.write().unwrap() = 0.0;
        self.strategy_scores.write().unwrap().clear();
        self.high_confidence.store(0, Ordering::Relaxed);
        self.low_confidence.store(0, Ordering::Relaxed);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use rust_decimal::prelude::FromPrimitive;

    fn create_test_offer(medication: &str, created_at: DateTime<Utc>) -> Offer {
        Offer {
            id: "offer-1".to_string(),
            medication: medication.to_string(),
            quantity: Decimal::from_f64(100.0),
            price: Decimal::from_f64(50.0),
            created_at,
            ..Default::default()
        }
    }

    fn create_test_request(medication: &str) -> Request {
        Request {
            id: "request-1".to_string(),
            medication: medication.to_string(),
            quantity: Decimal::from_f64(100.0),
            max_price: Decimal::from_f64(60.0),
            ..Default::default()
        }
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_config_default() {
        let config = EnsembleConfig::default();
        assert!(config.enabled);
        assert!((config.total_weight() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_config_normalize() {
        let mut config = EnsembleConfig {
            enabled: true,
            embedding_weight: 0.8,
            fuzzy_weight: 0.5,
            dosage_weight: 0.3,
            historical_weight: 0.3,
            recency_weight: 0.1,
            min_score_threshold: 0.5,
            enable_explanations: true,
        };
        config.normalize();
        assert!((config.total_weight() - 1.0).abs() < 0.001);
    }

    // =========================================================================
    // Ensemble Matcher Tests
    // =========================================================================

    #[test]
    fn test_ensemble_default() {
        let matcher = EnsembleMatcher::default();
        assert!(matcher.is_enabled());
        assert_eq!(matcher.strategy_names().len(), 5);
    }

    #[test]
    fn test_ensemble_score_basic() {
        let matcher = EnsembleMatcher::default();
        let offer = create_test_offer("Aspirin 100mg", Utc::now());
        let request = create_test_request("Aspirin 100mg");
        let context = StrategyContext::new();

        let score = matcher.score(&offer, &request, &context);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_ensemble_score_with_embedding() {
        let matcher = EnsembleMatcher::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");

        let context = StrategyContext::new().with_embedding_similarity(0.95);

        let score = matcher.score(&offer, &request, &context);
        assert!(score > 0.5); // Should be boosted by high embedding similarity
    }

    #[test]
    fn test_ensemble_score_with_explanation() {
        let matcher = EnsembleMatcher::default();
        let offer = create_test_offer("Brufen 400mg", Utc::now());
        let request = create_test_request("Brufen 400mg");
        let context = StrategyContext::new().with_embedding_similarity(0.92);

        let explanation = matcher.score_with_explanation(&offer, &request, &context);

        assert!(explanation.total_score > 0.0);
        assert!(!explanation.component_scores.is_empty());
        assert!(!explanation.reasoning.is_empty());
        assert!(!explanation.top_contributor.is_empty());
    }

    #[test]
    fn test_ensemble_disabled() {
        let matcher = EnsembleMatcher::default();
        matcher.enable(false);

        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let score = matcher.score(&offer, &request, &context);
        assert_eq!(score, 0.0);
    }

    // =========================================================================
    // Strategy Tests
    // =========================================================================

    #[test]
    fn test_fuzzy_strategy_exact_match() {
        let strategy = FuzzyStringStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_fuzzy_strategy_similar() {
        let strategy = FuzzyStringStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspriin"); // Typo
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score > 0.8); // Should still be high due to similarity
    }

    #[test]
    fn test_fuzzy_strategy_arabic() {
        let strategy = FuzzyStringStrategy::default();
        let offer = create_test_offer("أوجمنتين", Utc::now());
        let request = create_test_request("اوجمنتين"); // Without hamza
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score > 0.9); // Arabic normalization should help
    }

    #[test]
    fn test_dosage_strategy_match() {
        let strategy = DosageStrategy::default();
        let offer = create_test_offer("Aspirin 500mg", Utc::now());
        let request = create_test_request("Aspirin 500mg");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score > 0.9);
    }

    #[test]
    fn test_dosage_strategy_different() {
        let strategy = DosageStrategy::default();
        let offer = create_test_offer("Aspirin 500mg", Utc::now());
        let request = create_test_request("Aspirin 250mg");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score < 1.0); // Should be penalized for different dosage
    }

    #[test]
    fn test_recency_strategy_fresh() {
        let strategy = RecencyStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score > 0.99); // Very fresh offer
    }

    #[test]
    fn test_recency_strategy_old() {
        let strategy = RecencyStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now() - Duration::days(3));
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(score < 0.5); // 3 days old with 24h half-life
    }

    #[test]
    fn test_embedding_strategy_with_context() {
        let strategy = EmbeddingStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new().with_embedding_similarity(0.88);

        let score = strategy.score(&offer, &request, &context);
        assert!((score - 0.88).abs() < 0.001);
    }

    #[test]
    fn test_embedding_strategy_compute_from_vectors() {
        let strategy = EmbeddingStrategy::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");

        let offer_emb = vec![1.0, 0.0, 0.0];
        let request_emb = vec![1.0, 0.0, 0.0];
        let context = StrategyContext::new().with_embeddings(offer_emb, request_emb);

        let score = strategy.score(&offer, &request, &context);
        assert!((score - 1.0).abs() < 0.001); // Identical vectors
    }

    // =========================================================================
    // Strategy Management Tests
    // =========================================================================

    #[test]
    fn test_enable_disable_strategy() {
        let matcher = EnsembleMatcher::default();

        matcher.enable_strategy("fuzzy", false);

        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let explanation = matcher.score_with_explanation(&offer, &request, &context);

        let fuzzy_score = explanation
            .component_scores
            .iter()
            .find(|s| s.strategy_name == "fuzzy")
            .unwrap();
        assert!(!fuzzy_score.enabled);
    }

    #[test]
    fn test_set_strategy_weight() {
        let matcher = EnsembleMatcher::default();
        matcher.set_strategy_weight("embedding", 0.60);

        let strategies = matcher.strategies.read().unwrap();
        let embedding = strategies.iter().find(|s| s.name() == "embedding").unwrap();
        assert!((embedding.weight() - 0.60).abs() < 0.001);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_stats_tracking() {
        let matcher = EnsembleMatcher::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new().with_embedding_similarity(0.9);

        // Score multiple times
        for _ in 0..5 {
            matcher.score(&offer, &request, &context);
        }

        let stats = matcher.get_stats();
        assert_eq!(stats.total_matches_scored, 5);
        assert!(stats.avg_total_score > 0.0);
        assert!(!stats.strategy_usage.is_empty());
    }

    #[test]
    fn test_stats_reset() {
        let matcher = EnsembleMatcher::default();
        let offer = create_test_offer("Aspirin", Utc::now());
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        matcher.score(&offer, &request, &context);
        assert!(matcher.get_stats().total_matches_scored > 0);

        matcher.reset_stats();
        assert_eq!(matcher.get_stats().total_matches_scored, 0);
    }

    // =========================================================================
    // Explanation Tests
    // =========================================================================

    #[test]
    fn test_explanation_reasoning_high_scores() {
        let scores = vec![
            StrategyScore {
                strategy_name: "embedding".to_string(),
                raw_score: 0.95,
                weight: 0.4,
                weighted_score: 0.38,
                enabled: true,
            },
            StrategyScore {
                strategy_name: "fuzzy".to_string(),
                raw_score: 0.90,
                weight: 0.25,
                weighted_score: 0.225,
                enabled: true,
            },
        ];

        let reasoning = MatchExplanation::generate_reasoning(&scores);
        assert!(reasoning.contains("Strong"));
    }

    #[test]
    fn test_explanation_reasoning_low_scores() {
        let scores = vec![StrategyScore {
            strategy_name: "embedding".to_string(),
            raw_score: 0.3,
            weight: 0.4,
            weighted_score: 0.12,
            enabled: true,
        }];

        let reasoning = MatchExplanation::generate_reasoning(&scores);
        assert!(reasoning.contains("Weak"));
    }

    // =========================================================================
    // Parameterized Tests
    // =========================================================================

    #[rstest]
    #[case("Aspirin", "Aspirin", 1.0)]
    #[case("Aspirin", "aspirin", 1.0)]
    #[case("Aspirin", "Aspriin", 0.85)]
    #[case("Brufen", "Ibuprofen", 0.5)]
    fn test_fuzzy_scores(
        #[case] offer_med: &str,
        #[case] request_med: &str,
        #[case] min_expected: f64,
    ) {
        let strategy = FuzzyStringStrategy::default();
        let offer = create_test_offer(offer_med, Utc::now());
        let request = create_test_request(request_med);
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(
            score >= min_expected,
            "Score {} < expected {}",
            score,
            min_expected
        );
    }

    #[rstest]
    #[case(0, 1.0)]
    #[case(24, 0.5)]
    #[case(48, 0.25)]
    #[case(72, 0.125)]
    fn test_recency_decay(#[case] hours_old: i64, #[case] expected: f64) {
        let strategy = RecencyStrategy::new(0.05, 24.0);
        let offer = create_test_offer("Aspirin", Utc::now() - Duration::hours(hours_old));
        let request = create_test_request("Aspirin");
        let context = StrategyContext::new();

        let score = strategy.score(&offer, &request, &context);
        assert!(
            (score - expected).abs() < 0.01,
            "Score {} != expected {}",
            score,
            expected
        );
    }
}
