//! Fallback Matcher for AI-Free Matching
//!
//! Provides deterministic matching when AI services are unavailable.
//! Uses the MedicationResolver for staged resolution without AI calls.
//!
//! Matching strategies:
//! - Exact alias lookup (O(1))
//! - Fuzzy string matching (O(log n))
//! - Cached embedding similarity (if available)
//!
//! This matcher is used when the circuit breaker is open.

use std::sync::Arc;
use uuid::Uuid;

use crate::ai::FallbackStrategy;
use crate::matching::arabic::normalize_for_matching;
use crate::matching::medication_resolver::MedicationResolver;
use crate::repository::{OfferModel, RequestModel};

/// Configuration for fallback matching
#[derive(Debug, Clone)]
pub struct FallbackMatcherConfig {
    /// Minimum similarity score to consider a match
    pub min_similarity: f64,
    /// Weight for medication name similarity (primary factor)
    pub medication_weight: f64,
    /// Weight for raw text similarity (validation)
    pub raw_weight: f64,
    /// Whether to use cached embeddings if available
    pub use_cached_embeddings: bool,
    /// Minimum embedding similarity for cached embedding matches
    pub min_embedding_similarity: f64,
}

impl Default for FallbackMatcherConfig {
    fn default() -> Self {
        Self {
            min_similarity: 0.70,
            medication_weight: 0.85,
            raw_weight: 0.15,
            use_cached_embeddings: true,
            min_embedding_similarity: 0.80,
        }
    }
}

impl FallbackMatcherConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            min_similarity: std::env::var("FALLBACK_MIN_SIMILARITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.70),
            medication_weight: std::env::var("FALLBACK_MEDICATION_WEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.85),
            raw_weight: std::env::var("FALLBACK_RAW_WEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.15),
            use_cached_embeddings: std::env::var("FALLBACK_USE_CACHED_EMBEDDINGS")
                .map(|v| v != "false")
                .unwrap_or(true),
            min_embedding_similarity: std::env::var("FALLBACK_MIN_EMBEDDING_SIMILARITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.80),
        }
    }
}

/// Result of fallback matching
#[derive(Debug, Clone)]
pub struct FallbackMatchResult {
    /// The offer ID
    pub offer_id: Uuid,
    /// The request ID
    pub request_id: Uuid,
    /// Match score (0.0 - 1.0)
    pub score: f64,
    /// How the match was determined
    pub method: FallbackMatchMethod,
    /// Reasoning for the match
    pub reasoning: String,
    /// Whether this is a high-confidence match
    pub high_confidence: bool,
}

/// Method used for fallback matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackMatchMethod {
    /// Both resolved to same master medication
    SameMaster,
    /// Exact string match on medication names
    ExactMatch,
    /// Fuzzy string similarity
    FuzzyMatch,
    /// Cached embedding similarity
    CachedEmbedding,
    /// No match found
    NoMatch,
}

impl std::fmt::Display for FallbackMatchMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackMatchMethod::SameMaster => write!(f, "same_master"),
            FallbackMatchMethod::ExactMatch => write!(f, "exact_match"),
            FallbackMatchMethod::FuzzyMatch => write!(f, "fuzzy_match"),
            FallbackMatchMethod::CachedEmbedding => write!(f, "cached_embedding"),
            FallbackMatchMethod::NoMatch => write!(f, "no_match"),
        }
    }
}

/// Fallback matcher for AI-free matching
pub struct FallbackMatcher {
    config: FallbackMatcherConfig,
    resolver: Arc<MedicationResolver>,
}

impl FallbackMatcher {
    /// Create a new fallback matcher
    pub fn new(config: FallbackMatcherConfig, resolver: Arc<MedicationResolver>) -> Self {
        Self { config, resolver }
    }

    /// Create from environment variables
    pub fn from_env(resolver: Arc<MedicationResolver>) -> Self {
        Self::new(FallbackMatcherConfig::from_env(), resolver)
    }

    /// Match an offer against a request using deterministic methods only
    pub async fn match_deterministic(
        &self,
        offer: &OfferModel,
        request: &RequestModel,
    ) -> FallbackMatchResult {
        // Stage 1: Resolve both to master medications
        let offer_resolution = self.resolver.resolve(&offer.medication_raw).await;
        let request_resolution = self.resolver.resolve(&request.medication_raw).await;

        // If both resolve to the same master, it's a high-confidence match
        if let (Some(offer_master), Some(request_master)) = (
            offer_resolution.master_medication_id,
            request_resolution.master_medication_id,
        ) && offer_master == request_master
        {
            let confidence = (offer_resolution.confidence + request_resolution.confidence) / 2.0;
            return FallbackMatchResult {
                offer_id: offer.id,
                request_id: request.id,
                score: confidence,
                method: FallbackMatchMethod::SameMaster,
                reasoning: format!(
                    "Both resolved to same master medication (offer: {:?}, request: {:?})",
                    offer_resolution.method, request_resolution.method
                ),
                high_confidence: true,
            };
        }

        // Stage 2: Direct string comparison
        let offer_normalized = normalize_for_matching(&offer.medication_raw);
        let request_normalized = normalize_for_matching(&request.medication_raw);

        // Exact match
        if offer_normalized == request_normalized {
            return FallbackMatchResult {
                offer_id: offer.id,
                request_id: request.id,
                score: 1.0,
                method: FallbackMatchMethod::ExactMatch,
                reasoning: "Exact match on normalized medication names".to_string(),
                high_confidence: true,
            };
        }

        // Stage 3: Fuzzy string similarity
        let medication_sim = self.calculate_similarity(&offer_normalized, &request_normalized);
        let raw_sim = self.calculate_similarity(&offer.medication_raw, &request.medication_raw);

        // Combined score with weights
        let combined_score =
            medication_sim * self.config.medication_weight + raw_sim * self.config.raw_weight;

        if combined_score >= self.config.min_similarity {
            return FallbackMatchResult {
                offer_id: offer.id,
                request_id: request.id,
                score: combined_score,
                method: FallbackMatchMethod::FuzzyMatch,
                reasoning: format!(
                    "Fuzzy match: medication={:.1}%, raw={:.1}%, combined={:.1}%",
                    medication_sim * 100.0,
                    raw_sim * 100.0,
                    combined_score * 100.0
                ),
                high_confidence: combined_score >= 0.90,
            };
        }

        // No match
        FallbackMatchResult {
            offer_id: offer.id,
            request_id: request.id,
            score: combined_score,
            method: FallbackMatchMethod::NoMatch,
            reasoning: format!(
                "Below threshold: medication={:.1}%, raw={:.1}%, combined={:.1}% (min={:.1}%)",
                medication_sim * 100.0,
                raw_sim * 100.0,
                combined_score * 100.0,
                self.config.min_similarity * 100.0
            ),
            high_confidence: false,
        }
    }

    /// Match using cached embeddings (if available)
    pub async fn match_with_cached_embeddings(
        &self,
        offer: &OfferModel,
        request: &RequestModel,
    ) -> FallbackMatchResult {
        // First try deterministic matching
        let deterministic_result = self.match_deterministic(offer, request).await;

        // If deterministic found a match, return it
        if deterministic_result.method != FallbackMatchMethod::NoMatch {
            return deterministic_result;
        }

        // Try cached embeddings if available and enabled
        if self.config.use_cached_embeddings
            && let (Some(offer_emb), Some(request_emb)) =
                (&offer.content_embedding, &request.content_embedding)
        {
            let embedding_sim =
                crate::matching::cosine_similarity(offer_emb.as_slice(), request_emb.as_slice())
                    .unwrap_or(0.0);

            if embedding_sim >= self.config.min_embedding_similarity {
                return FallbackMatchResult {
                    offer_id: offer.id,
                    request_id: request.id,
                    score: embedding_sim,
                    method: FallbackMatchMethod::CachedEmbedding,
                    reasoning: format!(
                        "Cached embedding similarity: {:.1}%",
                        embedding_sim * 100.0
                    ),
                    high_confidence: embedding_sim >= 0.90,
                };
            }
        }

        // Return the deterministic result (which is NoMatch)
        deterministic_result
    }

    /// Match based on fallback strategy
    pub async fn match_with_strategy(
        &self,
        offer: &OfferModel,
        request: &RequestModel,
        strategy: FallbackStrategy,
    ) -> Option<FallbackMatchResult> {
        match strategy {
            FallbackStrategy::DeterministicOnly => {
                let result = self.match_deterministic(offer, request).await;
                if result.method != FallbackMatchMethod::NoMatch {
                    Some(result)
                } else {
                    None
                }
            }
            FallbackStrategy::CachedEmbeddings => {
                let result = self.match_with_cached_embeddings(offer, request).await;
                if result.method != FallbackMatchMethod::NoMatch {
                    Some(result)
                } else {
                    None
                }
            }
            FallbackStrategy::QueueForLater => {
                // Return a pending result - the match will be processed later
                Some(FallbackMatchResult {
                    offer_id: offer.id,
                    request_id: request.id,
                    score: 0.0,
                    method: FallbackMatchMethod::NoMatch,
                    reasoning: "Queued for later processing when AI recovers".to_string(),
                    high_confidence: false,
                })
            }
            FallbackStrategy::RejectAll => None,
        }
    }

    /// Calculate string similarity using Levenshtein distance
    fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();

        if a_lower == b_lower {
            return 1.0;
        }

        let max_len = a_lower.len().max(b_lower.len());
        if max_len == 0 {
            return 1.0;
        }

        let distance = Self::levenshtein(&a_lower, &b_lower);
        1.0 - (distance as f64 / max_len as f64)
    }

    /// Levenshtein distance calculation
    fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[a_len][b_len]
    }

    /// Get current configuration
    pub fn config(&self) -> &FallbackMatcherConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: FallbackMatcherConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FallbackMatcherConfig::default();
        assert!((config.min_similarity - 0.70).abs() < 0.001);
        assert!((config.medication_weight - 0.85).abs() < 0.001);
        assert!((config.raw_weight - 0.15).abs() < 0.001);
        assert!(config.use_cached_embeddings);
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(FallbackMatcher::levenshtein("kitten", "sitting"), 3);
        assert_eq!(FallbackMatcher::levenshtein("", "abc"), 3);
        assert_eq!(FallbackMatcher::levenshtein("abc", "abc"), 0);
        assert_eq!(FallbackMatcher::levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_fallback_match_method_display() {
        assert_eq!(
            format!("{}", FallbackMatchMethod::SameMaster),
            "same_master"
        );
        assert_eq!(
            format!("{}", FallbackMatchMethod::ExactMatch),
            "exact_match"
        );
        assert_eq!(
            format!("{}", FallbackMatchMethod::FuzzyMatch),
            "fuzzy_match"
        );
        assert_eq!(
            format!("{}", FallbackMatchMethod::CachedEmbedding),
            "cached_embedding"
        );
        assert_eq!(format!("{}", FallbackMatchMethod::NoMatch), "no_match");
    }

    #[test]
    fn test_fallback_strategy_matching() {
        // DeterministicOnly allows matching
        assert!(FallbackStrategy::DeterministicOnly.allows_matching());
        // CachedEmbeddings allows matching
        assert!(FallbackStrategy::CachedEmbeddings.allows_matching());
        // QueueForLater allows matching (returns pending)
        assert!(FallbackStrategy::QueueForLater.allows_matching());
        // RejectAll does not allow matching
        assert!(!FallbackStrategy::RejectAll.allows_matching());
    }
}
