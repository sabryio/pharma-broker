//! Priority detection service with caching
//!
//! Detects priority medications and calculates priority scores efficiently.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use pharma_db::Result;
use pharma_db::traits::PriorityMedicationRepository;

/// Cache entry with TTL
#[derive(Debug, Clone)]
struct CacheEntry {
    score: i32,
    cached_at: Instant,
}

/// Priority detector with caching
pub struct PriorityDetector {
    repo: Arc<dyn PriorityMedicationRepository>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: Duration,
}

impl PriorityDetector {
    /// Create a new priority detector
    pub fn new(repo: Arc<dyn PriorityMedicationRepository>) -> Self {
        Self {
            repo,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(60), // 60 second TTL
        }
    }

    /// Create with custom cache TTL
    pub fn with_ttl(repo: Arc<dyn PriorityMedicationRepository>, ttl: Duration) -> Self {
        Self {
            repo,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: ttl,
        }
    }

    /// Get priority score for a medication (0 if not priority)
    pub async fn get_priority_score(&self, medication: &str) -> Result<i32> {
        let normalized = normalize_medication_name(medication);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&normalized) {
                if entry.cached_at.elapsed() < self.cache_ttl {
                    tracing::trace!(
                        medication = %medication,
                        score = entry.score,
                        "Priority cache hit"
                    );
                    return Ok(entry.score);
                }
            }
        }

        // Cache miss - query database
        let score = self
            .repo
            .get_priority_for_medication(&normalized)
            .await?
            .unwrap_or(0);

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                normalized,
                CacheEntry {
                    score,
                    cached_at: Instant::now(),
                },
            );
        }

        tracing::trace!(
            medication = %medication,
            score = score,
            "Priority cache miss - fetched from DB"
        );

        Ok(score)
    }

    /// Get priority scores for multiple medications (batch operation)
    pub async fn get_priority_scores_batch(
        &self,
        medications: &[String],
    ) -> Result<HashMap<String, i32>> {
        let mut result = HashMap::new();
        let mut uncached = Vec::new();

        // Check cache for each medication
        {
            let cache = self.cache.read().await;
            for medication in medications {
                let normalized = normalize_medication_name(medication);
                if let Some(entry) = cache.get(&normalized) {
                    if entry.cached_at.elapsed() < self.cache_ttl {
                        result.insert(medication.clone(), entry.score);
                        continue;
                    }
                }
                uncached.push(medication.clone());
            }
        }

        // Fetch uncached medications from database
        if !uncached.is_empty() {
            let db_results = self.repo.get_priorities_for_medications(&uncached).await?;

            // Update cache and result
            let mut cache = self.cache.write().await;
            for medication in uncached {
                let normalized = normalize_medication_name(&medication);
                let score = db_results.get(&medication).copied().unwrap_or(0);

                cache.insert(
                    normalized,
                    CacheEntry {
                        score,
                        cached_at: Instant::now(),
                    },
                );
                result.insert(medication, score);
            }
        }

        Ok(result)
    }

    /// Check if a medication is priority (score > 0)
    pub async fn is_priority(&self, medication: &str) -> Result<bool> {
        let score = self.get_priority_score(medication).await?;
        Ok(score > 0)
    }

    /// Clear the cache (call when priorities are updated)
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        tracing::info!("Priority cache cleared");
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total = cache.len();
        let expired = cache
            .values()
            .filter(|entry| entry.cached_at.elapsed() >= self.cache_ttl)
            .count();

        CacheStats {
            total_entries: total,
            expired_entries: expired,
            active_entries: total - expired,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub active_entries: usize,
}

/// Normalize medication name for matching
fn normalize_medication_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_medication_name() {
        assert_eq!(normalize_medication_name("Insulin"), "insulin");
        assert_eq!(normalize_medication_name("  INSULIN  "), "insulin");
        assert_eq!(normalize_medication_name("Insulin  100mg"), "insulin 100mg");
    }
}
