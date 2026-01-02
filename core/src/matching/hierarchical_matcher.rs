//! Hierarchical Matching Pipeline
//!
//! Replaces single-pass matching with a staged approach that progressively
//! narrows candidates for better performance and accuracy.
//!
//! Stages:
//! 1. Exact Match (master_medication_id) - O(1)
//! 2. Alias Lookup - O(1) hash lookup
//! 3. FTS + Trigram - O(log n) index scan
//! 4. Embedding Similarity - top-k from Stage 3
//! 5. Fuzzy + Raw Validation - final scoring

use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::Offer;

/// Configuration for hierarchical matching
#[derive(Debug, Clone)]
pub struct HierarchicalConfig {
    pub enable_exact_match: bool,
    pub enable_alias_lookup: bool,
    pub enable_fts_search: bool,
    pub enable_embedding_search: bool,
    pub enable_fuzzy_validation: bool,
    pub fts_min_score: f64,
    pub embedding_min_similarity: f64,
    pub fuzzy_min_similarity: f64,
    pub fts_max_candidates: i64,
    pub embedding_top_k: i64,
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            enable_exact_match: true,
            enable_alias_lookup: true,
            enable_fts_search: true,
            enable_embedding_search: true,
            enable_fuzzy_validation: true,
            fts_min_score: 0.3,
            embedding_min_similarity: 0.7,
            fuzzy_min_similarity: 0.6,
            fts_max_candidates: 20,
            embedding_top_k: 10,
        }
    }
}

impl HierarchicalConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            enable_exact_match: std::env::var("HIERARCHICAL_EXACT_MATCH")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_alias_lookup: std::env::var("HIERARCHICAL_ALIAS_LOOKUP")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_fts_search: std::env::var("HIERARCHICAL_FTS_SEARCH")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_embedding_search: std::env::var("HIERARCHICAL_EMBEDDING_SEARCH")
                .map(|v| v != "false")
                .unwrap_or(true),
            enable_fuzzy_validation: std::env::var("HIERARCHICAL_FUZZY_VALIDATION")
                .map(|v| v != "false")
                .unwrap_or(true),
            fts_min_score: std::env::var("HIERARCHICAL_FTS_MIN_SCORE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            embedding_min_similarity: std::env::var("HIERARCHICAL_EMBEDDING_MIN_SIM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.7),
            fuzzy_min_similarity: std::env::var("HIERARCHICAL_FUZZY_MIN_SIM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.6),
            fts_max_candidates: std::env::var("HIERARCHICAL_FTS_MAX_CANDIDATES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            embedding_top_k: std::env::var("HIERARCHICAL_EMBEDDING_TOP_K")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// Match candidate with stage information
#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub offer: Offer,
    pub stage: MatchStage,
    pub stage_score: f64,
    pub final_score: Option<f64>,
}

/// Stage at which the match was found
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStage {
    ExactMatch,
    AliasMatch,
    FTSMatch,
    EmbeddingMatch,
    FuzzyValidated,
}

impl std::fmt::Display for MatchStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchStage::ExactMatch => write!(f, "exact"),
            MatchStage::AliasMatch => write!(f, "alias"),
            MatchStage::FTSMatch => write!(f, "fts"),
            MatchStage::EmbeddingMatch => write!(f, "embedding"),
            MatchStage::FuzzyValidated => write!(f, "fuzzy"),
        }
    }
}

/// Statistics for hierarchical matching
#[derive(Debug, Default)]
pub struct HierarchicalStats {
    pub exact_matches: AtomicU64,
    pub alias_matches: AtomicU64,
    pub fts_matches: AtomicU64,
    pub embedding_matches: AtomicU64,
    pub fuzzy_matches: AtomicU64,
    pub no_matches: AtomicU64,
    pub total_queries: AtomicU64,
}

impl HierarchicalStats {
    pub fn snapshot(&self) -> HierarchicalStatsSnapshot {
        HierarchicalStatsSnapshot {
            exact_matches: self.exact_matches.load(Ordering::Relaxed),
            alias_matches: self.alias_matches.load(Ordering::Relaxed),
            fts_matches: self.fts_matches.load(Ordering::Relaxed),
            embedding_matches: self.embedding_matches.load(Ordering::Relaxed),
            fuzzy_matches: self.fuzzy_matches.load(Ordering::Relaxed),
            no_matches: self.no_matches.load(Ordering::Relaxed),
            total_queries: self.total_queries.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.exact_matches.store(0, Ordering::Relaxed);
        self.alias_matches.store(0, Ordering::Relaxed);
        self.fts_matches.store(0, Ordering::Relaxed);
        self.embedding_matches.store(0, Ordering::Relaxed);
        self.fuzzy_matches.store(0, Ordering::Relaxed);
        self.no_matches.store(0, Ordering::Relaxed);
        self.total_queries.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of hierarchical matching statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HierarchicalStatsSnapshot {
    pub exact_matches: u64,
    pub alias_matches: u64,
    pub fts_matches: u64,
    pub embedding_matches: u64,
    pub fuzzy_matches: u64,
    pub no_matches: u64,
    pub total_queries: u64,
}

impl HierarchicalStatsSnapshot {
    /// Calculate deterministic match rate (exact + alias)
    pub fn deterministic_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        (self.exact_matches + self.alias_matches) as f64 / self.total_queries as f64
    }

    /// Calculate AI-dependent match rate (embedding)
    pub fn ai_dependent_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.embedding_matches as f64 / self.total_queries as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HierarchicalConfig::default();
        assert!(config.enable_exact_match);
        assert!(config.enable_alias_lookup);
        assert!(config.enable_fts_search);
        assert!(config.enable_embedding_search);
        assert!(config.enable_fuzzy_validation);
        assert!((config.fts_min_score - 0.3).abs() < 0.001);
        assert!((config.embedding_min_similarity - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_match_stage_display() {
        assert_eq!(format!("{}", MatchStage::ExactMatch), "exact");
        assert_eq!(format!("{}", MatchStage::AliasMatch), "alias");
        assert_eq!(format!("{}", MatchStage::FTSMatch), "fts");
        assert_eq!(format!("{}", MatchStage::EmbeddingMatch), "embedding");
        assert_eq!(format!("{}", MatchStage::FuzzyValidated), "fuzzy");
    }

    #[test]
    fn test_stats_snapshot() {
        let stats = HierarchicalStats::default();
        stats.exact_matches.store(10, Ordering::Relaxed);
        stats.alias_matches.store(5, Ordering::Relaxed);
        stats.total_queries.store(100, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.exact_matches, 10);
        assert_eq!(snapshot.alias_matches, 5);
        assert_eq!(snapshot.total_queries, 100);
        assert!((snapshot.deterministic_rate() - 0.15).abs() < 0.001);
    }

    #[test]
    fn test_stats_reset() {
        let stats = HierarchicalStats::default();
        stats.exact_matches.store(10, Ordering::Relaxed);
        stats.reset();
        assert_eq!(stats.exact_matches.load(Ordering::Relaxed), 0);
    }
}
