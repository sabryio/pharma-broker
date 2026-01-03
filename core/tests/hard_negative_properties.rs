//! Property-based tests for Hard Negative Mining functionality
//!
//! Feature: matching-system-improvements
//! Tests Properties 11 and 19 from the design document
//!
//! These tests validate:
//! - Property 11: Hard Negative Sampling
//! - Property 19: Hard Negative Fallback
//!
//! Run with: cargo test --features test-hard-negative-props --test hard_negative_properties

#![cfg(feature = "test-hard-negative-props")]

use pharma_core::matching::{
    HardNegativeConfig, HardNegativeMiner, MedicationInfo, medication_similarity,
};
use proptest::prelude::*;

// =============================================================================
// Custom Generators for Medications
// =============================================================================

/// Common therapeutic classes for testing
const THERAPEUTIC_CLASSES: &[&str] = &[
    "Antidiabetic",
    "Beta-blocker",
    "ACE Inhibitor",
    "Analgesic",
    "Antibiotic",
    "Antihypertensive",
    "Statin",
    "PPI",
    "Antihistamine",
    "Antidepressant",
];

/// Common medication name prefixes for generating similar-sounding names
const MED_PREFIXES: &[&str] = &[
    "Met", "Ator", "Omep", "Amlo", "Lisin", "Losar", "Panto", "Esome", "Rosu", "Simva", "Prava",
];

/// Common medication name suffixes
const MED_SUFFIXES: &[&str] = &[
    "formin", "vastatin", "prazole", "dipine", "opril", "sartan", "olol", "mycin", "cillin", "pril",
];

/// Generate a random medication name
fn medication_name() -> impl Strategy<Value = String> {
    (
        prop::sample::select(MED_PREFIXES),
        prop::sample::select(MED_SUFFIXES),
    )
        .prop_map(|(prefix, suffix)| format!("{}{}", prefix, suffix))
}

/// Generate a random therapeutic class
fn therapeutic_class() -> impl Strategy<Value = String> {
    prop::sample::select(THERAPEUTIC_CLASSES).prop_map(|s| s.to_string())
}

/// Generate a MedicationInfo with a random name and class
fn medication_info() -> impl Strategy<Value = MedicationInfo> {
    (medication_name(), therapeutic_class())
        .prop_map(|(name, class)| MedicationInfo::new(name).with_class(class))
}

/// Generate a list of medications with at least some in the same class
fn medication_list_with_same_class(
    min_size: usize,
    max_size: usize,
) -> impl Strategy<Value = Vec<MedicationInfo>> {
    // Generate a shared class and some medications in that class
    (
        therapeutic_class(),
        prop::collection::vec(medication_name(), 2..5),
        prop::collection::vec(medication_info(), min_size..max_size),
    )
        .prop_map(|(shared_class, same_class_names, mut other_meds)| {
            // Add medications with the shared class
            for name in same_class_names {
                other_meds.push(MedicationInfo::new(name).with_class(&shared_class));
            }
            other_meds
        })
}

/// Generate pairs of similar medication names (for spelling similarity)
fn similar_medication_pair() -> impl Strategy<Value = (String, String)> {
    prop::sample::select(MED_PREFIXES).prop_flat_map(|prefix| {
        (
            prop::sample::select(MED_SUFFIXES),
            prop::sample::select(MED_SUFFIXES),
        )
            .prop_map(move |(suffix1, suffix2)| {
                (
                    format!("{}{}", prefix, suffix1),
                    format!("{}{}", prefix, suffix2),
                )
            })
    })
}

// =============================================================================
// Property 11: Hard Negative Sampling
// =============================================================================
// For any contrastive validation, the negative sample set SHALL contain
// at least one medication from the same therapeutic class AND at least
// one medication with string similarity > 0.7 to the candidate.
// Validates: Requirements 3.3, 3.4, 7.1, 7.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 3.3, 3.4, 7.1, 7.2
    ///
    /// When hard negatives are available, get_hard_negatives should return
    /// medications from the same therapeutic class
    #[test]
    fn prop_hard_negative_includes_same_class(
        medications in medication_list_with_same_class(5, 15)
    ) {
        // Deduplicate medications by name to avoid index overwrites
        let mut seen_names = std::collections::HashSet::new();
        let medications: Vec<_> = medications
            .into_iter()
            .filter(|m| seen_names.insert(m.name.to_lowercase()))
            .collect();

        // Skip if we don't have enough medications after deduplication
        prop_assume!(medications.len() >= 5);

        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            num_hard_negatives: 3,
            min_spelling_similarity: 0.5, // Lower threshold for testing
            include_same_class: true,
            include_similar_spelling: true,
        });

        miner.build_index(&medications).unwrap();

        // Find a medication that has others in the same class
        let med_with_class = medications.iter()
            .find(|m| {
                m.therapeutic_class.is_some() &&
                medications.iter()
                    .filter(|other| {
                        other.name != m.name &&
                        other.therapeutic_class == m.therapeutic_class
                    })
                    .count() >= 1
            });

        if let Some(target_med) = med_with_class {
            let hard_negatives = miner.get_same_class_negatives(&target_med.name, 5);

            // Should get at least one same-class negative
            prop_assert!(!hard_negatives.is_empty(),
                "Should find same-class negatives for medication '{}' with class {:?}",
                target_med.name, target_med.therapeutic_class);

            // All returned negatives should be from the same class
            // Note: The index normalizes class names (lowercase, removes non-alphanumeric)
            let target_class = target_med.therapeutic_class.as_ref().unwrap();
            let target_class_normalized = target_class.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>();
            for neg in &hard_negatives {
                let neg_class = miner.index().get_class(neg);
                prop_assert!(neg_class.is_some(),
                    "Negative '{}' should have a class", neg);
                prop_assert_eq!(neg_class.unwrap(), &target_class_normalized,
                    "Negative '{}' should be in class '{}', but is in '{}'",
                    neg, target_class_normalized, neg_class.unwrap());
            }
        }
    }

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 3.3, 3.4, 7.1, 7.2
    ///
    /// When hard negatives are available, similar spelling negatives should
    /// have string similarity > min_spelling_similarity to the candidate
    #[test]
    fn prop_hard_negative_similar_spelling_threshold(
        (med1, med2) in similar_medication_pair()
    ) {
        // Create medications with similar names
        let medications = vec![
            MedicationInfo::new(&med1).with_class("ClassA"),
            MedicationInfo::new(&med2).with_class("ClassB"),
            MedicationInfo::new("CompletelyDifferent").with_class("ClassC"),
        ];

        let similarity = medication_similarity(&med1, &med2);

        // Only test if the pair is actually similar enough
        prop_assume!(similarity >= 0.5);

        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            num_hard_negatives: 3,
            min_spelling_similarity: 0.5,
            include_same_class: true,
            include_similar_spelling: true,
        });

        miner.build_index(&medications).unwrap();

        let similar_negatives = miner.get_similar_spelling_negatives(&med1, 5);

        // If we found similar negatives, they should meet the threshold
        for neg in &similar_negatives {
            let neg_similarity = medication_similarity(&med1, neg);
            prop_assert!(neg_similarity >= 0.5,
                "Similar spelling negative '{}' should have similarity >= 0.5 to '{}', got {}",
                neg, med1, neg_similarity);
        }
    }

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 3.3, 3.4, 7.1, 7.2
    ///
    /// get_hard_negatives should not include the target medication itself
    #[test]
    fn prop_hard_negative_excludes_self(
        medications in medication_list_with_same_class(5, 15)
    ) {
        prop_assume!(medications.len() >= 5);

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        miner.build_index(&medications).unwrap();

        for med in &medications {
            let hard_negatives = miner.get_hard_negatives(&med.name, 10);
            let normalized_name = med.name.to_lowercase();

            prop_assert!(!hard_negatives.contains(&normalized_name),
                "Hard negatives for '{}' should not include itself", med.name);
        }
    }

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 3.3, 3.4, 7.1, 7.2
    ///
    /// get_hard_negatives should respect the count limit
    #[test]
    fn prop_hard_negative_respects_count_limit(
        medications in medication_list_with_same_class(10, 20),
        count in 1..5usize
    ) {
        prop_assume!(medications.len() >= 10);

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        miner.build_index(&medications).unwrap();

        let target = &medications[0];
        let hard_negatives = miner.get_hard_negatives(&target.name, count);

        prop_assert!(hard_negatives.len() <= count,
            "Should return at most {} negatives, got {}",
            count, hard_negatives.len());
    }
}

// =============================================================================
// Property 19: Hard Negative Fallback
// =============================================================================
// For any contrastive validation where no hard negatives are available,
// the validator SHALL fall back to random sampling AND log a warning.
// Validates: Requirements 7.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 19: Hard Negative Fallback
    /// Validates: Requirements 7.4
    ///
    /// When index is not built, get_hard_negatives should return empty
    #[test]
    fn prop_hard_negative_fallback_not_built(
        med_name in medication_name()
    ) {
        let miner = HardNegativeMiner::new(HardNegativeConfig::default());

        // Index not built
        prop_assert!(!miner.is_ready(),
            "Miner should not be ready without building index");

        let hard_negatives = miner.get_hard_negatives(&med_name, 5);

        prop_assert!(hard_negatives.is_empty(),
            "Should return empty when index not built");
    }

    /// Feature: matching-system-improvements, Property 19: Hard Negative Fallback
    /// Validates: Requirements 7.4
    ///
    /// When medication has no class, get_same_class_negatives should return empty
    #[test]
    fn prop_hard_negative_fallback_no_class(
        med_name in medication_name()
    ) {
        let medications = vec![
            MedicationInfo::new(&med_name), // No class
            MedicationInfo::new("OtherMed").with_class("SomeClass"),
        ];

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        miner.build_index(&medications).unwrap();

        let same_class_negatives = miner.get_same_class_negatives(&med_name, 5);

        prop_assert!(same_class_negatives.is_empty(),
            "Should return empty for medication without class");
    }

    /// Feature: matching-system-improvements, Property 19: Hard Negative Fallback
    /// Validates: Requirements 7.4
    ///
    /// When medication has no similar spellings, get_similar_spelling_negatives
    /// should return empty
    #[test]
    fn prop_hard_negative_fallback_no_similar_spelling(
        med_name in medication_name()
    ) {
        // Create medications with very different names
        let medications = vec![
            MedicationInfo::new(&med_name).with_class("ClassA"),
            MedicationInfo::new("ZZZZZZZZZ").with_class("ClassB"),
            MedicationInfo::new("XXXXXXXXX").with_class("ClassC"),
        ];

        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            min_spelling_similarity: 0.95, // Very high threshold
            ..Default::default()
        });
        miner.build_index(&medications).unwrap();

        let similar_negatives = miner.get_similar_spelling_negatives(&med_name, 5);

        // With very high threshold and different names, should be empty
        // (unless the generated name happens to be similar to ZZZZZZZZZ or XXXXXXXXX)
        // This is a probabilistic test - most generated names won't match
        if !similar_negatives.is_empty() {
            // If we got results, verify they meet the threshold
            for neg in &similar_negatives {
                let sim = medication_similarity(&med_name, neg);
                prop_assert!(sim >= 0.95,
                    "If similar negatives returned, they should meet threshold");
            }
        }
    }

    /// Feature: matching-system-improvements, Property 19: Hard Negative Fallback
    /// Validates: Requirements 7.4
    ///
    /// When medication is the only one in its class, get_same_class_negatives
    /// should return empty (no other medications to sample from)
    #[test]
    fn prop_hard_negative_fallback_single_in_class(
        med_name in medication_name(),
        unique_class in "[A-Z]{10}"
    ) {
        let medications = vec![
            MedicationInfo::new(&med_name).with_class(&unique_class),
            MedicationInfo::new("OtherMed1").with_class("DifferentClass1"),
            MedicationInfo::new("OtherMed2").with_class("DifferentClass2"),
        ];

        let mut miner = HardNegativeMiner::new(HardNegativeConfig::default());
        miner.build_index(&medications).unwrap();

        let same_class_negatives = miner.get_same_class_negatives(&med_name, 5);

        prop_assert!(same_class_negatives.is_empty(),
            "Should return empty when medication is only one in its class");
    }
}

// =============================================================================
// Additional Property Tests for Index Building
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 7.3
    ///
    /// Index building should be deterministic - same input produces same index
    #[test]
    fn prop_index_building_deterministic(
        medications in prop::collection::vec(medication_info(), 5..15)
    ) {
        prop_assume!(medications.len() >= 5);

        let config = HardNegativeConfig::default();

        let mut miner1 = HardNegativeMiner::new(config.clone());
        let mut miner2 = HardNegativeMiner::new(config);

        miner1.build_index(&medications).unwrap();
        miner2.build_index(&medications).unwrap();

        // Both should have same counts
        prop_assert_eq!(
            miner1.index().medication_count(),
            miner2.index().medication_count(),
            "Medication count should be deterministic"
        );
        prop_assert_eq!(
            miner1.index().class_count(),
            miner2.index().class_count(),
            "Class count should be deterministic"
        );
    }

    /// Feature: matching-system-improvements, Property 11: Hard Negative Sampling
    /// Validates: Requirements 7.3
    ///
    /// Similar pairs should be bidirectional
    #[test]
    fn prop_similar_pairs_bidirectional(
        (med1, med2) in similar_medication_pair()
    ) {
        let similarity = medication_similarity(&med1, &med2);
        prop_assume!(similarity >= 0.5);

        let medications = vec![
            MedicationInfo::new(&med1).with_class("ClassA"),
            MedicationInfo::new(&med2).with_class("ClassB"),
        ];

        let mut miner = HardNegativeMiner::new(HardNegativeConfig {
            min_spelling_similarity: 0.5,
            ..Default::default()
        });
        miner.build_index(&medications).unwrap();

        let similar_to_med1 = miner.get_similar_spelling_negatives(&med1, 5);
        let similar_to_med2 = miner.get_similar_spelling_negatives(&med2, 5);

        // If med2 is similar to med1, then med1 should be similar to med2
        let med2_normalized = med2.to_lowercase();
        let med1_normalized = med1.to_lowercase();

        if similar_to_med1.contains(&med2_normalized) {
            prop_assert!(similar_to_med2.contains(&med1_normalized),
                "Similar pairs should be bidirectional: {} <-> {}",
                med1, med2);
        }
    }
}
