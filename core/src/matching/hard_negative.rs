//! Hard Negative Mining for Contrastive Validation
//!
//! Provides strategic selection of challenging negative samples for contrastive
//! validation. Hard negatives are medications that are similar to the candidate
//! but are incorrect matches - either from the same therapeutic class or with
//! similar spelling.
//!
//! This improves false positive detection by ensuring the validator tests against
//! the most challenging cases rather than random samples.

use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::matching::fuzzy::medication_similarity;

/// Configuration for hard negative mining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardNegativeConfig {
    /// Number of hard negatives to include in validation samples
    pub num_hard_negatives: usize,
    /// Minimum string similarity threshold for "similar spelling" pairs (0.0-1.0)
    pub min_spelling_similarity: f64,
    /// Whether to include same-class negatives in sampling
    pub include_same_class: bool,
    /// Whether to include similar-spelling negatives in sampling
    pub include_similar_spelling: bool,
}

impl Default for HardNegativeConfig {
    fn default() -> Self {
        Self {
            num_hard_negatives: 3,
            min_spelling_similarity: 0.7,
            include_same_class: true,
            include_similar_spelling: true,
        }
    }
}

impl HardNegativeConfig {
    /// Create a new configuration with specified parameters
    pub fn new(num_hard_negatives: usize, min_spelling_similarity: f64) -> Self {
        Self {
            num_hard_negatives,
            min_spelling_similarity,
            include_same_class: true,
            include_similar_spelling: true,
        }
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            num_hard_negatives: std::env::var("HARD_NEGATIVE_COUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            min_spelling_similarity: std::env::var("HARD_NEGATIVE_MIN_SIMILARITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.7),
            include_same_class: std::env::var("HARD_NEGATIVE_INCLUDE_CLASS")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            include_similar_spelling: std::env::var("HARD_NEGATIVE_INCLUDE_SPELLING")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
        }
    }

    /// Create a strict configuration with more hard negatives
    pub fn strict() -> Self {
        Self {
            num_hard_negatives: 5,
            min_spelling_similarity: 0.6,
            include_same_class: true,
            include_similar_spelling: true,
        }
    }

    /// Create a relaxed configuration with fewer hard negatives
    pub fn relaxed() -> Self {
        Self {
            num_hard_negatives: 2,
            min_spelling_similarity: 0.8,
            include_same_class: true,
            include_similar_spelling: true,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), HardNegativeError> {
        if self.min_spelling_similarity < 0.0 || self.min_spelling_similarity > 1.0 {
            return Err(HardNegativeError::InvalidConfig(
                "min_spelling_similarity must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.num_hard_negatives == 0 {
            return Err(HardNegativeError::InvalidConfig(
                "num_hard_negatives must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Pre-computed index for efficient hard negative lookup
///
/// This index maintains two mappings:
/// 1. Medications grouped by therapeutic class for same-class negative sampling
/// 2. Pre-computed similar spelling pairs for efficient lookup
#[derive(Debug, Clone, Default)]
pub struct HardNegativeIndex {
    /// Medications grouped by therapeutic class
    /// Key: therapeutic class name (normalized)
    /// Value: list of medication names in that class
    pub by_class: HashMap<String, Vec<String>>,

    /// Pre-computed similar spelling pairs
    /// Key: medication name (normalized)
    /// Value: list of medication names with similar spelling
    pub similar_pairs: HashMap<String, Vec<String>>,

    /// Reverse mapping from medication to its therapeutic class
    medication_to_class: HashMap<String, String>,

    /// Whether the index has been built
    is_built: bool,
}

impl HardNegativeIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            by_class: HashMap::new(),
            similar_pairs: HashMap::new(),
            medication_to_class: HashMap::new(),
            is_built: false,
        }
    }

    /// Check if the index has been built
    pub fn is_built(&self) -> bool {
        self.is_built
    }

    /// Get the number of therapeutic classes in the index
    pub fn class_count(&self) -> usize {
        self.by_class.len()
    }

    /// Get the number of medications in the index
    pub fn medication_count(&self) -> usize {
        self.medication_to_class.len()
    }

    /// Get the number of similar spelling pairs in the index
    pub fn similar_pair_count(&self) -> usize {
        self.similar_pairs.values().map(|v| v.len()).sum()
    }

    /// Get the therapeutic class for a medication
    pub fn get_class(&self, medication: &str) -> Option<&String> {
        let normalized = Self::normalize(medication);
        self.medication_to_class.get(&normalized)
    }

    /// Get all medications in a therapeutic class
    pub fn get_medications_in_class(&self, class: &str) -> Option<&Vec<String>> {
        let normalized = Self::normalize(class);
        self.by_class.get(&normalized)
    }

    /// Get similar spelling medications for a given medication
    pub fn get_similar_spellings(&self, medication: &str) -> Option<&Vec<String>> {
        let normalized = Self::normalize(medication);
        self.similar_pairs.get(&normalized)
    }

    /// Add a medication to the index with its therapeutic class
    pub fn add_medication(&mut self, medication: &str, therapeutic_class: Option<&str>) {
        let med_normalized = Self::normalize(medication);

        if let Some(class) = therapeutic_class {
            let class_normalized = Self::normalize(class);

            // Add to by_class mapping
            self.by_class
                .entry(class_normalized.clone())
                .or_default()
                .push(med_normalized.clone());

            // Add to medication_to_class mapping
            self.medication_to_class
                .insert(med_normalized, class_normalized);
        }
    }

    /// Add a similar spelling pair to the index
    pub fn add_similar_pair(&mut self, medication_a: &str, medication_b: &str) {
        let a_normalized = Self::normalize(medication_a);
        let b_normalized = Self::normalize(medication_b);

        // Add bidirectional mapping
        self.similar_pairs
            .entry(a_normalized.clone())
            .or_default()
            .push(b_normalized.clone());

        self.similar_pairs
            .entry(b_normalized)
            .or_default()
            .push(a_normalized);
    }

    /// Mark the index as built
    pub fn mark_built(&mut self) {
        self.is_built = true;
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.by_class.clear();
        self.similar_pairs.clear();
        self.medication_to_class.clear();
        self.is_built = false;
    }

    /// Normalize a string for consistent lookup
    pub fn normalize(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Error type for hard negative mining operations
#[derive(Debug)]
pub enum HardNegativeError {
    /// Invalid configuration
    InvalidConfig(String),
    /// Index not built
    IndexNotBuilt,
    /// No medications available
    NoMedications,
}

impl std::fmt::Display for HardNegativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardNegativeError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            HardNegativeError::IndexNotBuilt => write!(f, "Hard negative index has not been built"),
            HardNegativeError::NoMedications => write!(f, "No medications available for sampling"),
        }
    }
}

impl std::error::Error for HardNegativeError {}

/// Information about a medication for building the hard negative index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationInfo {
    /// Medication name (English)
    pub name: String,
    /// Arabic name (optional)
    pub name_arabic: Option<String>,
    /// Therapeutic class (e.g., "Antidiabetic", "Beta-blocker")
    pub therapeutic_class: Option<String>,
}

impl MedicationInfo {
    /// Create a new medication info
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            name_arabic: None,
            therapeutic_class: None,
        }
    }

    /// Create with therapeutic class
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.therapeutic_class = Some(class.into());
        self
    }

    /// Create with Arabic name
    pub fn with_arabic(mut self, arabic: impl Into<String>) -> Self {
        self.name_arabic = Some(arabic.into());
        self
    }
}

/// Hard negative miner for selecting challenging negative samples
///
/// This component builds and maintains an index of medications grouped by
/// therapeutic class and similar spelling, enabling efficient selection of
/// hard negatives for contrastive validation.
#[derive(Debug, Clone)]
pub struct HardNegativeMiner {
    /// Configuration for hard negative mining
    config: HardNegativeConfig,
    /// Pre-computed index for efficient lookup
    index: HardNegativeIndex,
}

impl Default for HardNegativeMiner {
    fn default() -> Self {
        Self::new(HardNegativeConfig::default())
    }
}

impl HardNegativeMiner {
    /// Create a new hard negative miner with the given configuration
    pub fn new(config: HardNegativeConfig) -> Self {
        Self {
            config,
            index: HardNegativeIndex::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Self {
        Self::new(HardNegativeConfig::from_env())
    }

    /// Get the current configuration
    pub fn config(&self) -> &HardNegativeConfig {
        &self.config
    }

    /// Get the index
    pub fn index(&self) -> &HardNegativeIndex {
        &self.index
    }

    /// Check if the index has been built
    pub fn is_ready(&self) -> bool {
        self.index.is_built()
    }

    /// Build the index from a list of medications
    ///
    /// This method:
    /// 1. Groups medications by therapeutic class
    /// 2. Pre-computes similar spelling pairs based on min_spelling_similarity
    ///
    /// # Arguments
    /// * `medications` - List of medication information to index
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(HardNegativeError)` if medications list is empty
    pub fn build_index(&mut self, medications: &[MedicationInfo]) -> Result<(), HardNegativeError> {
        if medications.is_empty() {
            return Err(HardNegativeError::NoMedications);
        }

        // Clear any existing index
        self.index.clear();

        // Step 1: Add all medications to the index with their therapeutic class
        for med in medications {
            self.index
                .add_medication(&med.name, med.therapeutic_class.as_deref());
        }

        // Step 2: Pre-compute similar spelling pairs
        // This is O(n²) but only done once during index building
        let med_names: Vec<&str> = medications.iter().map(|m| m.name.as_str()).collect();

        for i in 0..med_names.len() {
            for j in (i + 1)..med_names.len() {
                let similarity = medication_similarity(med_names[i], med_names[j]);
                if similarity >= self.config.min_spelling_similarity {
                    self.index.add_similar_pair(med_names[i], med_names[j]);
                }
            }
        }

        // Mark the index as built
        self.index.mark_built();

        tracing::info!(
            medications = medications.len(),
            classes = self.index.class_count(),
            similar_pairs = self.index.similar_pair_count() / 2, // Divide by 2 since bidirectional
            "Hard negative index built"
        );

        Ok(())
    }

    /// Get hard negatives for a medication
    ///
    /// Combines same-class negatives and similar-spelling negatives based on config.
    /// Returns up to `count` hard negatives, prioritizing:
    /// 1. Same therapeutic class (if enabled)
    /// 2. Similar spelling (if enabled)
    ///
    /// # Arguments
    /// * `medication` - The medication to find hard negatives for
    /// * `count` - Maximum number of hard negatives to return
    ///
    /// # Returns
    /// A vector of medication names that are hard negatives
    pub fn get_hard_negatives(&self, medication: &str, count: usize) -> Vec<String> {
        if !self.index.is_built() {
            tracing::warn!("Hard negative index not built, returning empty list");
            return vec![];
        }

        let mut result = Vec::with_capacity(count);
        let normalized = HardNegativeIndex::normalize(medication);

        // Collect same-class negatives if enabled
        if self.config.include_same_class {
            let class_negatives = self.get_same_class_negatives(medication, count);
            for neg in class_negatives {
                if result.len() >= count {
                    break;
                }
                if !result.contains(&neg) && neg != normalized {
                    result.push(neg);
                }
            }
        }

        // Collect similar-spelling negatives if enabled
        if self.config.include_similar_spelling && result.len() < count {
            let spelling_negatives =
                self.get_similar_spelling_negatives(medication, count - result.len());
            for neg in spelling_negatives {
                if result.len() >= count {
                    break;
                }
                if !result.contains(&neg) && neg != normalized {
                    result.push(neg);
                }
            }
        }

        result
    }

    /// Get negatives from the same therapeutic class
    ///
    /// # Arguments
    /// * `medication` - The medication to find same-class negatives for
    /// * `count` - Maximum number of negatives to return
    ///
    /// # Returns
    /// A vector of medication names from the same therapeutic class
    pub fn get_same_class_negatives(&self, medication: &str, count: usize) -> Vec<String> {
        if !self.index.is_built() {
            return vec![];
        }

        let normalized = HardNegativeIndex::normalize(medication);

        // Get the therapeutic class for this medication
        let class = match self.index.get_class(medication) {
            Some(c) => c.clone(),
            None => return vec![],
        };

        // Get all medications in the same class
        let class_meds = match self.index.get_medications_in_class(&class) {
            Some(meds) => meds,
            None => return vec![],
        };

        // Filter out the medication itself and sample
        let candidates: Vec<&String> = class_meds.iter().filter(|m| **m != normalized).collect();

        if candidates.is_empty() {
            return vec![];
        }

        // Sample up to `count` medications
        if candidates.len() <= count {
            return candidates.into_iter().cloned().collect();
        }

        let mut rng = rand::rng();
        candidates
            .choose_multiple(&mut rng, count)
            .cloned()
            .cloned()
            .collect()
    }

    /// Get negatives with similar spelling
    ///
    /// # Arguments
    /// * `medication` - The medication to find similar-spelling negatives for
    /// * `count` - Maximum number of negatives to return
    ///
    /// # Returns
    /// A vector of medication names with similar spelling
    pub fn get_similar_spelling_negatives(&self, medication: &str, count: usize) -> Vec<String> {
        if !self.index.is_built() {
            return vec![];
        }

        // Get pre-computed similar spellings
        let similar = match self.index.get_similar_spellings(medication) {
            Some(s) => s,
            None => return vec![],
        };

        if similar.is_empty() {
            return vec![];
        }

        // Sample up to `count` medications
        if similar.len() <= count {
            return similar.clone();
        }

        let mut rng = rand::rng();
        similar.choose_multiple(&mut rng, count).cloned().collect()
    }

    /// Reset the miner, clearing the index
    pub fn reset(&mut self) {
        self.index.clear();
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: HardNegativeConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // HardNegativeConfig Tests
    // =========================================================================

    #[test]
    fn test_config_default() {
        let config = HardNegativeConfig::default();
        assert_eq!(config.num_hard_negatives, 3);
        assert!((config.min_spelling_similarity - 0.7).abs() < 0.001);
        assert!(config.include_same_class);
        assert!(config.include_similar_spelling);
    }

    #[test]
    fn test_config_new() {
        let config = HardNegativeConfig::new(5, 0.8);
        assert_eq!(config.num_hard_negatives, 5);
        assert!((config.min_spelling_similarity - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_config_strict() {
        let config = HardNegativeConfig::strict();
        assert_eq!(config.num_hard_negatives, 5);
        assert!((config.min_spelling_similarity - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_config_relaxed() {
        let config = HardNegativeConfig::relaxed();
        assert_eq!(config.num_hard_negatives, 2);
        assert!((config.min_spelling_similarity - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_config_validate_valid() {
        let config = HardNegativeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_similarity_low() {
        let config = HardNegativeConfig {
            min_spelling_similarity: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_invalid_similarity_high() {
        let config = HardNegativeConfig {
            min_spelling_similarity: 1.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_invalid_count() {
        let config = HardNegativeConfig {
            num_hard_negatives: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = HardNegativeConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("num_hard_negatives"));
        assert!(json.contains("min_spelling_similarity"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "num_hard_negatives": 4,
            "min_spelling_similarity": 0.75,
            "include_same_class": true,
            "include_similar_spelling": false
        }"#;
        let config: HardNegativeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.num_hard_negatives, 4);
        assert!((config.min_spelling_similarity - 0.75).abs() < 0.001);
        assert!(config.include_same_class);
        assert!(!config.include_similar_spelling);
    }

    // =========================================================================
    // HardNegativeIndex Tests
    // =========================================================================

    #[test]
    fn test_index_new() {
        let index = HardNegativeIndex::new();
        assert!(!index.is_built());
        assert_eq!(index.class_count(), 0);
        assert_eq!(index.medication_count(), 0);
    }

    #[test]
    fn test_index_add_medication() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Metformin", Some("Antidiabetic"));

        assert_eq!(index.medication_count(), 1);
        assert_eq!(index.class_count(), 1);
        assert_eq!(
            index.get_class("Metformin"),
            Some(&"antidiabetic".to_string())
        );
    }

    #[test]
    fn test_index_add_medication_no_class() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Unknown Med", None);

        // Should not be added to class mappings
        assert_eq!(index.medication_count(), 0);
        assert_eq!(index.class_count(), 0);
    }

    #[test]
    fn test_index_add_multiple_medications_same_class() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Metformin", Some("Antidiabetic"));
        index.add_medication("Glipizide", Some("Antidiabetic"));
        index.add_medication("Insulin", Some("Antidiabetic"));

        assert_eq!(index.medication_count(), 3);
        assert_eq!(index.class_count(), 1);

        let meds = index.get_medications_in_class("Antidiabetic").unwrap();
        assert_eq!(meds.len(), 3);
    }

    #[test]
    fn test_index_add_similar_pair() {
        let mut index = HardNegativeIndex::new();
        index.add_similar_pair("Metformin", "Metoprolol");

        // Should be bidirectional
        let similar_to_metformin = index.get_similar_spellings("Metformin").unwrap();
        assert!(similar_to_metformin.contains(&"metoprolol".to_string()));

        let similar_to_metoprolol = index.get_similar_spellings("Metoprolol").unwrap();
        assert!(similar_to_metoprolol.contains(&"metformin".to_string()));
    }

    #[test]
    fn test_index_get_class_case_insensitive() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Metformin", Some("Antidiabetic"));

        // Should find regardless of case
        assert!(index.get_class("METFORMIN").is_some());
        assert!(index.get_class("metformin").is_some());
        assert!(index.get_class("Metformin").is_some());
    }

    #[test]
    fn test_index_get_medications_in_class_case_insensitive() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Metformin", Some("Antidiabetic"));

        // Should find regardless of case
        assert!(index.get_medications_in_class("ANTIDIABETIC").is_some());
        assert!(index.get_medications_in_class("antidiabetic").is_some());
    }

    #[test]
    fn test_index_mark_built() {
        let mut index = HardNegativeIndex::new();
        assert!(!index.is_built());

        index.mark_built();
        assert!(index.is_built());
    }

    #[test]
    fn test_index_clear() {
        let mut index = HardNegativeIndex::new();
        index.add_medication("Metformin", Some("Antidiabetic"));
        index.add_similar_pair("Metformin", "Metoprolol");
        index.mark_built();

        index.clear();

        assert!(!index.is_built());
        assert_eq!(index.class_count(), 0);
        assert_eq!(index.medication_count(), 0);
        assert_eq!(index.similar_pair_count(), 0);
    }

    #[test]
    fn test_index_similar_pair_count() {
        let mut index = HardNegativeIndex::new();
        index.add_similar_pair("Metformin", "Metoprolol");
        index.add_similar_pair("Losartan", "Lisinopril");

        // Each pair adds 2 entries (bidirectional)
        assert_eq!(index.similar_pair_count(), 4);
    }

    #[test]
    fn test_index_normalize() {
        // Test normalization through add_medication
        let mut index = HardNegativeIndex::new();
        index.add_medication("  METFORMIN  500mg  ", Some("  ANTIDIABETIC  "));

        // Should normalize to lowercase and clean whitespace
        assert!(index.get_class("metformin 500mg").is_some());
        assert!(index.get_medications_in_class("antidiabetic").is_some());
    }

    // =========================================================================
    // HardNegativeError Tests
    // =========================================================================

    #[test]
    fn test_error_display_invalid_config() {
        let err = HardNegativeError::InvalidConfig("test error".to_string());
        assert!(err.to_string().contains("Invalid configuration"));
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_error_display_index_not_built() {
        let err = HardNegativeError::IndexNotBuilt;
        assert!(err.to_string().contains("not been built"));
    }

    #[test]
    fn test_error_display_no_medications() {
        let err = HardNegativeError::NoMedications;
        assert!(err.to_string().contains("No medications"));
    }

    // =========================================================================
    // MedicationInfo Tests
    // =========================================================================

    #[test]
    fn test_medication_info_new() {
        let info = MedicationInfo::new("Metformin");
        assert_eq!(info.name, "Metformin");
        assert!(info.name_arabic.is_none());
        assert!(info.therapeutic_class.is_none());
    }

    #[test]
    fn test_medication_info_with_class() {
        let info = MedicationInfo::new("Metformin").with_class("Antidiabetic");
        assert_eq!(info.name, "Metformin");
        assert_eq!(info.therapeutic_class, Some("Antidiabetic".to_string()));
    }

    #[test]
    fn test_medication_info_with_arabic() {
        let info = MedicationInfo::new("Metformin").with_arabic("ميتفورمين");
        assert_eq!(info.name, "Metformin");
        assert_eq!(info.name_arabic, Some("ميتفورمين".to_string()));
    }

    #[test]
    fn test_medication_info_builder_chain() {
        let info = MedicationInfo::new("Metformin")
            .with_class("Antidiabetic")
            .with_arabic("ميتفورمين");
        assert_eq!(info.name, "Metformin");
        assert_eq!(info.therapeutic_class, Some("Antidiabetic".to_string()));
        assert_eq!(info.name_arabic, Some("ميتفورمين".to_string()));
    }

    // =========================================================================
    // HardNegativeMiner Tests
    // =========================================================================

    #[test]
    fn test_miner_new() {
        let miner = HardNegativeMiner::new(HardNegativeConfig::default());
        assert!(!miner.is_ready());
        assert_eq!(miner.config().num_hard_negatives, 3);
    }

    #[test]
    fn test_miner_default() {
        let miner = HardNegativeMiner::default();
        assert!(!miner.is_ready());
    }

    #[test]
    fn test_miner_build_index_empty() {
        let mut miner = HardNegativeMiner::default();
        let result = miner.build_index(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_miner_build_index_success() {
        let mut miner = HardNegativeMiner::default();
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Glipizide").with_class("Antidiabetic"),
            MedicationInfo::new("Metoprolol").with_class("Beta-blocker"),
        ];

        let result = miner.build_index(&medications);
        assert!(result.is_ok());
        assert!(miner.is_ready());
        assert_eq!(miner.index().medication_count(), 3);
        assert_eq!(miner.index().class_count(), 2);
    }

    #[test]
    fn test_miner_get_same_class_negatives() {
        let mut miner = HardNegativeMiner::default();
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Glipizide").with_class("Antidiabetic"),
            MedicationInfo::new("Insulin").with_class("Antidiabetic"),
            MedicationInfo::new("Metoprolol").with_class("Beta-blocker"),
        ];

        miner.build_index(&medications).unwrap();

        let negatives = miner.get_same_class_negatives("Metformin", 5);
        // Should get Glipizide and Insulin (same class), but not Metformin itself
        assert_eq!(negatives.len(), 2);
        assert!(!negatives.contains(&"metformin".to_string()));
        assert!(negatives.contains(&"glipizide".to_string()));
        assert!(negatives.contains(&"insulin".to_string()));
    }

    #[test]
    fn test_miner_get_same_class_negatives_no_class() {
        let mut miner = HardNegativeMiner::default();
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Unknown"), // No class
        ];

        miner.build_index(&medications).unwrap();

        // Unknown has no class, so should return empty
        let negatives = miner.get_same_class_negatives("Unknown", 5);
        assert!(negatives.is_empty());
    }

    #[test]
    fn test_miner_get_similar_spelling_negatives() {
        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            min_spelling_similarity: 0.6, // Lower threshold to catch Metformin/Metoprolol
            ..Default::default()
        });
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Metoprolol").with_class("Beta-blocker"),
            MedicationInfo::new("Aspirin").with_class("Analgesic"),
        ];

        miner.build_index(&medications).unwrap();

        let negatives = miner.get_similar_spelling_negatives("Metformin", 5);
        // Metformin and Metoprolol should be similar enough
        assert!(
            negatives.contains(&"metoprolol".to_string()),
            "Expected Metoprolol to be similar to Metformin, got: {:?}",
            negatives
        );
    }

    #[test]
    fn test_miner_get_hard_negatives_combined() {
        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            num_hard_negatives: 5,
            min_spelling_similarity: 0.6,
            include_same_class: true,
            include_similar_spelling: true,
        });
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Glipizide").with_class("Antidiabetic"),
            MedicationInfo::new("Metoprolol").with_class("Beta-blocker"),
            MedicationInfo::new("Aspirin").with_class("Analgesic"),
        ];

        miner.build_index(&medications).unwrap();

        let negatives = miner.get_hard_negatives("Metformin", 5);
        // Should include Glipizide (same class) and possibly Metoprolol (similar spelling)
        assert!(!negatives.is_empty());
        assert!(!negatives.contains(&"metformin".to_string()));
    }

    #[test]
    fn test_miner_get_hard_negatives_not_built() {
        let miner = HardNegativeMiner::default();
        let negatives = miner.get_hard_negatives("Metformin", 5);
        assert!(negatives.is_empty());
    }

    #[test]
    fn test_miner_reset() {
        let mut miner = HardNegativeMiner::default();
        let medications = vec![MedicationInfo::new("Metformin").with_class("Antidiabetic")];

        miner.build_index(&medications).unwrap();
        assert!(miner.is_ready());

        miner.reset();
        assert!(!miner.is_ready());
    }

    #[test]
    fn test_miner_set_config() {
        let mut miner = HardNegativeMiner::default();
        assert_eq!(miner.config().num_hard_negatives, 3);

        miner.set_config(HardNegativeConfig::strict());
        assert_eq!(miner.config().num_hard_negatives, 5);
    }

    #[test]
    fn test_miner_get_same_class_negatives_limited() {
        let mut miner = HardNegativeMiner::default();
        let medications = vec![
            MedicationInfo::new("Metformin").with_class("Antidiabetic"),
            MedicationInfo::new("Glipizide").with_class("Antidiabetic"),
            MedicationInfo::new("Insulin").with_class("Antidiabetic"),
            MedicationInfo::new("Pioglitazone").with_class("Antidiabetic"),
            MedicationInfo::new("Sitagliptin").with_class("Antidiabetic"),
        ];

        miner.build_index(&medications).unwrap();

        // Request only 2 negatives
        let negatives = miner.get_same_class_negatives("Metformin", 2);
        assert_eq!(negatives.len(), 2);
    }
}
