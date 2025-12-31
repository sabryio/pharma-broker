//! Embedding Cache Module
//!
//! Ported from legacy/parsing/embedding.go
//!
//! Manages in-memory embeddings and synonym index for fast medication matching.
//! Provides O(1) lookup for embeddings and synonym relationships.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::domain::MedicationMapping;

// =============================================================================
// Synonym Index
// =============================================================================

/// Index for fast synonym lookups
/// Maps each medication name to its canonical form and all synonyms
#[derive(Debug, Default)]
pub struct SynonymIndex {
    /// Maps any name (Arabic, English, synonym) to canonical English name
    name_to_canonical: HashMap<String, String>,
    /// Maps canonical name to all its synonyms
    canonical_to_synonyms: HashMap<String, HashSet<String>>,
}

impl SynonymIndex {
    /// Create a new synonym index from medication mappings
    pub fn new(mappings: &[MedicationMapping]) -> Self {
        let mut name_to_canonical = HashMap::new();
        let mut canonical_to_synonyms: HashMap<String, HashSet<String>> = HashMap::new();

        for mapping in mappings {
            let canonical = mapping.english_name.to_lowercase();

            // Map Arabic name
            if !mapping.arabic_name.is_empty() {
                let arabic_lower = mapping.arabic_name.to_lowercase();
                name_to_canonical.insert(arabic_lower.clone(), canonical.clone());
                canonical_to_synonyms
                    .entry(canonical.clone())
                    .or_default()
                    .insert(arabic_lower);
            }

            // Map English name
            if !mapping.english_name.is_empty() {
                let english_lower = mapping.english_name.to_lowercase();
                name_to_canonical.insert(english_lower.clone(), canonical.clone());
                canonical_to_synonyms
                    .entry(canonical.clone())
                    .or_default()
                    .insert(english_lower);
            }

            // Map all synonyms
            if let Some(synonyms) = &mapping.synonyms {
                for synonym in synonyms {
                    if !synonym.is_empty() {
                        let syn_lower = synonym.to_lowercase();
                        name_to_canonical.insert(syn_lower.clone(), canonical.clone());
                        canonical_to_synonyms
                            .entry(canonical.clone())
                            .or_default()
                            .insert(syn_lower);
                    }
                }
            }
        }

        Self {
            name_to_canonical,
            canonical_to_synonyms,
        }
    }

    /// Check if two terms are synonyms
    /// Ported from Go: SynonymIndex.AreSynonyms
    pub fn are_synonyms(&self, term1: &str, term2: &str) -> bool {
        let t1_lower = term1.to_lowercase();
        let t2_lower = term2.to_lowercase();

        // Same term
        if t1_lower == t2_lower {
            return true;
        }

        // Check if they map to the same canonical form
        match (
            self.name_to_canonical.get(&t1_lower),
            self.name_to_canonical.get(&t2_lower),
        ) {
            (Some(c1), Some(c2)) => c1 == c2,
            _ => false,
        }
    }

    /// Get the canonical (English) name for a term
    pub fn get_canonical(&self, term: &str) -> Option<&String> {
        self.name_to_canonical.get(&term.to_lowercase())
    }

    /// Get all synonyms for a canonical name
    pub fn get_synonyms(&self, canonical: &str) -> Option<&HashSet<String>> {
        self.canonical_to_synonyms.get(&canonical.to_lowercase())
    }

    /// Get the number of unique medications
    pub fn size(&self) -> usize {
        self.canonical_to_synonyms.len()
    }

    /// Get total number of name mappings
    pub fn total_mappings(&self) -> usize {
        self.name_to_canonical.len()
    }
}

// =============================================================================
// Embedding Cache
// =============================================================================

/// In-memory cache for medication embeddings and synonyms
/// Ported from Go: EmbeddingCache (embedding.go:17-24)
pub struct EmbeddingCache {
    /// Maps medication name (lowercase) to embedding vector
    embeddings: RwLock<HashMap<String, Vec<f32>>>,
    /// Synonym index for fast lookups
    synonym_index: RwLock<Option<SynonymIndex>>,
    /// Statistics
    stats: EmbeddingCacheStats,
}

/// Statistics for the embedding cache
#[derive(Debug, Default)]
struct EmbeddingCacheStats {
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
}

impl Default for EmbeddingCache {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingCache {
    /// Create a new empty embedding cache
    pub fn new() -> Self {
        Self {
            embeddings: RwLock::new(HashMap::new()),
            synonym_index: RwLock::new(None),
            stats: EmbeddingCacheStats::default(),
        }
    }

    /// Refresh the cache from medication mappings
    /// Ported from Go: EmbeddingCache.Refresh (embedding.go:32-62)
    pub fn refresh(&self, mappings: &[MedicationMapping]) {
        let mut new_embeddings = HashMap::new();
        let mut count = 0;

        for mapping in mappings {
            // Get embedding as Vec<f32>, skip if none
            let embedding = match mapping.get_embedding() {
                Some(e) if !e.is_empty() => e,
                _ => continue,
            };

            // Map all known names to this embedding
            if !mapping.arabic_name.is_empty() {
                new_embeddings.insert(mapping.arabic_name.to_lowercase(), embedding.clone());
            }
            if !mapping.english_name.is_empty() {
                new_embeddings.insert(mapping.english_name.to_lowercase(), embedding.clone());
            }
            if let Some(synonyms) = &mapping.synonyms {
                for synonym in synonyms {
                    if !synonym.is_empty() {
                        new_embeddings.insert(synonym.to_lowercase(), embedding.clone());
                    }
                }
            }
            count += 1;
        }

        // Build synonym index
        let synonym_index = SynonymIndex::new(mappings);

        // Update cache atomically
        *self.embeddings.write().unwrap() = new_embeddings;
        *self.synonym_index.write().unwrap() = Some(synonym_index);

        let embedding_keys = self.embeddings.read().unwrap().len();
        let synonym_size = self
            .synonym_index
            .read()
            .unwrap()
            .as_ref()
            .map_or(0, |s| s.size());
        let synonym_mappings = self
            .synonym_index
            .read()
            .unwrap()
            .as_ref()
            .map_or(0, |s| s.total_mappings());

        tracing::info!(
            embeddings = count,
            embedding_keys = embedding_keys,
            synonym_medications = synonym_size,
            synonym_mappings = synonym_mappings,
            "📊 Refreshed in-memory embeddings and synonym index"
        );
    }

    /// Get the embedding vector for a term
    /// Ported from Go: EmbeddingCache.GetEmbedding (embedding.go:65-69)
    pub fn get_embedding(&self, term: &str) -> Option<Vec<f32>> {
        let embeddings = self.embeddings.read().unwrap();
        let result = embeddings.get(&term.to_lowercase()).cloned();

        if result.is_some() {
            self.stats
                .hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.stats
                .misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        result
    }

    /// Check if two terms are synonyms
    /// Ported from Go: EmbeddingCache.AreSynonyms (embedding.go:72-78)
    pub fn are_synonyms(&self, term1: &str, term2: &str) -> bool {
        let index = self.synonym_index.read().unwrap();
        match index.as_ref() {
            Some(idx) => idx.are_synonyms(term1, term2),
            None => false,
        }
    }

    /// Get the canonical name for a term
    pub fn get_canonical(&self, term: &str) -> Option<String> {
        let index = self.synonym_index.read().unwrap();
        index.as_ref()?.get_canonical(term).cloned()
    }

    /// Get all synonyms for a term
    pub fn get_all_synonyms(&self, term: &str) -> Vec<String> {
        let index = self.synonym_index.read().unwrap();
        if let Some(idx) = index.as_ref()
            && let Some(canonical) = idx.get_canonical(term)
            && let Some(synonyms) = idx.get_synonyms(canonical)
        {
            return synonyms.iter().cloned().collect();
        }
        Vec::new()
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> EmbeddingCacheStatsSnapshot {
        let embeddings = self.embeddings.read().unwrap();
        let index = self.synonym_index.read().unwrap();

        EmbeddingCacheStatsSnapshot {
            embedding_count: embeddings.len(),
            medication_count: index.as_ref().map_or(0, |i| i.size()),
            synonym_mappings: index.as_ref().map_or(0, |i| i.total_mappings()),
            cache_hits: self.stats.hits.load(std::sync::atomic::Ordering::Relaxed),
            cache_misses: self.stats.misses.load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.embeddings.read().unwrap().is_empty()
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.embeddings.write().unwrap().clear();
        *self.synonym_index.write().unwrap() = None;
    }
}

/// Snapshot of cache statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingCacheStatsSnapshot {
    pub embedding_count: usize,
    pub medication_count: usize,
    pub synonym_mappings: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl EmbeddingCacheStatsSnapshot {
    /// Calculate cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64 * 100.0
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_mappings() -> Vec<MedicationMapping> {
        let now = Utc::now();
        vec![
            MedicationMapping {
                id: Uuid::new_v4(),
                arabic_name: "بروفين".to_string(),
                english_name: "Brufen".to_string(),
                synonyms: Some(vec!["Ibuprofen".to_string(), "ايبوبروفين".to_string()]),
                embedding: Some(pgvector::Vector::from(vec![0.1, 0.2, 0.3])),
                created_at: now,
                updated_at: now,
            },
            MedicationMapping {
                id: Uuid::new_v4(),
                arabic_name: "بنادول".to_string(),
                english_name: "Panadol".to_string(),
                synonyms: Some(vec!["Paracetamol".to_string()]),
                embedding: Some(pgvector::Vector::from(vec![0.4, 0.5, 0.6])),
                created_at: now,
                updated_at: now,
            },
            MedicationMapping {
                id: Uuid::new_v4(),
                arabic_name: "أوجمنتين".to_string(),
                english_name: "Augmentin".to_string(),
                synonyms: None,
                embedding: None, // No embedding
                created_at: now,
                updated_at: now,
            },
        ]
    }

    // =========================================================================
    // Synonym Index Tests
    // =========================================================================

    #[test]
    fn test_synonym_index_creation() {
        let mappings = create_test_mappings();
        let index = SynonymIndex::new(&mappings);

        assert_eq!(index.size(), 3); // 3 unique medications
        assert!(index.total_mappings() > 0);
    }

    #[test]
    fn test_are_synonyms_same_term() {
        let mappings = create_test_mappings();
        let index = SynonymIndex::new(&mappings);

        assert!(index.are_synonyms("Brufen", "brufen"));
        assert!(index.are_synonyms("بروفين", "بروفين"));
    }

    #[test]
    fn test_are_synonyms_different_names() {
        let mappings = create_test_mappings();
        let index = SynonymIndex::new(&mappings);

        // Arabic and English names of same medication
        assert!(index.are_synonyms("بروفين", "Brufen"));
        assert!(index.are_synonyms("Brufen", "Ibuprofen"));
        assert!(index.are_synonyms("بروفين", "Ibuprofen"));
    }

    #[test]
    fn test_are_synonyms_different_medications() {
        let mappings = create_test_mappings();
        let index = SynonymIndex::new(&mappings);

        // Different medications should not be synonyms
        assert!(!index.are_synonyms("Brufen", "Panadol"));
        assert!(!index.are_synonyms("بروفين", "بنادول"));
    }

    #[test]
    fn test_get_canonical() {
        let mappings = create_test_mappings();
        let index = SynonymIndex::new(&mappings);

        assert_eq!(index.get_canonical("بروفين"), Some(&"brufen".to_string()));
        assert_eq!(
            index.get_canonical("Ibuprofen"),
            Some(&"brufen".to_string())
        );
        assert_eq!(index.get_canonical("unknown"), None);
    }

    // =========================================================================
    // Embedding Cache Tests
    // =========================================================================

    #[test]
    fn test_cache_refresh() {
        let cache = EmbeddingCache::new();
        assert!(cache.is_empty());

        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        assert!(!cache.is_empty());
    }

    #[test]
    fn test_get_embedding() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        // Get by Arabic name
        let emb = cache.get_embedding("بروفين");
        assert!(emb.is_some());
        assert_eq!(emb.unwrap(), vec![0.1, 0.2, 0.3]);

        // Get by English name
        let emb = cache.get_embedding("Brufen");
        assert!(emb.is_some());

        // Get by synonym
        let emb = cache.get_embedding("Ibuprofen");
        assert!(emb.is_some());

        // Case insensitive
        let emb = cache.get_embedding("BRUFEN");
        assert!(emb.is_some());

        // Unknown term
        let emb = cache.get_embedding("Unknown");
        assert!(emb.is_none());
    }

    #[test]
    fn test_get_embedding_no_embedding() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        // Augmentin has no embedding
        let emb = cache.get_embedding("Augmentin");
        assert!(emb.is_none());
    }

    #[test]
    fn test_are_synonyms_via_cache() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        assert!(cache.are_synonyms("بروفين", "Brufen"));
        assert!(cache.are_synonyms("Brufen", "Ibuprofen"));
        assert!(!cache.are_synonyms("Brufen", "Panadol"));
    }

    #[test]
    fn test_get_all_synonyms() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        let synonyms = cache.get_all_synonyms("Brufen");
        assert!(!synonyms.is_empty());
        assert!(synonyms.contains(&"brufen".to_string()));
        assert!(synonyms.contains(&"ibuprofen".to_string()));
    }

    #[test]
    fn test_cache_stats() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        // Generate some hits and misses
        cache.get_embedding("Brufen"); // Hit
        cache.get_embedding("Panadol"); // Hit
        cache.get_embedding("Unknown"); // Miss

        let stats = cache.get_stats();
        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.cache_misses, 1);
        assert!((stats.hit_rate() - 66.67).abs() < 1.0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = EmbeddingCache::new();
        let mappings = create_test_mappings();
        cache.refresh(&mappings);

        assert!(!cache.is_empty());

        cache.clear();

        assert!(cache.is_empty());
        assert!(cache.get_embedding("Brufen").is_none());
    }
}
