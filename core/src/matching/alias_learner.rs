//! Automated Alias Learning
//!
//! Automatically learns new medication aliases from operator feedback.
//! When operators confirm matches, the system learns that the two medication
//! names refer to the same product and creates aliases for future lookups.

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use crate::matching::arabic::normalize_for_matching;

/// Configuration for alias learning
#[derive(Debug, Clone)]
pub struct AliasLearnerConfig {
    /// Minimum match score to learn from
    pub min_score_threshold: f64,
    /// Confidence assigned to learned aliases
    pub learned_alias_confidence: f64,
    /// Number of confirmations required before creating alias
    pub min_confirmations: u32,
    /// Enable automatic learning
    pub enabled: bool,
    /// Maximum pending confirmations to track
    pub max_pending: usize,
}

impl Default for AliasLearnerConfig {
    fn default() -> Self {
        Self {
            min_score_threshold: 0.85,
            learned_alias_confidence: 0.90,
            min_confirmations: 2,
            enabled: true,
            max_pending: 10000,
        }
    }
}

impl AliasLearnerConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            min_score_threshold: std::env::var("ALIAS_MIN_SCORE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.85),
            learned_alias_confidence: std::env::var("ALIAS_LEARNED_CONFIDENCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.90),
            min_confirmations: std::env::var("ALIAS_MIN_CONFIRMATIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            enabled: std::env::var("ALIAS_LEARNING_ENABLED")
                .map(|v| v != "false")
                .unwrap_or(true),
            max_pending: std::env::var("ALIAS_MAX_PENDING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10000),
        }
    }
}

/// Pending alias confirmation tracking
#[derive(Debug, Clone)]
struct PendingAlias {
    /// Normalized medication name
    normalized_name: String,
    /// Master medication ID to link to
    master_id: Uuid,
    /// Number of confirmations received
    confirmations: u32,
    /// Match IDs that confirmed this alias
    match_ids: Vec<Uuid>,
}

/// Statistics for alias learning
#[derive(Debug, Default)]
pub struct AliasLearnerStats {
    pub aliases_learned: AtomicU64,
    pub aliases_rejected: AtomicU64,
    pub confirmations_received: AtomicU64,
    pub rejections_received: AtomicU64,
}

impl AliasLearnerStats {
    pub fn snapshot(&self) -> AliasLearnerStatsSnapshot {
        AliasLearnerStatsSnapshot {
            aliases_learned: self.aliases_learned.load(Ordering::Relaxed),
            aliases_rejected: self.aliases_rejected.load(Ordering::Relaxed),
            confirmations_received: self.confirmations_received.load(Ordering::Relaxed),
            rejections_received: self.rejections_received.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of alias learner statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AliasLearnerStatsSnapshot {
    pub aliases_learned: u64,
    pub aliases_rejected: u64,
    pub confirmations_received: u64,
    pub rejections_received: u64,
}

/// Result of learning from a confirmation
#[derive(Debug, Clone)]
pub struct LearnResult {
    /// Whether a new alias was created
    pub alias_created: bool,
    /// The normalized alias name
    pub alias_name: String,
    /// The master medication ID
    pub master_id: Option<Uuid>,
    /// Current confirmation count
    pub confirmations: u32,
    /// Required confirmations
    pub required: u32,
}

/// Alias learner service
///
/// Tracks medication name pairs from confirmed matches and creates
/// aliases when enough confirmations are received.
pub struct AliasLearner {
    config: RwLock<AliasLearnerConfig>,
    stats: AliasLearnerStats,
    /// Pending aliases waiting for more confirmations
    /// Key: normalized medication name
    pending: RwLock<HashMap<String, PendingAlias>>,
}

impl Default for AliasLearner {
    fn default() -> Self {
        Self::new(AliasLearnerConfig::default())
    }
}

impl AliasLearner {
    /// Create a new alias learner
    pub fn new(config: AliasLearnerConfig) -> Self {
        Self {
            config: RwLock::new(config),
            stats: AliasLearnerStats::default(),
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(AliasLearnerConfig::from_env())
    }

    /// Learn from a confirmed match
    ///
    /// Returns Some(LearnResult) if learning was attempted, None if disabled or below threshold.
    pub fn learn_from_confirmation(
        &self,
        match_id: Uuid,
        score: f64,
        offer_medication: &str,
        request_medication: &str,
        offer_master_id: Option<Uuid>,
        request_master_id: Option<Uuid>,
    ) -> Option<LearnResult> {
        let config = self.config.read().unwrap();

        if !config.enabled {
            return None;
        }

        if score < config.min_score_threshold {
            return None;
        }

        self.stats
            .confirmations_received
            .fetch_add(1, Ordering::Relaxed);

        // Determine which side is known (has master_id) and which is unknown
        let (known_master_id, unknown_medication) = match (offer_master_id, request_master_id) {
            (Some(master_id), None) => (master_id, request_medication),
            (None, Some(master_id)) => (master_id, offer_medication),
            (Some(_), Some(_)) => {
                // Both known - nothing to learn
                return None;
            }
            (None, None) => {
                // Neither known - can't learn without a reference
                return None;
            }
        };

        let normalized = normalize_for_matching(unknown_medication);
        let min_confirmations = config.min_confirmations;
        let max_pending = config.max_pending;
        drop(config);

        self.maybe_create_alias(
            &normalized,
            known_master_id,
            match_id,
            min_confirmations,
            max_pending,
        )
    }

    /// Internal: track pending alias and create if threshold met
    fn maybe_create_alias(
        &self,
        normalized: &str,
        master_id: Uuid,
        match_id: Uuid,
        min_confirmations: u32,
        max_pending: usize,
    ) -> Option<LearnResult> {
        let mut pending = self.pending.write().unwrap();

        // Check if we already have this pending
        if let Some(entry) = pending.get_mut(normalized) {
            // Verify it's for the same master
            if entry.master_id != master_id {
                // Conflicting master IDs - reject
                tracing::warn!(
                    alias = normalized,
                    existing_master = %entry.master_id,
                    new_master = %master_id,
                    "Conflicting master IDs for alias, rejecting"
                );
                self.stats.aliases_rejected.fetch_add(1, Ordering::Relaxed);
                pending.remove(normalized);
                return Some(LearnResult {
                    alias_created: false,
                    alias_name: normalized.to_string(),
                    master_id: None,
                    confirmations: 0,
                    required: min_confirmations,
                });
            }

            entry.confirmations += 1;
            entry.match_ids.push(match_id);

            if entry.confirmations >= min_confirmations {
                // Threshold met - create alias
                self.stats.aliases_learned.fetch_add(1, Ordering::Relaxed);
                let result = LearnResult {
                    alias_created: true,
                    alias_name: normalized.to_string(),
                    master_id: Some(master_id),
                    confirmations: entry.confirmations,
                    required: min_confirmations,
                };
                pending.remove(normalized);

                tracing::info!(
                    alias = normalized,
                    master_id = %master_id,
                    confirmations = result.confirmations,
                    "Learned new medication alias"
                );

                return Some(result);
            }

            return Some(LearnResult {
                alias_created: false,
                alias_name: normalized.to_string(),
                master_id: Some(master_id),
                confirmations: entry.confirmations,
                required: min_confirmations,
            });
        }

        // New pending alias
        if pending.len() >= max_pending {
            // Evict oldest entry (simple LRU would be better but this is simpler)
            if let Some(key) = pending.keys().next().cloned() {
                pending.remove(&key);
            }
        }

        pending.insert(
            normalized.to_string(),
            PendingAlias {
                normalized_name: normalized.to_string(),
                master_id,
                confirmations: 1,
                match_ids: vec![match_id],
            },
        );

        Some(LearnResult {
            alias_created: false,
            alias_name: normalized.to_string(),
            master_id: Some(master_id),
            confirmations: 1,
            required: min_confirmations,
        })
    }

    /// Learn from a rejected match (negative example)
    ///
    /// Clears any pending confirmations for these medications.
    pub fn learn_from_rejection(&self, offer_medication: &str, request_medication: &str) {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return;
        }
        drop(config);

        self.stats
            .rejections_received
            .fetch_add(1, Ordering::Relaxed);

        let offer_normalized = normalize_for_matching(offer_medication);
        let request_normalized = normalize_for_matching(request_medication);

        let mut pending = self.pending.write().unwrap();

        if pending.remove(&offer_normalized).is_some() {
            tracing::debug!(
                medication = offer_normalized,
                "Cleared pending alias due to rejection"
            );
        }

        if pending.remove(&request_normalized).is_some() {
            tracing::debug!(
                medication = request_normalized,
                "Cleared pending alias due to rejection"
            );
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> AliasLearnerStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get number of pending aliases
    pub fn pending_count(&self) -> usize {
        self.pending.read().unwrap().len()
    }

    /// Get current configuration
    pub fn config(&self) -> AliasLearnerConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: AliasLearnerConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable learning
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Check if learning is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Clear all pending aliases
    pub fn clear_pending(&self) {
        self.pending.write().unwrap().clear();
    }

    /// Export pending aliases for persistence
    pub fn export_pending(&self) -> Vec<(String, Uuid, u32)> {
        self.pending
            .read()
            .unwrap()
            .values()
            .map(|p| (p.normalized_name.clone(), p.master_id, p.confirmations))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AliasLearnerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_confirmations, 2);
        assert!((config.min_score_threshold - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_learn_from_confirmation_disabled() {
        let mut config = AliasLearnerConfig::default();
        config.enabled = false;
        let learner = AliasLearner::new(config);

        let result = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(Uuid::new_v4()),
            None,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_learn_from_confirmation_below_threshold() {
        let learner = AliasLearner::default();

        let result = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.5, // Below threshold
            "Augmentin",
            "اوجمنتين",
            Some(Uuid::new_v4()),
            None,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_learn_from_confirmation_both_known() {
        let learner = AliasLearner::default();

        let result = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()), // Both have master IDs
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_learn_from_confirmation_neither_known() {
        let learner = AliasLearner::default();

        let result = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            None,
            None, // Neither has master ID
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_learn_requires_multiple_confirmations() {
        let learner = AliasLearner::default();
        let master_id = Uuid::new_v4();

        // First confirmation
        let result1 = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id),
            None,
        );

        assert!(result1.is_some());
        let r1 = result1.unwrap();
        assert!(!r1.alias_created);
        assert_eq!(r1.confirmations, 1);
        assert_eq!(r1.required, 2);

        // Second confirmation - should create alias
        let result2 = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id),
            None,
        );

        assert!(result2.is_some());
        let r2 = result2.unwrap();
        assert!(r2.alias_created);
        assert_eq!(r2.confirmations, 2);
        assert_eq!(r2.master_id, Some(master_id));
    }

    #[test]
    fn test_learn_conflicting_masters() {
        let learner = AliasLearner::default();
        let master_id1 = Uuid::new_v4();
        let master_id2 = Uuid::new_v4();

        // First confirmation with master_id1
        learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id1),
            None,
        );

        // Second confirmation with different master_id2 - should reject
        let result = learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id2),
            None,
        );

        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.alias_created);
        assert!(r.master_id.is_none());

        // Pending should be cleared
        assert_eq!(learner.pending_count(), 0);
    }

    #[test]
    fn test_learn_from_rejection_clears_pending() {
        let learner = AliasLearner::default();
        let master_id = Uuid::new_v4();

        // Add pending alias
        learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id),
            None,
        );

        assert_eq!(learner.pending_count(), 1);

        // Rejection should clear it
        learner.learn_from_rejection("Augmentin", "اوجمنتين");

        assert_eq!(learner.pending_count(), 0);
    }

    #[test]
    fn test_stats_tracking() {
        let learner = AliasLearner::default();
        let master_id = Uuid::new_v4();

        // Two confirmations
        learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id),
            None,
        );
        learner.learn_from_confirmation(
            Uuid::new_v4(),
            0.9,
            "Augmentin",
            "اوجمنتين",
            Some(master_id),
            None,
        );

        let stats = learner.stats();
        assert_eq!(stats.confirmations_received, 2);
        assert_eq!(stats.aliases_learned, 1);
    }
}
