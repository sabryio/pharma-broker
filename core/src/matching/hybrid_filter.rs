//! Hybrid Mapping Filter Module
//!
//! Ported from legacy/ai/docker/provider.go (filterMappingsHybrid)
//!
//! Combines keyword matching and vector similarity for efficient medication
//! mapping filtering. This reduces the context sent to AI models while
//! maintaining high recall.
//!
//! Key features:
//! - Keyword filtering with Arabic normalization
//! - Vector similarity search (top-K)
//! - Hybrid combination for best of both approaches

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::domain::MedicationMapping;
use crate::matching::{arabic, cosine_similarity};

// =============================================================================
// Configuration
// =============================================================================

/// Default top-K for vector search
pub const DEFAULT_VECTOR_TOP_K: usize = 10;

/// Configuration for hybrid mapping filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridFilterConfig {
    /// Enable keyword filtering
    pub enable_keyword_filter: bool,
    /// Enable vector similarity filtering
    pub enable_vector_filter: bool,
    /// Top-K results for vector search
    pub vector_top_k: usize,
    /// Minimum similarity score for vector matches
    pub min_similarity: f64,
    /// Include synonyms in keyword matching
    pub include_synonyms: bool,
}

impl Default for HybridFilterConfig {
    fn default() -> Self {
        Self {
            enable_keyword_filter: true,
            enable_vector_filter: true,
            vector_top_k: DEFAULT_VECTOR_TOP_K,
            min_similarity: 0.0, // No minimum by default
            include_synonyms: true,
        }
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics for hybrid filter operations
#[derive(Debug, Default)]
pub struct HybridFilterStats {
    /// Total filter operations
    total_operations: AtomicU64,
    /// Keyword matches found
    keyword_matches: AtomicU64,
    /// Vector matches found
    vector_matches: AtomicU64,
    /// Total mappings before filtering
    total_input_mappings: AtomicU64,
    /// Total mappings after filtering
    total_output_mappings: AtomicU64,
}

impl HybridFilterStats {
    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> HybridFilterStatsSnapshot {
        HybridFilterStatsSnapshot {
            total_operations: self.total_operations.load(Ordering::Relaxed),
            keyword_matches: self.keyword_matches.load(Ordering::Relaxed),
            vector_matches: self.vector_matches.load(Ordering::Relaxed),
            total_input_mappings: self.total_input_mappings.load(Ordering::Relaxed),
            total_output_mappings: self.total_output_mappings.load(Ordering::Relaxed),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_operations.store(0, Ordering::Relaxed);
        self.keyword_matches.store(0, Ordering::Relaxed);
        self.vector_matches.store(0, Ordering::Relaxed);
        self.total_input_mappings.store(0, Ordering::Relaxed);
        self.total_output_mappings.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of filter statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridFilterStatsSnapshot {
    pub total_operations: u64,
    pub keyword_matches: u64,
    pub vector_matches: u64,
    pub total_input_mappings: u64,
    pub total_output_mappings: u64,
}

impl HybridFilterStatsSnapshot {
    /// Calculate average reduction ratio
    pub fn reduction_ratio(&self) -> f64 {
        if self.total_input_mappings == 0 {
            return 0.0;
        }
        1.0 - (self.total_output_mappings as f64 / self.total_input_mappings as f64)
    }
}

// =============================================================================
// Hybrid Mapping Filter
// =============================================================================

/// Hybrid mapping filter combining keyword and vector search
/// Ported from Go: filterMappingsHybrid (provider.go:920-940)
pub struct HybridMappingFilter {
    config: std::sync::RwLock<HybridFilterConfig>,
    stats: HybridFilterStats,
}

impl Default for HybridMappingFilter {
    fn default() -> Self {
        Self::new(HybridFilterConfig::default())
    }
}

impl HybridMappingFilter {
    /// Create a new hybrid mapping filter
    pub fn new(config: HybridFilterConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            stats: HybridFilterStats::default(),
        }
    }

    // =========================================================================
    // Core Filtering Methods
    // =========================================================================

    /// Filter mappings using hybrid approach (keyword + vector)
    /// Ported from Go: filterMappingsHybrid (provider.go:920-940)
    pub fn filter(
        &self,
        content: &str,
        mappings: &[MedicationMapping],
        content_embedding: Option<&[f32]>,
    ) -> HashMap<String, String> {
        let config = self.config.read().unwrap();

        self.stats.total_operations.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_input_mappings
            .fetch_add(mappings.len() as u64, Ordering::Relaxed);

        let mut result = HashMap::new();

        // Step 1: Keyword filtering (always)
        if config.enable_keyword_filter {
            let keyword_matches =
                self.filter_by_keyword(content, mappings, config.include_synonyms);
            self.stats
                .keyword_matches
                .fetch_add(keyword_matches.len() as u64, Ordering::Relaxed);
            result.extend(keyword_matches);
        }

        // Step 2: Vector similarity (if embedding provided)
        if config.enable_vector_filter
            && let Some(embedding) = content_embedding
        {
            let vector_matches = self.filter_by_similarity(
                embedding,
                mappings,
                config.vector_top_k,
                config.min_similarity,
            );

            // Count new matches (not already in keyword results)
            let new_matches = vector_matches
                .iter()
                .filter(|(k, _)| !result.contains_key(*k))
                .count();
            self.stats
                .vector_matches
                .fetch_add(new_matches as u64, Ordering::Relaxed);

            // Merge (keyword matches take priority)
            for (arabic, english) in vector_matches {
                result.entry(arabic).or_insert(english);
            }
        }

        self.stats
            .total_output_mappings
            .fetch_add(result.len() as u64, Ordering::Relaxed);

        tracing::debug!(
            input_mappings = mappings.len(),
            output_mappings = result.len(),
            reduction = format!(
                "{:.1}%",
                (1.0 - result.len() as f64 / mappings.len() as f64) * 100.0
            ),
            "🔍 Hybrid filter applied"
        );

        result
    }

    /// Filter mappings by keyword matching
    /// Ported from Go: filterMappingsByKeyword (provider.go:870-882)
    pub fn filter_by_keyword(
        &self,
        content: &str,
        mappings: &[MedicationMapping],
        include_synonyms: bool,
    ) -> HashMap<String, String> {
        let mut result = HashMap::new();
        let content_normalized = arabic::normalize_for_matching(content);

        for mapping in mappings {
            // Check Arabic name
            let arabic_normalized = arabic::normalize_for_matching(&mapping.arabic_name);
            if content_normalized.contains(&arabic_normalized) {
                result.insert(mapping.arabic_name.clone(), mapping.english_name.clone());
                continue;
            }

            // Check English name (case-insensitive)
            if content_normalized.contains(&mapping.english_name.to_lowercase()) {
                result.insert(mapping.arabic_name.clone(), mapping.english_name.clone());
                continue;
            }

            // Check synonyms
            if include_synonyms && let Some(synonyms) = &mapping.synonyms {
                for synonym in synonyms {
                    let syn_normalized = arabic::normalize_for_matching(synonym);
                    if content_normalized.contains(&syn_normalized) {
                        result.insert(mapping.arabic_name.clone(), mapping.english_name.clone());
                        break;
                    }
                }
            }
        }

        result
    }

    /// Filter mappings by vector similarity
    /// Ported from Go: filterMappingsBySimilarity (provider.go:885-915)
    pub fn filter_by_similarity(
        &self,
        content_embedding: &[f32],
        mappings: &[MedicationMapping],
        top_k: usize,
        min_similarity: f64,
    ) -> HashMap<String, String> {
        if mappings.is_empty() || top_k == 0 {
            return HashMap::new();
        }

        // Score all mappings by similarity
        let mut scored: Vec<(f64, &MedicationMapping)> = mappings
            .iter()
            .filter_map(|m| {
                let embedding = m.get_embedding()?;
                if embedding.is_empty() {
                    return None;
                }
                let score = cosine_similarity(content_embedding, &embedding).ok()?;
                if score >= min_similarity {
                    Some((score, m))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-K
        scored
            .into_iter()
            .take(top_k)
            .map(|(_, m)| (m.arabic_name.clone(), m.english_name.clone()))
            .collect()
    }

    // =========================================================================
    // Utility Methods
    // =========================================================================

    /// Build a full mapping map including synonyms
    /// Ported from Go: buildFullMappingMap (provider.go:943-952)
    pub fn build_full_mapping_map(mappings: &[MedicationMapping]) -> HashMap<String, String> {
        let mut full_map = HashMap::new();

        for m in mappings {
            full_map.insert(m.arabic_name.clone(), m.english_name.clone());

            if let Some(synonyms) = &m.synonyms {
                for syn in synonyms {
                    full_map.insert(syn.clone(), m.english_name.clone());
                }
            }
        }

        full_map
    }

    /// Convert HashMap to Vec<MedicationMapping> (for API compatibility)
    pub fn map_to_mappings(map: &HashMap<String, String>) -> Vec<MedicationMapping> {
        map.iter()
            .map(|(arabic, english)| MedicationMapping {
                id: String::new(),
                arabic_name: arabic.clone(),
                english_name: english.clone(),
                synonyms: None,
                embedding: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .collect()
    }

    // =========================================================================
    // Configuration & Statistics
    // =========================================================================

    /// Get current configuration
    pub fn get_config(&self) -> HybridFilterConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: HybridFilterConfig) {
        tracing::info!(
            keyword_filter = config.enable_keyword_filter,
            vector_filter = config.enable_vector_filter,
            vector_top_k = config.vector_top_k,
            "Hybrid filter configuration updated"
        );
        *self.config.write().unwrap() = config;
    }

    /// Get statistics snapshot
    pub fn get_stats(&self) -> HybridFilterStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rstest::rstest;

    fn create_test_mappings() -> Vec<MedicationMapping> {
        let now = Utc::now();
        vec![
            MedicationMapping {
                id: "1".to_string(),
                arabic_name: "بروفين".to_string(),
                english_name: "Brufen".to_string(),
                synonyms: Some(vec!["Ibuprofen".to_string(), "ايبوبروفين".to_string()]),
                embedding: Some(pgvector::Vector::from(vec![0.9, 0.1, 0.0])),
                created_at: now,
                updated_at: now,
            },
            MedicationMapping {
                id: "2".to_string(),
                arabic_name: "بنادول".to_string(),
                english_name: "Panadol".to_string(),
                synonyms: Some(vec!["Paracetamol".to_string()]),
                embedding: Some(pgvector::Vector::from(vec![0.1, 0.9, 0.0])),
                created_at: now,
                updated_at: now,
            },
            MedicationMapping {
                id: "3".to_string(),
                arabic_name: "أوجمنتين".to_string(),
                english_name: "Augmentin".to_string(),
                synonyms: None,
                embedding: Some(pgvector::Vector::from(vec![0.0, 0.1, 0.9])),
                created_at: now,
                updated_at: now,
            },
            MedicationMapping {
                id: "4".to_string(),
                arabic_name: "فلاجيل".to_string(),
                english_name: "Flagyl".to_string(),
                synonyms: None,
                embedding: None, // No embedding
                created_at: now,
                updated_at: now,
            },
        ]
    }

    // =========================================================================
    // Keyword Filter Tests
    // =========================================================================

    #[test]
    fn test_keyword_filter_arabic_match() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("أريد بروفين 400", &mappings, true);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("بروفين"), Some(&"Brufen".to_string()));
    }

    #[test]
    fn test_keyword_filter_english_match() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("I need Panadol 500mg", &mappings, true);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("بنادول"), Some(&"Panadol".to_string()));
    }

    #[test]
    fn test_keyword_filter_synonym_match() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("Looking for Ibuprofen", &mappings, true);

        assert_eq!(result.len(), 1);
        assert_eq!(result.get("بروفين"), Some(&"Brufen".to_string()));
    }

    #[test]
    fn test_keyword_filter_no_synonyms() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("Looking for Ibuprofen", &mappings, false);

        assert!(result.is_empty());
    }

    #[test]
    fn test_keyword_filter_multiple_matches() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("بروفين و بنادول متوفر", &mappings, true);

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("بروفين"));
        assert!(result.contains_key("بنادول"));
    }

    #[test]
    fn test_keyword_filter_no_match() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter_by_keyword("Hello world", &mappings, true);

        assert!(result.is_empty());
    }

    // =========================================================================
    // Vector Similarity Filter Tests
    // =========================================================================

    #[test]
    fn test_similarity_filter_top_k() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        // Embedding similar to Brufen
        let content_embedding = vec![0.85, 0.15, 0.0];

        let result = filter.filter_by_similarity(&content_embedding, &mappings, 2, 0.0);

        assert_eq!(result.len(), 2);
        // Brufen should be first (highest similarity)
        assert!(result.contains_key("بروفين"));
    }

    #[test]
    fn test_similarity_filter_min_threshold() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        // Embedding similar to Brufen
        let content_embedding = vec![0.85, 0.15, 0.0];

        let result = filter.filter_by_similarity(&content_embedding, &mappings, 10, 0.9);

        // Only Brufen should pass the high threshold
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("بروفين"));
    }

    #[test]
    fn test_similarity_filter_skips_no_embedding() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let content_embedding = vec![0.0, 0.0, 1.0]; // Similar to Augmentin

        let result = filter.filter_by_similarity(&content_embedding, &mappings, 10, 0.0);

        // Flagyl has no embedding, should not appear
        assert!(!result.contains_key("فلاجيل"));
    }

    // =========================================================================
    // Hybrid Filter Tests
    // =========================================================================

    #[test]
    fn test_hybrid_filter_combines_results() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        // Content mentions Brufen, embedding similar to Panadol
        let content = "أريد بروفين";
        let content_embedding = vec![0.1, 0.9, 0.0]; // Similar to Panadol

        let result = filter.filter(content, &mappings, Some(&content_embedding));

        // Should have both: Brufen (keyword) and Panadol (vector)
        assert!(result.contains_key("بروفين")); // Keyword match
        assert!(result.contains_key("بنادول")); // Vector match
    }

    #[test]
    fn test_hybrid_filter_keyword_only() {
        let config = HybridFilterConfig {
            enable_keyword_filter: true,
            enable_vector_filter: false,
            ..Default::default()
        };
        let filter = HybridMappingFilter::new(config);
        let mappings = create_test_mappings();

        let content_embedding = vec![0.1, 0.9, 0.0];
        let result = filter.filter("بروفين", &mappings, Some(&content_embedding));

        // Only keyword match
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("بروفين"));
    }

    #[test]
    fn test_hybrid_filter_vector_only() {
        let config = HybridFilterConfig {
            enable_keyword_filter: false,
            enable_vector_filter: true,
            vector_top_k: 2,
            ..Default::default()
        };
        let filter = HybridMappingFilter::new(config);
        let mappings = create_test_mappings();

        let content_embedding = vec![0.9, 0.1, 0.0]; // Similar to Brufen
        let result = filter.filter("random text", &mappings, Some(&content_embedding));

        // Only vector matches
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_hybrid_filter_no_embedding() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let result = filter.filter("بروفين", &mappings, None);

        // Only keyword match (no vector search without embedding)
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("بروفين"));
    }

    // =========================================================================
    // Utility Tests
    // =========================================================================

    #[test]
    fn test_build_full_mapping_map() {
        let mappings = create_test_mappings();
        let full_map = HybridMappingFilter::build_full_mapping_map(&mappings);

        // Should include Arabic names
        assert!(full_map.contains_key("بروفين"));
        assert!(full_map.contains_key("بنادول"));

        // Should include synonyms
        assert!(full_map.contains_key("Ibuprofen"));
        assert!(full_map.contains_key("Paracetamol"));

        // All should map to English names
        assert_eq!(full_map.get("Ibuprofen"), Some(&"Brufen".to_string()));
    }

    #[test]
    fn test_map_to_mappings() {
        let mut map = HashMap::new();
        map.insert("بروفين".to_string(), "Brufen".to_string());
        map.insert("بنادول".to_string(), "Panadol".to_string());

        let mappings = HybridMappingFilter::map_to_mappings(&map);

        assert_eq!(mappings.len(), 2);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_statistics_tracking() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        let content_embedding = vec![0.9, 0.1, 0.0];
        filter.filter("بروفين", &mappings, Some(&content_embedding));

        let stats = filter.get_stats();
        assert_eq!(stats.total_operations, 1);
        assert!(stats.keyword_matches > 0);
        assert_eq!(stats.total_input_mappings, 4);
    }

    #[test]
    fn test_statistics_reset() {
        let filter = HybridMappingFilter::default();
        let mappings = create_test_mappings();

        filter.filter("بروفين", &mappings, None);
        assert!(filter.get_stats().total_operations > 0);

        filter.reset_stats();
        assert_eq!(filter.get_stats().total_operations, 0);
    }

    #[rstest]
    #[case(100, 50, 0.5)]
    #[case(100, 100, 0.0)]
    #[case(100, 0, 1.0)]
    #[case(0, 0, 0.0)]
    fn test_reduction_ratio(#[case] input: u64, #[case] output: u64, #[case] expected_ratio: f64) {
        let stats = HybridFilterStatsSnapshot {
            total_operations: 1,
            keyword_matches: 0,
            vector_matches: 0,
            total_input_mappings: input,
            total_output_mappings: output,
        };

        assert!((stats.reduction_ratio() - expected_ratio).abs() < 0.001);
    }

    // =========================================================================
    // Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = HybridFilterConfig::default();

        assert!(config.enable_keyword_filter);
        assert!(config.enable_vector_filter);
        assert_eq!(config.vector_top_k, DEFAULT_VECTOR_TOP_K);
        assert!(config.include_synonyms);
    }

    #[test]
    fn test_config_update() {
        let filter = HybridMappingFilter::default();

        let new_config = HybridFilterConfig {
            vector_top_k: 20,
            min_similarity: 0.5,
            ..Default::default()
        };
        filter.set_config(new_config);

        let config = filter.get_config();
        assert_eq!(config.vector_top_k, 20);
        assert_eq!(config.min_similarity, 0.5);
    }
}
