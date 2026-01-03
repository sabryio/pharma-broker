//! Property-based tests for Arabic matching functionality
//!
//! Feature: matching-system-improvements
//! Tests Properties 5, 6, and 7 from the design document
//!
//! These tests validate:
//! - Property 5: Arabic Normalization Consistency
//! - Property 6: Arabic Phonetic Key Grouping
//! - Property 7: Arabic-Aware Distance Selection
//!
//! Run with: cargo test --features test-arabic-props --test arabic_matching_properties

#![cfg(feature = "test-arabic-props")]

use pharma_core::matching::{
    ArabicPhoneticMatcher, FuzzyStringStrategy, arabic_string_similarity, contains_arabic,
    normalize_arabic, phonetic_key,
};
use proptest::prelude::*;

// =============================================================================
// Custom Generators for Arabic Text
// =============================================================================

/// Generate random Arabic strings with alef variants
fn arabic_string_with_alef_variants() -> impl Strategy<Value = String> {
    // Base Arabic letters
    let base_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
    // Alef variants that should normalize to ا
    let alef_variants = ['أ', 'إ', 'آ', 'ٱ', 'ا'];

    prop::collection::vec(
        prop::strategy::Union::new(vec![
            // Regular Arabic letters
            Just(base_letters.chars().collect::<Vec<_>>())
                .prop_flat_map(|chars| prop::sample::select(chars).prop_map(|c| c.to_string()))
                .boxed(),
            // Alef variants
            prop::sample::select(alef_variants.to_vec())
                .prop_map(|c| c.to_string())
                .boxed(),
        ]),
        1..10,
    )
    .prop_map(|v| v.join(""))
}

/// Generate random Arabic strings with taa marbuta
fn arabic_string_with_taa_marbuta() -> impl Strategy<Value = String> {
    let base_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
    let taa_variants = ['ة', 'ه'];

    prop::collection::vec(
        prop::strategy::Union::new(vec![
            Just(base_letters.chars().collect::<Vec<_>>())
                .prop_flat_map(|chars| prop::sample::select(chars).prop_map(|c| c.to_string()))
                .boxed(),
            prop::sample::select(taa_variants.to_vec())
                .prop_map(|c| c.to_string())
                .boxed(),
        ]),
        1..10,
    )
    .prop_map(|v| v.join(""))
}

/// Generate random Arabic strings with alef maksura
fn arabic_string_with_alef_maksura() -> impl Strategy<Value = String> {
    let base_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
    let ya_variants = ['ى', 'ي'];

    prop::collection::vec(
        prop::strategy::Union::new(vec![
            Just(base_letters.chars().collect::<Vec<_>>())
                .prop_flat_map(|chars| prop::sample::select(chars).prop_map(|c| c.to_string()))
                .boxed(),
            prop::sample::select(ya_variants.to_vec())
                .prop_map(|c| c.to_string())
                .boxed(),
        ]),
        1..10,
    )
    .prop_map(|v| v.join(""))
}

/// Generate pairs of Arabic strings that differ only by phonetic variations
fn phonetically_equivalent_pair() -> impl Strategy<Value = (String, String)> {
    // Phonetic pairs: (variant, canonical)
    let phonetic_pairs = [
        ('ص', 'س'), // Emphatic S
        ('ض', 'د'), // Emphatic D
        ('ط', 'ت'), // Emphatic T
        ('ظ', 'ذ'), // Emphatic TH/Z
        ('ق', 'ك'), // Q/K confusion
        ('ح', 'ه'), // Guttural H
    ];

    let base_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";

    // Generate a base string and a position to substitute
    (1..8usize, prop::sample::select(phonetic_pairs.to_vec())).prop_flat_map(
        move |(len, (variant, canonical))| {
            prop::collection::vec(
                Just(base_letters.chars().collect::<Vec<_>>()).prop_flat_map(prop::sample::select),
                len,
            )
            .prop_flat_map(move |chars| {
                (0..chars.len()).prop_map(move |pos| {
                    let mut str1: Vec<char> = chars.clone();
                    let mut str2: Vec<char> = chars.clone();

                    // Insert variant at position in str1
                    str1.insert(pos, variant);
                    // Insert canonical at same position in str2
                    str2.insert(pos, canonical);

                    (str1.into_iter().collect(), str2.into_iter().collect())
                })
            })
        },
    )
}

/// Generate random English strings (ASCII alphabetic)
fn english_string() -> impl Strategy<Value = String> {
    "[a-zA-Z]{1,20}"
}

/// Generate random Arabic strings
fn arabic_string() -> impl Strategy<Value = String> {
    let arabic_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
    prop::collection::vec(
        Just(arabic_letters.chars().collect::<Vec<_>>()).prop_flat_map(prop::sample::select),
        1..15,
    )
    .prop_map(|v| v.into_iter().collect())
}

// =============================================================================
// Property 5: Arabic Normalization Consistency
// =============================================================================
// For any Arabic string containing alef variants (أ إ آ ٱ), taa marbuta (ة),
// or alef maksura (ى), normalizing the string SHALL produce the same output
// as normalizing the canonical form.
// Validates: Requirements 2.1

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 5: Arabic Normalization Consistency
    /// Validates: Requirements 2.1
    ///
    /// For any Arabic string with alef variants, normalizing produces consistent output
    #[test]
    fn prop_arabic_normalization_alef_variants(s in arabic_string_with_alef_variants()) {
        let normalized = normalize_arabic(&s);

        // Normalizing again should produce the same result (idempotent)
        let double_normalized = normalize_arabic(&normalized);
        prop_assert_eq!(&normalized, &double_normalized,
            "Normalization should be idempotent: '{}' -> '{}' -> '{}'",
            s, normalized, double_normalized);

        // Normalized string should not contain alef variants
        prop_assert!(!normalized.contains('أ'), "Should not contain أ");
        prop_assert!(!normalized.contains('إ'), "Should not contain إ");
        prop_assert!(!normalized.contains('آ'), "Should not contain آ");
        prop_assert!(!normalized.contains('ٱ'), "Should not contain ٱ");
    }

    /// Feature: matching-system-improvements, Property 5: Arabic Normalization Consistency
    /// Validates: Requirements 2.1
    ///
    /// For any Arabic string with taa marbuta, normalizing produces consistent output
    #[test]
    fn prop_arabic_normalization_taa_marbuta(s in arabic_string_with_taa_marbuta()) {
        let normalized = normalize_arabic(&s);

        // Normalizing again should produce the same result
        let double_normalized = normalize_arabic(&normalized);
        prop_assert_eq!(&normalized, &double_normalized,
            "Normalization should be idempotent");

        // Normalized string should not contain taa marbuta
        prop_assert!(!normalized.contains('ة'), "Should not contain ة (taa marbuta)");
    }

    /// Feature: matching-system-improvements, Property 5: Arabic Normalization Consistency
    /// Validates: Requirements 2.1
    ///
    /// For any Arabic string with alef maksura, normalizing produces consistent output
    #[test]
    fn prop_arabic_normalization_alef_maksura(s in arabic_string_with_alef_maksura()) {
        let normalized = normalize_arabic(&s);

        // Normalizing again should produce the same result
        let double_normalized = normalize_arabic(&normalized);
        prop_assert_eq!(&normalized, &double_normalized,
            "Normalization should be idempotent");

        // Normalized string should not contain alef maksura
        prop_assert!(!normalized.contains('ى'), "Should not contain ى (alef maksura)");
    }
}

// =============================================================================
// Property 6: Arabic Phonetic Key Grouping
// =============================================================================
// For any two Arabic medication names that differ only by common phonetic
// variations (e.g., missing alef, hamza position), their phonetic keys
// SHALL be identical.
// Validates: Requirements 2.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 6: Arabic Phonetic Key Grouping
    /// Validates: Requirements 2.2
    ///
    /// For any two strings differing only by phonetic variations,
    /// their phonetic keys should be identical
    #[test]
    fn prop_phonetic_key_grouping((s1, s2) in phonetically_equivalent_pair()) {
        let key1 = phonetic_key(&s1);
        let key2 = phonetic_key(&s2);

        prop_assert_eq!(key1, key2,
            "Phonetically equivalent strings should have same key: '{}' -> '{}', '{}' -> '{}'",
            s1, phonetic_key(&s1), s2, phonetic_key(&s2));
    }

    /// Feature: matching-system-improvements, Property 6: Arabic Phonetic Key Grouping
    /// Validates: Requirements 2.2
    ///
    /// Phonetic key generation should be deterministic
    #[test]
    fn prop_phonetic_key_deterministic(s in arabic_string()) {
        let key1 = phonetic_key(&s);
        let key2 = phonetic_key(&s);

        prop_assert_eq!(key1, key2,
            "Phonetic key should be deterministic for '{}'", s);
    }

    /// Feature: matching-system-improvements, Property 6: Arabic Phonetic Key Grouping
    /// Validates: Requirements 2.2
    ///
    /// ArabicPhoneticMatcher should produce same results as standalone function
    #[test]
    fn prop_phonetic_matcher_consistency(s in arabic_string()) {
        let matcher = ArabicPhoneticMatcher::new();
        let matcher_key = matcher.phonetic_key(&s);

        // Note: matcher.phonetic_key normalizes first, so we compare with normalized input
        let normalized = normalize_arabic(&s);
        let standalone_normalized_key = phonetic_key(&normalized);

        prop_assert_eq!(matcher_key, standalone_normalized_key,
            "Matcher should produce consistent keys");
    }
}

// =============================================================================
// Property 7: Arabic-Aware Distance Selection
// =============================================================================
// For any string containing Arabic characters (Unicode range 0x0600-0x06FF),
// the fuzzy matching algorithm SHALL use Arabic-aware string distance
// rather than standard Levenshtein.
// Validates: Requirements 2.4

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// FuzzyStringStrategy::detect should return Arabic for Arabic text
    #[test]
    fn prop_arabic_detection(s in arabic_string()) {
        let strategy = FuzzyStringStrategy::detect(&s, &s);
        prop_assert_eq!(strategy, FuzzyStringStrategy::Arabic,
            "Arabic text '{}' should use Arabic strategy", s);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// FuzzyStringStrategy::detect should return Standard for English text
    #[test]
    fn prop_english_detection(s in english_string()) {
        let strategy = FuzzyStringStrategy::detect(&s, &s);
        prop_assert_eq!(strategy, FuzzyStringStrategy::Standard,
            "English text '{}' should use Standard strategy", s);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// contains_arabic should correctly identify Arabic text
    #[test]
    fn prop_contains_arabic_detection(s in arabic_string()) {
        prop_assert!(contains_arabic(&s),
            "Arabic string '{}' should be detected as containing Arabic", s);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// contains_arabic should correctly identify non-Arabic text
    #[test]
    fn prop_contains_arabic_english(s in english_string()) {
        prop_assert!(!contains_arabic(&s),
            "English string '{}' should not be detected as containing Arabic", s);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// Arabic-aware similarity should give high scores for phonetically equivalent strings
    #[test]
    fn prop_arabic_similarity_phonetic_equivalence((s1, s2) in phonetically_equivalent_pair()) {
        let sim = arabic_string_similarity(&s1, &s2);

        prop_assert!(sim > 0.8,
            "Phonetically equivalent strings should have high similarity: '{}' vs '{}' = {}",
            s1, s2, sim);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// Arabic-aware similarity should be symmetric
    #[test]
    fn prop_arabic_similarity_symmetric(s1 in arabic_string(), s2 in arabic_string()) {
        let sim1 = arabic_string_similarity(&s1, &s2);
        let sim2 = arabic_string_similarity(&s2, &s1);

        // Allow small floating point tolerance
        prop_assert!((sim1 - sim2).abs() < 0.001,
            "Similarity should be symmetric: sim('{}', '{}') = {} vs sim('{}', '{}') = {}",
            s1, s2, sim1, s2, s1, sim2);
    }

    /// Feature: matching-system-improvements, Property 7: Arabic-Aware Distance Selection
    /// Validates: Requirements 2.4
    ///
    /// Arabic-aware similarity should return 1.0 for identical strings
    #[test]
    fn prop_arabic_similarity_identity(s in arabic_string()) {
        let sim = arabic_string_similarity(&s, &s);

        prop_assert!((sim - 1.0).abs() < 0.001,
            "Identical strings should have similarity 1.0: '{}' = {}", s, sim);
    }
}
