//! Property-based tests for MatchingEngine integration
//!
//! Feature: matching-system-improvements
//! Tests Properties 8 and 10 from the design document
//!
//! These tests validate:
//! - Property 8: Dual-Language Matching
//! - Property 10: Class Mismatch Flagging

use pharma_core::matching::{ClassMismatchResult, MatchingEngine, contains_arabic};
use proptest::prelude::*;

// =============================================================================
// Custom Generators
// =============================================================================

/// Generate random Arabic medication names
fn arabic_medication_name() -> impl Strategy<Value = String> {
    let arabic_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";
    prop::collection::vec(
        Just(arabic_letters.chars().collect::<Vec<_>>()).prop_flat_map(prop::sample::select),
        3..12,
    )
    .prop_map(|v| v.into_iter().collect())
}

/// Generate random English medication names
fn english_medication_name() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,10}"
}

/// Generate a pair of Arabic medication names with phonetic variations
fn phonetically_similar_arabic_pair() -> impl Strategy<Value = (String, String)> {
    // Phonetic pairs: (variant, canonical)
    let phonetic_pairs = [
        ('ص', 'س'), // Emphatic S
        ('ض', 'د'), // Emphatic D
        ('ط', 'ت'), // Emphatic T
        ('ق', 'ك'), // Q/K confusion
    ];

    let base_letters = "ابتثجحخدذرزسشصضطظعغفقكلمنهوي";

    (3..8usize, prop::sample::select(phonetic_pairs.to_vec())).prop_flat_map(
        move |(len, (variant, canonical))| {
            prop::collection::vec(
                Just(base_letters.chars().collect::<Vec<_>>()).prop_flat_map(prop::sample::select),
                len,
            )
            .prop_flat_map(move |chars| {
                (0..chars.len()).prop_map(move |pos| {
                    let mut str1: Vec<char> = chars.clone();
                    let mut str2: Vec<char> = chars.clone();

                    str1.insert(pos, variant);
                    str2.insert(pos, canonical);

                    (str1.into_iter().collect(), str2.into_iter().collect())
                })
            })
        },
    )
}

/// Common therapeutic classes for testing
const THERAPEUTIC_CLASSES: &[&str] = &[
    "Antidiabetic",
    "Beta-blocker",
    "ACE-inhibitor",
    "Antibiotic",
    "Analgesic",
    "Antihistamine",
    "Antihypertensive",
    "Statin",
];

/// Generate a random therapeutic class
fn therapeutic_class() -> impl Strategy<Value = String> {
    prop::sample::select(THERAPEUTIC_CLASSES).prop_map(|s| s.to_string())
}

/// Generate a pair of different therapeutic classes
fn different_therapeutic_classes() -> impl Strategy<Value = (String, String)> {
    (therapeutic_class(), therapeutic_class())
        .prop_filter("Classes must be different", |(c1, c2)| c1 != c2)
}

/// Generate a high embedding similarity score (>0.8)
fn high_similarity_score() -> impl Strategy<Value = f64> {
    0.81..=0.99f64
}

/// Generate a low embedding similarity score (<0.8)
fn low_similarity_score() -> impl Strategy<Value = f64> {
    0.0..0.79f64
}

/// Generate a very high embedding similarity score (>0.9)
fn very_high_similarity_score() -> impl Strategy<Value = f64> {
    0.91..=0.99f64
}

// =============================================================================
// Property 8: Dual-Language Matching
// =============================================================================
// For any medication with both Arabic and English names, the matching score
// SHALL be the maximum of scores computed from both representations.
// Validates: Requirements 2.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Feature: matching-system-improvements, Property 8: Dual-Language Matching
    /// Validates: Requirements 2.5
    ///
    /// For Arabic medication names, dual-language scoring should use Arabic phonetic matching
    #[test]
    fn prop_dual_language_uses_arabic_for_arabic_text(
        arabic_name in arabic_medication_name()
    ) {
        let engine = MatchingEngine::default();

        // Verify the medication is detected as Arabic
        prop_assert!(contains_arabic(&arabic_name),
            "Generated name '{}' should contain Arabic", arabic_name);

        // Dual-language score should be at least as high as base score
        let base_score = 0.5;
        let dual_score = engine.get_dual_language_score(&arabic_name, &arabic_name, base_score);

        // For identical Arabic strings, the Arabic phonetic score should be 1.0
        // So dual_score should be max(0.5, 1.0) = 1.0
        prop_assert!(dual_score >= base_score,
            "Dual-language score {} should be >= base score {} for '{}'",
            dual_score, base_score, arabic_name);
    }

    /// Feature: matching-system-improvements, Property 8: Dual-Language Matching
    /// Validates: Requirements 2.5
    ///
    /// For English medication names, dual-language scoring should return base score
    #[test]
    fn prop_dual_language_returns_base_for_english(
        english_name in english_medication_name(),
        base_score in 0.0..=1.0f64
    ) {
        let engine = MatchingEngine::default();

        // Verify the medication is not Arabic
        prop_assert!(!contains_arabic(&english_name),
            "Generated name '{}' should not contain Arabic", english_name);

        // For non-Arabic text, dual-language score should equal base score
        let dual_score = engine.get_dual_language_score(&english_name, &english_name, base_score);

        prop_assert!((dual_score - base_score).abs() < 0.001,
            "Dual-language score {} should equal base score {} for English '{}'",
            dual_score, base_score, english_name);
    }

    /// Feature: matching-system-improvements, Property 8: Dual-Language Matching
    /// Validates: Requirements 2.5
    ///
    /// For phonetically similar Arabic names, dual-language scoring should give high scores
    #[test]
    fn prop_dual_language_high_score_for_phonetic_variants(
        (name1, name2) in phonetically_similar_arabic_pair()
    ) {
        let engine = MatchingEngine::default();

        // Use a low base score to verify Arabic matching improves it
        let base_score = 0.3;
        let dual_score = engine.get_dual_language_score(&name1, &name2, base_score);

        // Phonetically similar Arabic names should get a higher score
        prop_assert!(dual_score > base_score,
            "Dual-language score {} should be > base score {} for phonetically similar '{}' vs '{}'",
            dual_score, base_score, name1, name2);
    }

    /// Feature: matching-system-improvements, Property 8: Dual-Language Matching
    /// Validates: Requirements 2.5
    ///
    /// Dual-language scoring should be symmetric
    #[test]
    fn prop_dual_language_symmetric(
        name1 in arabic_medication_name(),
        name2 in arabic_medication_name(),
        base_score in 0.0..=1.0f64
    ) {
        let engine = MatchingEngine::default();

        let score1 = engine.get_dual_language_score(&name1, &name2, base_score);
        let score2 = engine.get_dual_language_score(&name2, &name1, base_score);

        prop_assert!((score1 - score2).abs() < 0.001,
            "Dual-language scoring should be symmetric: ({}, {}) = {} vs {} for base {}",
            name1, name2, score1, score2, base_score);
    }

    /// Feature: matching-system-improvements, Property 8: Dual-Language Matching
    /// Validates: Requirements 2.5
    ///
    /// Dual-language score should always be in valid range [0, 1]
    #[test]
    fn prop_dual_language_score_in_range(
        name1 in arabic_medication_name(),
        name2 in arabic_medication_name(),
        base_score in 0.0..=1.0f64
    ) {
        let engine = MatchingEngine::default();

        let dual_score = engine.get_dual_language_score(&name1, &name2, base_score);

        prop_assert!(dual_score >= 0.0 && dual_score <= 1.0,
            "Dual-language score {} should be in [0, 1] for '{}' vs '{}' with base {}",
            dual_score, name1, name2, base_score);
    }
}

// =============================================================================
// Property 10: Class Mismatch Flagging
// =============================================================================
// When embedding similarity > 0.8 but therapeutic classes differ,
// the match SHALL be flagged as suspicious.
// Validates: Requirements 3.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Feature: matching-system-improvements, Property 10: Class Mismatch Flagging
    /// Validates: Requirements 3.2
    ///
    /// High similarity with different classes should flag as suspicious
    #[test]
    fn prop_class_mismatch_flagged_when_high_similarity(
        med1 in english_medication_name(),
        med2 in english_medication_name(),
        (class1, class2) in different_therapeutic_classes(),
        similarity in high_similarity_score()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let engine = MatchingEngine::default();

            // Add medications with different classes
            engine.add_medication_class(&med1, &class1).await;
            engine.add_medication_class(&med2, &class2).await;
            engine.mark_class_index_built().await;

            // Detect class mismatch
            engine.detect_class_mismatch(&med1, &med2, similarity).await
        });

        prop_assert!(result.is_mismatch,
            "Should detect mismatch for '{}' ({}) vs '{}' ({}) with similarity {}",
            med1, class1, med2, class2, similarity);

        prop_assert!(result.suspicious,
            "Should flag as suspicious for different classes with high similarity");

        prop_assert!(result.reason.is_some(),
            "Should provide a reason for the mismatch");
    }

    /// Feature: matching-system-improvements, Property 10: Class Mismatch Flagging
    /// Validates: Requirements 3.2
    ///
    /// Low similarity should not trigger class mismatch check
    #[test]
    fn prop_class_mismatch_not_flagged_when_low_similarity(
        med1 in english_medication_name(),
        med2 in english_medication_name(),
        (class1, class2) in different_therapeutic_classes(),
        similarity in low_similarity_score()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let engine = MatchingEngine::default();

            // Add medications with different classes
            engine.add_medication_class(&med1, &class1).await;
            engine.add_medication_class(&med2, &class2).await;
            engine.mark_class_index_built().await;

            // Detect class mismatch
            engine.detect_class_mismatch(&med1, &med2, similarity).await
        });

        prop_assert!(!result.is_mismatch,
            "Should NOT detect mismatch for low similarity {} (< 0.8)", similarity);

        prop_assert!(!result.suspicious,
            "Should NOT flag as suspicious for low similarity");
    }

    /// Feature: matching-system-improvements, Property 10: Class Mismatch Flagging
    /// Validates: Requirements 3.2
    ///
    /// Same class should not trigger mismatch even with high similarity
    #[test]
    fn prop_same_class_not_flagged(
        med1 in english_medication_name(),
        med2 in english_medication_name(),
        class in therapeutic_class(),
        similarity in high_similarity_score()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let engine = MatchingEngine::default();

            // Add medications with SAME class
            engine.add_medication_class(&med1, &class).await;
            engine.add_medication_class(&med2, &class).await;
            engine.mark_class_index_built().await;

            // Detect class mismatch
            engine.detect_class_mismatch(&med1, &med2, similarity).await
        });

        prop_assert!(!result.is_mismatch,
            "Should NOT detect mismatch for same class '{}' with similarity {}",
            class, similarity);

        prop_assert!(!result.suspicious,
            "Should NOT flag as suspicious for same class");
    }

    /// Feature: matching-system-improvements, Property 10: Class Mismatch Flagging
    /// Validates: Requirements 3.2
    ///
    /// Very high similarity with unknown class should flag as suspicious
    #[test]
    fn prop_unknown_class_flagged_when_very_high_similarity(
        med1 in english_medication_name(),
        med2 in english_medication_name(),
        class in therapeutic_class(),
        similarity in very_high_similarity_score()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let engine = MatchingEngine::default();

            // Add only one medication with class
            engine.add_medication_class(&med1, &class).await;
            engine.mark_class_index_built().await;

            // Detect class mismatch (med2 has unknown class)
            engine.detect_class_mismatch(&med1, &med2, similarity).await
        });

        prop_assert!(result.is_mismatch,
            "Should detect mismatch for unknown class with very high similarity {}",
            similarity);

        prop_assert!(result.suspicious,
            "Should flag as suspicious for unknown class with very high similarity");
    }

    /// Feature: matching-system-improvements, Property 10: Class Mismatch Flagging
    /// Validates: Requirements 3.2
    ///
    /// ClassMismatchResult constructors should be consistent
    #[test]
    fn prop_class_mismatch_result_consistency(
        class1 in therapeutic_class(),
        class2 in therapeutic_class()
    ) {
        // Test no_mismatch constructor
        let no_mismatch = ClassMismatchResult::no_mismatch();
        prop_assert!(!no_mismatch.is_mismatch);
        prop_assert!(!no_mismatch.suspicious);
        prop_assert!(no_mismatch.offer_class.is_none());
        prop_assert!(no_mismatch.request_class.is_none());

        // Test mismatch constructor with both classes
        let mismatch = ClassMismatchResult::mismatch(
            Some(class1.clone()),
            Some(class2.clone())
        );
        prop_assert!(mismatch.is_mismatch);
        prop_assert!(mismatch.suspicious);
        prop_assert_eq!(mismatch.offer_class, Some(class1.clone()));
        prop_assert_eq!(mismatch.request_class, Some(class2.clone()));

        // Test mismatch constructor with one unknown class
        let partial = ClassMismatchResult::mismatch(Some(class1.clone()), None);
        prop_assert!(partial.is_mismatch);
        prop_assert!(partial.suspicious);
        prop_assert_eq!(partial.offer_class, Some(class1));
        prop_assert!(partial.request_class.is_none());
    }
}

// =============================================================================
// Integration Tests (Non-Property)
// =============================================================================

#[tokio::test]
async fn test_engine_blocklist_integration() {
    let engine = MatchingEngine::default();

    // Verify default blocklist is loaded
    let blocklist_len = engine.blocklist_len().await;
    assert!(blocklist_len > 0, "Default blocklist should have entries");

    // Check known dangerous pair
    let blocked = engine
        .is_medication_pair_blocked("Metformin", "Metoprolol")
        .await;
    assert!(blocked.is_some(), "Metformin/Metoprolol should be blocked");
}

#[tokio::test]
async fn test_engine_dosage_gate_integration() {
    use chrono::Utc;
    use pharma_core::domain::{Offer, Request};
    use rust_decimal::Decimal;
    use rust_decimal::prelude::FromPrimitive;

    let engine = MatchingEngine::default();

    let offer = Offer {
        medication: "Aspirin 100mg".to_string(),
        quantity: Decimal::from_f64(100.0),
        price: Decimal::from_f64(50.0),
        created_at: Utc::now(),
        ..Default::default()
    };

    let request = Request {
        medication: "Aspirin 500mg".to_string(), // Different dosage
        quantity: Decimal::from_f64(100.0),
        max_price: Decimal::from_f64(60.0),
        ..Default::default()
    };

    // Evaluate dosage gate with low dosage score
    let result = engine.evaluate_dosage_gate(&offer, &request, 0.3).await;

    // Should fail gate due to low dosage score
    assert!(
        !result.passed,
        "Dosage gate should fail for low dosage score"
    );
}

#[tokio::test]
async fn test_engine_arabic_matching_integration() {
    let engine = MatchingEngine::default();

    // Test Arabic phonetic similarity
    let sim = engine.get_arabic_phonetic_similarity("ميتفورمين", "متفورمين");
    assert!(
        sim > 0.8,
        "Phonetically similar Arabic names should have high similarity"
    );

    // Test Arabic detection
    assert!(engine.medication_contains_arabic("ميتفورمين"));
    assert!(!engine.medication_contains_arabic("Metformin"));
}

#[tokio::test]
async fn test_engine_class_index_bulk_load() {
    let engine = MatchingEngine::default();

    let medications = vec![
        ("Metformin".to_string(), "Antidiabetic".to_string()),
        ("Glipizide".to_string(), "Antidiabetic".to_string()),
        ("Metoprolol".to_string(), "Beta-blocker".to_string()),
        ("Atenolol".to_string(), "Beta-blocker".to_string()),
        ("Lisinopril".to_string(), "ACE-inhibitor".to_string()),
    ];

    engine.load_medication_classes(&medications).await;

    assert!(engine.is_class_index_ready().await);
    assert_eq!(engine.class_index_medication_count().await, 5);
    assert_eq!(engine.class_index_class_count().await, 3);

    // Test class lookup
    let class = engine.get_medication_class("Metformin").await;
    assert_eq!(class, Some("antidiabetic".to_string()));
}
