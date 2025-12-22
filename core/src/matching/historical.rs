//! Historical Pattern Learning Module
//!
//! Learns from confirmed/rejected matches to improve future scoring.
//! Tracks medication pair affinity and applies learned bonuses.
//!
//! Ported from Task 6 in advanced_matching_tasks.md

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::matching::arabic::normalize_arabic;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for historical learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalLearningConfig {
    /// Enable historical learning
    pub enabled: bool,
    /// Minimum confirmations before applying affinity bonus
    pub min_confirmations: u32,
    /// Maximum affinity score (cap)
    pub max_affinity: f64,
    /// Minimum affinity score (floor)
    pub min_affinity: f64,
    /// Affinity increase per confirmation
    pub confirmation_boost: f64,
    /// Affinity decrease per rejection
    pub rejection_penalty: f64,
    /// Weight of historical bonus in final score (0.0 - 1.0)
    pub historical_weight: f64,
    /// Decay factor for old feedback (per day)
    pub decay_rate: f64,
    /// Days after which feedback is considered stale
    pub staleness_days: i64,
    /// Confidence interval threshold (require this many samples)
    pub confidence_threshold: u32,
}

impl Default for HistoricalLearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_confirmations: 3,
            max_affinity: 1.0,
            min_affinity: 0.0,
            confirmation_boost: 0.1,
            rejection_penalty: 0.15,
            historical_weight: 0.10, // 10% weight in final score
            decay_rate: 0.01,        // 1% decay per day
            staleness_days: 90,      // 3 months
            confidence_threshold: 5,
        }
    }
}

impl HistoricalLearningConfig {
    /// Conservative preset - requires more data before applying
    pub fn conservative() -> Self {
        Self {
            min_confirmations: 10,
            confirmation_boost: 0.05,
            rejection_penalty: 0.10,
            historical_weight: 0.05,
            confidence_threshold: 15,
            ..Default::default()
        }
    }

    /// Aggressive preset - learns faster
    pub fn aggressive() -> Self {
        Self {
            min_confirmations: 2,
            confirmation_boost: 0.15,
            rejection_penalty: 0.20,
            historical_weight: 0.15,
            confidence_threshold: 3,
            ..Default::default()
        }
    }
}

// =============================================================================
// Medication Affinity
// =============================================================================

/// Tracks learned affinity between two medication names
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAffinity {
    /// Normalized medication name A
    pub medication_a: String,
    /// Normalized medication name B
    pub medication_b: String,
    /// Number of confirmed matches
    pub confirmation_count: u32,
    /// Number of rejected matches
    pub rejection_count: u32,
    /// Calculated affinity score (0.0 - 1.0)
    pub affinity_score: f64,
    /// Last time this pair was updated
    pub last_updated: DateTime<Utc>,
    /// First time this pair was seen
    pub first_seen: DateTime<Utc>,
}

impl MedicationAffinity {
    /// Create a new affinity record
    pub fn new(medication_a: &str, medication_b: &str) -> Self {
        let (a, b) = Self::normalize_pair(medication_a, medication_b);
        let now = Utc::now();
        Self {
            medication_a: a,
            medication_b: b,
            confirmation_count: 0,
            rejection_count: 0,
            affinity_score: 0.5, // Neutral starting point
            last_updated: now,
            first_seen: now,
        }
    }

    /// Normalize and order medication pair for consistent lookup
    fn normalize_pair(a: &str, b: &str) -> (String, String) {
        let norm_a = normalize_arabic(a).to_lowercase();
        let norm_b = normalize_arabic(b).to_lowercase();

        // Always store in alphabetical order for consistent keys
        if norm_a <= norm_b {
            (norm_a, norm_b)
        } else {
            (norm_b, norm_a)
        }
    }

    /// Generate lookup key for this pair
    pub fn key(medication_a: &str, medication_b: &str) -> String {
        let (a, b) = Self::normalize_pair(medication_a, medication_b);
        format!("{}|{}", a, b)
    }

    /// Total feedback count
    pub fn total_feedback(&self) -> u32 {
        self.confirmation_count + self.rejection_count
    }

    /// Confirmation rate
    pub fn confirmation_rate(&self) -> f64 {
        let total = self.total_feedback();
        if total == 0 {
            0.5 // Neutral
        } else {
            self.confirmation_count as f64 / total as f64
        }
    }

    /// Check if we have enough data to be confident
    pub fn is_confident(&self, threshold: u32) -> bool {
        self.total_feedback() >= threshold
    }

    /// Record a confirmation
    pub fn record_confirmation(&mut self, boost: f64, max: f64) {
        self.confirmation_count += 1;
        self.affinity_score = (self.affinity_score + boost).min(max);
        self.last_updated = Utc::now();
    }

    /// Record a rejection
    pub fn record_rejection(&mut self, penalty: f64, min: f64) {
        self.rejection_count += 1;
        self.affinity_score = (self.affinity_score - penalty).max(min);
        self.last_updated = Utc::now();
    }

    /// Apply time decay to affinity score
    pub fn apply_decay(&mut self, decay_rate: f64, staleness_days: i64) {
        let days_since_update = (Utc::now() - self.last_updated).num_days();

        if days_since_update > staleness_days {
            // Reset to neutral if too old
            self.affinity_score = 0.5;
        } else if days_since_update > 0 {
            // Gradual decay towards neutral (0.5)
            let decay = decay_rate * days_since_update as f64;
            if self.affinity_score > 0.5 {
                self.affinity_score = (self.affinity_score - decay).max(0.5);
            } else if self.affinity_score < 0.5 {
                self.affinity_score = (self.affinity_score + decay).min(0.5);
            }
        }
    }
}

// =============================================================================
// Historical Learner
// =============================================================================

/// Statistics for historical learning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoricalLearnerStats {
    pub total_pairs_tracked: usize,
    pub confident_pairs: usize,
    pub total_confirmations: u64,
    pub total_rejections: u64,
    pub bonuses_applied: u64,
    pub avg_affinity_score: f64,
    pub high_affinity_pairs: usize, // > 0.8
    pub low_affinity_pairs: usize,  // < 0.3
}

/// Historical pattern learner
/// Tracks medication pair affinity based on operator feedback
pub struct HistoricalLearner {
    config: RwLock<HistoricalLearningConfig>,
    /// Medication pair affinities (key: "med_a|med_b")
    affinities: RwLock<HashMap<String, MedicationAffinity>>,
    /// Statistics
    total_confirmations: AtomicU64,
    total_rejections: AtomicU64,
    bonuses_applied: AtomicU64,
}

impl Default for HistoricalLearner {
    fn default() -> Self {
        Self::new(HistoricalLearningConfig::default())
    }
}

impl HistoricalLearner {
    /// Create a new historical learner
    pub fn new(config: HistoricalLearningConfig) -> Self {
        Self {
            config: RwLock::new(config),
            affinities: RwLock::new(HashMap::new()),
            total_confirmations: AtomicU64::new(0),
            total_rejections: AtomicU64::new(0),
            bonuses_applied: AtomicU64::new(0),
        }
    }

    /// Record feedback for a medication pair
    pub fn record_feedback(
        &self,
        offer_medication: &str,
        request_medication: &str,
        confirmed: bool,
    ) {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return;
        }

        let key = MedicationAffinity::key(offer_medication, request_medication);
        let mut affinities = self.affinities.write().unwrap();

        let affinity = affinities
            .entry(key)
            .or_insert_with(|| MedicationAffinity::new(offer_medication, request_medication));

        if confirmed {
            affinity.record_confirmation(config.confirmation_boost, config.max_affinity);
            self.total_confirmations.fetch_add(1, Ordering::Relaxed);
        } else {
            affinity.record_rejection(config.rejection_penalty, config.min_affinity);
            self.total_rejections.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get historical bonus for a medication pair
    /// Returns a value between -historical_weight and +historical_weight
    pub fn get_historical_bonus(&self, offer_medication: &str, request_medication: &str) -> f64 {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return 0.0;
        }

        let key = MedicationAffinity::key(offer_medication, request_medication);
        let affinities = self.affinities.read().unwrap();

        if let Some(affinity) = affinities.get(&key) {
            // For positive bonus: require min_confirmations
            // For negative penalty: require confidence_threshold total feedback
            let can_apply_positive = affinity.confirmation_count >= config.min_confirmations;
            let can_apply_negative = affinity.total_feedback() >= config.confidence_threshold
                && affinity.affinity_score < 0.5;

            if !can_apply_positive && !can_apply_negative {
                return 0.0;
            }

            if !affinity.is_confident(config.confidence_threshold) {
                return 0.0;
            }

            if !affinity.is_confident(config.confidence_threshold) {
                return 0.0;
            }

            self.bonuses_applied.fetch_add(1, Ordering::Relaxed);

            // Convert affinity (0.0-1.0) to bonus (-weight to +weight)
            // 0.5 affinity = 0 bonus (neutral)
            // 1.0 affinity = +weight bonus
            // 0.0 affinity = -weight penalty
            let deviation = affinity.affinity_score - 0.5;
            deviation * 2.0 * config.historical_weight
        } else {
            0.0
        }
    }

    /// Get affinity for a medication pair (raw score)
    pub fn get_affinity(&self, offer_medication: &str, request_medication: &str) -> Option<f64> {
        let key = MedicationAffinity::key(offer_medication, request_medication);
        let affinities = self.affinities.read().unwrap();
        affinities.get(&key).map(|a| a.affinity_score)
    }

    /// Get full affinity record for a medication pair
    pub fn get_affinity_record(
        &self,
        offer_medication: &str,
        request_medication: &str,
    ) -> Option<MedicationAffinity> {
        let key = MedicationAffinity::key(offer_medication, request_medication);
        let affinities = self.affinities.read().unwrap();
        affinities.get(&key).cloned()
    }

    /// Apply time decay to all affinities
    pub fn apply_decay(&self) {
        let config = self.config.read().unwrap();
        let mut affinities = self.affinities.write().unwrap();

        for affinity in affinities.values_mut() {
            affinity.apply_decay(config.decay_rate, config.staleness_days);
        }
    }

    /// Get top affinity pairs (highest affinity)
    pub fn get_top_affinities(&self, limit: usize) -> Vec<MedicationAffinity> {
        let affinities = self.affinities.read().unwrap();
        let mut pairs: Vec<_> = affinities.values().cloned().collect();
        pairs.sort_by(|a, b| {
            b.affinity_score
                .partial_cmp(&a.affinity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pairs.truncate(limit);
        pairs
    }

    /// Get bottom affinity pairs (lowest affinity - problematic pairs)
    pub fn get_bottom_affinities(&self, limit: usize) -> Vec<MedicationAffinity> {
        let affinities = self.affinities.read().unwrap();
        let mut pairs: Vec<_> = affinities.values().cloned().collect();
        pairs.sort_by(|a, b| {
            a.affinity_score
                .partial_cmp(&b.affinity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pairs.truncate(limit);
        pairs
    }

    /// Get statistics
    pub fn get_stats(&self) -> HistoricalLearnerStats {
        let config = self.config.read().unwrap();
        let affinities = self.affinities.read().unwrap();

        let total_pairs = affinities.len();
        let confident_pairs = affinities
            .values()
            .filter(|a| a.is_confident(config.confidence_threshold))
            .count();

        let (sum, high, low) = affinities
            .values()
            .fold((0.0, 0, 0), |(sum, high, low), a| {
                let new_high = if a.affinity_score > 0.8 {
                    high + 1
                } else {
                    high
                };
                let new_low = if a.affinity_score < 0.3 { low + 1 } else { low };
                (sum + a.affinity_score, new_high, new_low)
            });

        let avg = if total_pairs > 0 {
            sum / total_pairs as f64
        } else {
            0.5
        };

        HistoricalLearnerStats {
            total_pairs_tracked: total_pairs,
            confident_pairs,
            total_confirmations: self.total_confirmations.load(Ordering::Relaxed),
            total_rejections: self.total_rejections.load(Ordering::Relaxed),
            bonuses_applied: self.bonuses_applied.load(Ordering::Relaxed),
            avg_affinity_score: avg,
            high_affinity_pairs: high,
            low_affinity_pairs: low,
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> HistoricalLearningConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: HistoricalLearningConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable historical learning
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Clear all learned affinities
    pub fn clear(&self) {
        self.affinities.write().unwrap().clear();
        self.total_confirmations.store(0, Ordering::Relaxed);
        self.total_rejections.store(0, Ordering::Relaxed);
        self.bonuses_applied.store(0, Ordering::Relaxed);
    }

    /// Load affinities from external source (e.g., database)
    pub fn load_affinities(&self, affinities: Vec<MedicationAffinity>) {
        let mut map = self.affinities.write().unwrap();
        for affinity in affinities {
            let key = MedicationAffinity::key(&affinity.medication_a, &affinity.medication_b);
            map.insert(key, affinity);
        }
    }

    /// Export all affinities (for persistence)
    pub fn export_affinities(&self) -> Vec<MedicationAffinity> {
        self.affinities.read().unwrap().values().cloned().collect()
    }

    /// Get number of tracked pairs
    pub fn pair_count(&self) -> usize {
        self.affinities.read().unwrap().len()
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

    // =========================================================================
    // MedicationAffinity Tests
    // =========================================================================

    #[test]
    fn test_affinity_new() {
        let affinity = MedicationAffinity::new("Brufen", "Ibuprofen");

        assert_eq!(affinity.medication_a, "brufen");
        assert_eq!(affinity.medication_b, "ibuprofen");
        assert_eq!(affinity.confirmation_count, 0);
        assert_eq!(affinity.rejection_count, 0);
        assert!((affinity.affinity_score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_affinity_key_ordering() {
        // Keys should be consistent regardless of order
        let key1 = MedicationAffinity::key("Brufen", "Ibuprofen");
        let key2 = MedicationAffinity::key("Ibuprofen", "Brufen");

        assert_eq!(key1, key2);
        assert_eq!(key1, "brufen|ibuprofen");
    }

    #[test]
    fn test_affinity_key_arabic_normalization() {
        let key1 = MedicationAffinity::key("البروفين", "Brufen");
        let key2 = MedicationAffinity::key("الـبـروفـيـن", "brufen"); // With tatweel

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_affinity_record_confirmation() {
        let mut affinity = MedicationAffinity::new("A", "B");

        affinity.record_confirmation(0.1, 1.0);

        assert_eq!(affinity.confirmation_count, 1);
        assert!((affinity.affinity_score - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_affinity_record_rejection() {
        let mut affinity = MedicationAffinity::new("A", "B");

        affinity.record_rejection(0.15, 0.0);

        assert_eq!(affinity.rejection_count, 1);
        assert!((affinity.affinity_score - 0.35).abs() < 0.001);
    }

    #[test]
    fn test_affinity_max_cap() {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.affinity_score = 0.95;

        affinity.record_confirmation(0.1, 1.0);

        assert!((affinity.affinity_score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_affinity_min_floor() {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.affinity_score = 0.05;

        affinity.record_rejection(0.15, 0.0);

        assert!((affinity.affinity_score - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_affinity_confirmation_rate() {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.confirmation_count = 8;
        affinity.rejection_count = 2;

        assert!((affinity.confirmation_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_affinity_confirmation_rate_empty() {
        let affinity = MedicationAffinity::new("A", "B");

        assert!((affinity.confirmation_rate() - 0.5).abs() < 0.001);
    }

    #[rstest]
    #[case(5, 5, true)]
    #[case(4, 5, false)]
    #[case(10, 5, true)]
    fn test_affinity_is_confident(
        #[case] total: u32,
        #[case] threshold: u32,
        #[case] expected: bool,
    ) {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.confirmation_count = total / 2;
        affinity.rejection_count = total - total / 2;

        assert_eq!(affinity.is_confident(threshold), expected);
    }

    // =========================================================================
    // HistoricalLearner Tests
    // =========================================================================

    #[test]
    fn test_learner_default() {
        let learner = HistoricalLearner::default();

        assert!(learner.is_enabled());
        assert_eq!(learner.pair_count(), 0);
    }

    #[test]
    fn test_learner_record_confirmation() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("Brufen", "Ibuprofen", true);

        let affinity = learner.get_affinity("Brufen", "Ibuprofen");
        assert!(affinity.is_some());
        assert!(affinity.unwrap() > 0.5);
    }

    #[test]
    fn test_learner_record_rejection() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("Brufen", "Aspirin", false);

        let affinity = learner.get_affinity("Brufen", "Aspirin");
        assert!(affinity.is_some());
        assert!(affinity.unwrap() < 0.5);
    }

    #[test]
    fn test_learner_disabled() {
        let config = HistoricalLearningConfig {
            enabled: false,
            ..Default::default()
        };
        let learner = HistoricalLearner::new(config);

        learner.record_feedback("A", "B", true);

        assert_eq!(learner.pair_count(), 0);
    }

    #[test]
    fn test_learner_historical_bonus_insufficient_confirmations() {
        let learner = HistoricalLearner::default();

        // Record only 2 confirmations (default min is 3)
        learner.record_feedback("A", "B", true);
        learner.record_feedback("A", "B", true);

        let bonus = learner.get_historical_bonus("A", "B");
        assert!((bonus - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_learner_historical_bonus_sufficient_confirmations() {
        let config = HistoricalLearningConfig {
            min_confirmations: 3,
            confidence_threshold: 3,
            historical_weight: 0.10,
            ..Default::default()
        };
        let learner = HistoricalLearner::new(config);

        // Record 5 confirmations
        for _ in 0..5 {
            learner.record_feedback("Brufen", "Ibuprofen", true);
        }

        let bonus = learner.get_historical_bonus("Brufen", "Ibuprofen");
        assert!(bonus > 0.0);
        assert!(bonus <= 0.10); // Max is historical_weight
    }

    #[test]
    fn test_learner_historical_penalty() {
        let config = HistoricalLearningConfig {
            min_confirmations: 0, // Allow penalty without confirmations
            confidence_threshold: 3,
            historical_weight: 0.10,
            ..Default::default()
        };
        let learner = HistoricalLearner::new(config);

        // Record 5 rejections
        for _ in 0..5 {
            learner.record_feedback("Brufen", "Aspirin", false);
        }

        let bonus = learner.get_historical_bonus("Brufen", "Aspirin");
        assert!(bonus < 0.0);
        assert!(bonus >= -0.10); // Min is -historical_weight
    }

    #[test]
    fn test_learner_no_bonus_for_unknown_pair() {
        let learner = HistoricalLearner::default();

        let bonus = learner.get_historical_bonus("Unknown1", "Unknown2");
        assert!((bonus - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_learner_get_affinity_record() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("A", "B", true);
        learner.record_feedback("A", "B", true);
        learner.record_feedback("A", "B", false);

        let record = learner.get_affinity_record("A", "B").unwrap();
        assert_eq!(record.confirmation_count, 2);
        assert_eq!(record.rejection_count, 1);
    }

    #[test]
    fn test_learner_top_affinities() {
        let learner = HistoricalLearner::default();

        // Create pairs with different affinities
        for _ in 0..10 {
            learner.record_feedback("High1", "High2", true);
        }
        for _ in 0..5 {
            learner.record_feedback("Med1", "Med2", true);
        }
        learner.record_feedback("Low1", "Low2", false);

        let top = learner.get_top_affinities(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].affinity_score >= top[1].affinity_score);
    }

    #[test]
    fn test_learner_bottom_affinities() {
        let learner = HistoricalLearner::default();

        for _ in 0..5 {
            learner.record_feedback("Bad1", "Bad2", false);
        }
        learner.record_feedback("Good1", "Good2", true);

        let bottom = learner.get_bottom_affinities(1);
        assert_eq!(bottom.len(), 1);
        assert!(bottom[0].affinity_score < 0.5);
    }

    #[test]
    fn test_learner_stats() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("A", "B", true);
        learner.record_feedback("A", "B", true);
        learner.record_feedback("C", "D", false);

        let stats = learner.get_stats();
        assert_eq!(stats.total_pairs_tracked, 2);
        assert_eq!(stats.total_confirmations, 2);
        assert_eq!(stats.total_rejections, 1);
    }

    #[test]
    fn test_learner_clear() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("A", "B", true);
        assert_eq!(learner.pair_count(), 1);

        learner.clear();
        assert_eq!(learner.pair_count(), 0);
    }

    #[test]
    fn test_learner_export_import() {
        let learner1 = HistoricalLearner::default();

        learner1.record_feedback("A", "B", true);
        learner1.record_feedback("C", "D", false);

        let exported = learner1.export_affinities();
        assert_eq!(exported.len(), 2);

        let learner2 = HistoricalLearner::default();
        learner2.load_affinities(exported);

        assert_eq!(learner2.pair_count(), 2);
        assert!(learner2.get_affinity("A", "B").is_some());
    }

    #[test]
    fn test_learner_config_presets() {
        let conservative = HistoricalLearningConfig::conservative();
        let aggressive = HistoricalLearningConfig::aggressive();

        assert!(conservative.min_confirmations > aggressive.min_confirmations);
        assert!(conservative.historical_weight < aggressive.historical_weight);
    }

    #[test]
    fn test_learner_enable_disable() {
        let learner = HistoricalLearner::default();

        assert!(learner.is_enabled());

        learner.enable(false);
        assert!(!learner.is_enabled());

        learner.enable(true);
        assert!(learner.is_enabled());
    }

    // =========================================================================
    // Decay Tests
    // =========================================================================

    #[test]
    fn test_affinity_decay_towards_neutral() {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.affinity_score = 0.9;
        affinity.last_updated = Utc::now() - Duration::days(10);

        affinity.apply_decay(0.01, 90);

        // Should decay towards 0.5
        assert!(affinity.affinity_score < 0.9);
        assert!(affinity.affinity_score >= 0.5);
    }

    #[test]
    fn test_affinity_decay_stale_reset() {
        let mut affinity = MedicationAffinity::new("A", "B");
        affinity.affinity_score = 0.9;
        affinity.last_updated = Utc::now() - Duration::days(100);

        affinity.apply_decay(0.01, 90);

        // Should reset to neutral
        assert!((affinity.affinity_score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_learner_apply_decay() {
        let learner = HistoricalLearner::default();

        learner.record_feedback("A", "B", true);

        // Manually set old timestamp
        {
            let mut affinities = learner.affinities.write().unwrap();
            let key = MedicationAffinity::key("A", "B");
            if let Some(a) = affinities.get_mut(&key) {
                a.last_updated = Utc::now() - Duration::days(50);
                a.affinity_score = 0.9;
            }
        }

        learner.apply_decay();

        let affinity = learner.get_affinity("A", "B").unwrap();
        assert!(affinity < 0.9);
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_learner_realistic_scenario() {
        let config = HistoricalLearningConfig {
            min_confirmations: 5,
            confidence_threshold: 5,
            ..Default::default()
        };
        let learner = HistoricalLearner::new(config);

        // Simulate real usage: Brufen and Ibuprofen are often confirmed together
        for _ in 0..20 {
            learner.record_feedback("Brufen", "Ibuprofen", true);
        }
        for _ in 0..2 {
            learner.record_feedback("Brufen", "Ibuprofen", false);
        }

        // Aspirin and Ibuprofen are often rejected (need enough to push below 0.5)
        for _ in 0..25 {
            learner.record_feedback("Aspirin", "Ibuprofen", false);
        }
        for _ in 0..3 {
            learner.record_feedback("Aspirin", "Ibuprofen", true);
        }

        // Check bonuses
        let brufen_bonus = learner.get_historical_bonus("Brufen", "Ibuprofen");
        let aspirin_bonus = learner.get_historical_bonus("Aspirin", "Ibuprofen");

        assert!(
            brufen_bonus > 0.0,
            "Brufen-Ibuprofen should have positive bonus (got {})",
            brufen_bonus
        );
        assert!(
            aspirin_bonus < 0.0,
            "Aspirin-Ibuprofen should have negative bonus (got {})",
            aspirin_bonus
        );

        // Check stats
        let stats = learner.get_stats();
        assert_eq!(stats.total_pairs_tracked, 2);

        // Brufen-Ibuprofen should have above-neutral affinity (> 0.5)
        let brufen_affinity = learner.get_affinity("Brufen", "Ibuprofen").unwrap();
        assert!(
            brufen_affinity > 0.5,
            "Brufen-Ibuprofen affinity should be > 0.5 (got {})",
            brufen_affinity
        );

        // Aspirin-Ibuprofen should have below-neutral affinity (< 0.5)
        let aspirin_affinity = learner.get_affinity("Aspirin", "Ibuprofen").unwrap();
        assert!(
            aspirin_affinity < 0.5,
            "Aspirin-Ibuprofen affinity should be < 0.5 (got {})",
            aspirin_affinity
        );
    }
}
