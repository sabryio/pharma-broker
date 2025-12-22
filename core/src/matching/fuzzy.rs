//! Fuzzy string matching utilities
//!
//! Provides distance and similarity metrics for medication names.

use super::arabic::normalize_for_matching;
use strsim::jaro_winkler;

/// Calculate similarity between two medication names (0.0 to 1.0)
///
/// Uses:
/// 1. Arabic normalization
/// 2. Lowercasing
/// 3. Jaro-Winkler similarity
pub fn medication_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_for_matching(a);
    let norm_b = normalize_for_matching(b);

    // If identical after normalization, it's a perfect match
    if norm_a == norm_b {
        return 1.0;
    }

    // Otherwise use Jaro-Winkler for fuzzy similarity
    // Jaro-Winkler is preferred for short strings/names over Levenshtein
    jaro_winkler(&norm_a, &norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_match() {
        assert_eq!(medication_similarity("Aspirin", "aspirin"), 1.0);
        assert_eq!(medication_similarity("أوجمنتين", "أُوجْمَنْتِين"), 1.0);
        // Test unit cleanup
        assert_eq!(medication_similarity("Panadol 500mg", "Panadol"), 1.0);
        assert_eq!(medication_similarity("Concor 5 mg", "Concor"), 1.0);
    }

    #[test]
    fn test_typo_match() {
        let sim = medication_similarity("Aspirin", "Aspriin");
        assert!(sim > 0.9, "Typo should have high similarity: {}", sim);
    }

    #[test]
    fn test_arabic_variation_match() {
        // With normalization, these should be identical (1.0)
        assert_eq!(medication_similarity("أوجمنتين", "اوجمنتين"), 1.0);
    }

    #[test]
    fn test_unrelated_strings() {
        let sim = medication_similarity("Panadol", "Augmentin");
        assert!(
            sim < 0.6,
            "Unrelated strings should have low similarity: {}",
            sim
        );
    }
}
