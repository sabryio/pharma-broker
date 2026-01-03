//! Fuzzy string matching utilities
//!
//! Provides distance and similarity metrics for medication names.
//! Enhanced with n-gram and phonetic (Soundex) matching.
//! Includes Arabic-aware string distance for better Arabic medication matching.

use super::arabic::{contains_arabic, normalize_for_matching, phonetic_key};
use std::collections::HashSet;
use strsim::jaro_winkler;

/// Calculate n-gram similarity between two strings (0.0 to 1.0)
/// Uses character n-grams (default n=2 for bigrams)
fn ngram_similarity(a: &str, b: &str, n: usize) -> f64 {
    if a.is_empty() || b.is_empty() {
        return if a.is_empty() && b.is_empty() {
            1.0
        } else {
            0.0
        };
    }
    if a.len() < n || b.len() < n {
        // Fall back to exact match for very short strings
        return if a == b { 1.0 } else { 0.0 };
    }

    let ngrams_a: HashSet<&str> = (0..=a.len().saturating_sub(n))
        .filter_map(|i| a.get(i..i + n))
        .collect();
    let ngrams_b: HashSet<&str> = (0..=b.len().saturating_sub(n))
        .filter_map(|i| b.get(i..i + n))
        .collect();

    if ngrams_a.is_empty() && ngrams_b.is_empty() {
        return 1.0;
    }

    let intersection = ngrams_a.intersection(&ngrams_b).count();
    let union = ngrams_a.union(&ngrams_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Generate Soundex code for phonetic matching
/// Implementation based on American Soundex algorithm
fn soundex(s: &str) -> String {
    let s = s.to_uppercase();
    let chars: Vec<char> = s.chars().filter(|c| c.is_ascii_alphabetic()).collect();

    if chars.is_empty() {
        return String::from("0000");
    }

    let first = chars[0];

    let code = |c: char| -> Option<char> {
        match c {
            'B' | 'F' | 'P' | 'V' => Some('1'),
            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => Some('2'),
            'D' | 'T' => Some('3'),
            'L' => Some('4'),
            'M' | 'N' => Some('5'),
            'R' => Some('6'),
            _ => None, // A, E, I, O, U, H, W, Y are ignored
        }
    };

    let mut result = String::with_capacity(4);
    result.push(first);

    let mut prev_code = code(first);

    for &c in &chars[1..] {
        if let Some(cc) = code(c) {
            if Some(cc) != prev_code {
                result.push(cc);
                if result.len() == 4 {
                    break;
                }
            }
            prev_code = Some(cc);
        }
    }

    // Pad with zeros to length 4
    while result.len() < 4 {
        result.push('0');
    }

    result
}

/// Calculate phonetic similarity using Soundex codes
fn phonetic_similarity(a: &str, b: &str) -> f64 {
    let soundex_a = soundex(a);
    let soundex_b = soundex(b);

    if soundex_a == soundex_b {
        return 1.0;
    }

    // Partial match: count matching characters in Soundex code
    let matches = soundex_a
        .chars()
        .zip(soundex_b.chars())
        .filter(|(ca, cb)| ca == cb)
        .count();

    matches as f64 / 4.0
}

/// Arabic-aware edit distance that accounts for letter variations
///
/// This function calculates a modified Levenshtein distance that treats
/// phonetically similar Arabic letters as having a lower substitution cost.
///
/// # Arguments
/// * `a` - First string
/// * `b` - Second string
///
/// # Returns
/// The edit distance as a usize
pub fn arabic_edit_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Create distance matrix
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    // Initialize first row and column
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) {
        *val = j;
    }

    // Fill the matrix
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else if are_phonetically_similar(a_chars[i - 1], b_chars[j - 1]) {
                // Reduced cost for phonetically similar Arabic letters
                // We use 0 cost to treat them as equivalent
                0
            } else {
                1
            };

            dp[i][j] = (dp[i - 1][j] + 1) // deletion
                .min(dp[i][j - 1] + 1) // insertion
                .min(dp[i - 1][j - 1] + cost); // substitution
        }
    }

    dp[m][n]
}

/// Check if two Arabic characters are phonetically similar
///
/// Returns true if the characters belong to the same phonetic group
fn are_phonetically_similar(a: char, b: char) -> bool {
    // Define phonetic groups
    let groups: &[&[char]] = &[
        // Emphatic vs non-emphatic S sounds
        &['س', 'ص'],
        // Emphatic vs non-emphatic D sounds
        &['د', 'ض'],
        // Emphatic vs non-emphatic T sounds
        &['ت', 'ط'],
        // Emphatic vs non-emphatic TH/Z sounds
        &['ذ', 'ظ'],
        // Guttural H sounds
        &['ح', 'ه'],
        // Glottal sounds
        &['ع', 'ء', 'ا', 'أ', 'إ', 'آ', 'ٱ'],
        // Q/K sounds
        &['ق', 'ك'],
        // Taa marbuta and ha
        &['ة', 'ه'],
        // Alef maksura and ya
        &['ى', 'ي'],
    ];

    for group in groups {
        if group.contains(&a) && group.contains(&b) {
            return true;
        }
    }

    false
}

/// Calculate Arabic-aware string similarity (0.0 to 1.0)
///
/// Uses Arabic edit distance normalized by the maximum string length.
/// Also incorporates phonetic key comparison for better matching.
pub fn arabic_string_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Normalize both strings
    let norm_a = normalize_for_matching(a);
    let norm_b = normalize_for_matching(b);

    // If identical after normalization, perfect match
    if norm_a == norm_b {
        return 1.0;
    }

    // Calculate Arabic edit distance
    let distance = arabic_edit_distance(&norm_a, &norm_b);
    let max_len = norm_a.chars().count().max(norm_b.chars().count());

    // Convert distance to similarity
    let edit_sim = if max_len == 0 {
        1.0
    } else {
        1.0 - (distance as f64 / max_len as f64)
    };

    // Also compare phonetic keys
    let key_a = phonetic_key(&norm_a);
    let key_b = phonetic_key(&norm_b);

    let phonetic_sim = if key_a == key_b {
        1.0
    } else {
        jaro_winkler(&key_a, &key_b)
    };

    // Return the maximum of edit-based and phonetic similarity
    edit_sim.max(phonetic_sim)
}

/// Fuzzy string matching strategy that automatically selects
/// the appropriate algorithm based on text content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FuzzyStringStrategy {
    /// Standard Levenshtein/Jaro-Winkler for non-Arabic text
    #[default]
    Standard,
    /// Arabic-aware distance for Arabic text
    Arabic,
    /// Automatically detect based on text content
    Auto,
}

impl FuzzyStringStrategy {
    /// Calculate similarity using the selected strategy
    pub fn similarity(&self, a: &str, b: &str) -> f64 {
        match self {
            FuzzyStringStrategy::Standard => standard_string_similarity(a, b),
            FuzzyStringStrategy::Arabic => arabic_string_similarity(a, b),
            FuzzyStringStrategy::Auto => {
                // Detect if either string contains Arabic
                if contains_arabic(a) || contains_arabic(b) {
                    arabic_string_similarity(a, b)
                } else {
                    standard_string_similarity(a, b)
                }
            }
        }
    }

    /// Detect the appropriate strategy based on text content
    pub fn detect(a: &str, b: &str) -> Self {
        if contains_arabic(a) || contains_arabic(b) {
            FuzzyStringStrategy::Arabic
        } else {
            FuzzyStringStrategy::Standard
        }
    }
}

/// Standard string similarity using Jaro-Winkler
fn standard_string_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_for_matching(a);
    let norm_b = normalize_for_matching(b);

    if norm_a == norm_b {
        return 1.0;
    }

    jaro_winkler(&norm_a, &norm_b)
}

/// Calculate similarity between two medication names (0.0 to 1.0)
///
/// Uses ensemble of algorithms:
/// 1. Arabic normalization
/// 2. Jaro-Winkler similarity (best for typos)
/// 3. N-gram similarity (best for partial matches)
/// 4. Phonetic/Soundex similarity (best for pronunciation-based errors)
/// 5. Arabic-aware distance (for Arabic text)
pub fn medication_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize_for_matching(a);
    let norm_b = normalize_for_matching(b);

    // If identical after normalization, it's a perfect match
    if norm_a == norm_b {
        return 1.0;
    }

    // Check if we should use Arabic-aware matching
    let use_arabic = contains_arabic(a) || contains_arabic(b);

    if use_arabic {
        // Use Arabic-aware similarity for Arabic text
        let arabic_sim = arabic_string_similarity(a, b);

        // Also calculate standard similarity as a fallback
        let jw = jaro_winkler(&norm_a, &norm_b);
        let ngram = ngram_similarity(&norm_a, &norm_b, 2);

        // Weighted ensemble for Arabic: Arabic (50%) + JW (30%) + N-gram (20%)
        let ensemble = arabic_sim * 0.50 + jw * 0.30 + ngram * 0.20;

        // Return the best of ensemble, Arabic similarity, or JW
        return ensemble.max(arabic_sim).max(jw);
    }

    // Standard matching for non-Arabic text
    // Jaro-Winkler for typo detection (highest weight)
    let jw = jaro_winkler(&norm_a, &norm_b);

    // N-gram similarity for partial matches (bigrams)
    let ngram = ngram_similarity(&norm_a, &norm_b, 2);

    // Phonetic similarity for pronunciation-based errors
    let phonetic = phonetic_similarity(&norm_a, &norm_b);

    // Weighted ensemble: JW (50%) + N-gram (30%) + Phonetic (20%)
    // Use max of ensemble score and pure JW to avoid regression
    let ensemble = jw * 0.50 + ngram * 0.30 + phonetic * 0.20;

    // Return the better of ensemble or pure JW
    ensemble.max(jw)
}

/// Calculate similarity with explicit algorithm weights
/// Allows callers to adjust the ensemble weights
pub fn _medication_similarity_weighted(
    a: &str,
    b: &str,
    jw_weight: f64,
    ngram_weight: f64,
    phonetic_weight: f64,
) -> f64 {
    let norm_a = normalize_for_matching(a);
    let norm_b = normalize_for_matching(b);

    if norm_a == norm_b {
        return 1.0;
    }

    let jw = jaro_winkler(&norm_a, &norm_b);
    let ngram = ngram_similarity(&norm_a, &norm_b, 2);
    let phonetic = phonetic_similarity(&norm_a, &norm_b);

    let total_weight = jw_weight + ngram_weight + phonetic_weight;
    if total_weight == 0.0 {
        return jw;
    }

    (jw * jw_weight + ngram * ngram_weight + phonetic * phonetic_weight) / total_weight
}

/// Calculate medication similarity using both parsed and raw text
///
/// This function addresses the issue where AI-parsed medication names may be
/// incorrectly transliterated, leading to false positive matches.
///
/// Strategy:
/// 1. Calculate similarity on parsed names (AI output)
/// 2. Calculate similarity on raw names (original Arabic/user input)
/// 3. If raw names are very different (< 0.5), penalize the score heavily
/// 4. Use the minimum of parsed and raw similarity as a gate
///
/// This prevents matches like "Kozentex 150" ↔ "Gonapure 150" when the
/// raw Arabic texts "كوزنتكس" and "جونابيور" are completely different.
pub fn medication_similarity_with_raw(
    parsed_a: &str,
    parsed_b: &str,
    raw_a: Option<&str>,
    raw_b: Option<&str>,
) -> f64 {
    // Calculate parsed name similarity
    let parsed_sim = medication_similarity(parsed_a, parsed_b);

    // If no raw text available, return parsed similarity
    let (raw_a, raw_b) = match (raw_a, raw_b) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => (a, b),
        _ => return parsed_sim,
    };

    // Calculate raw text similarity
    let raw_sim = medication_similarity(raw_a, raw_b);

    // Log for debugging
    tracing::debug!(
        parsed_a = %parsed_a,
        parsed_b = %parsed_b,
        raw_a = %raw_a,
        raw_b = %raw_b,
        parsed_sim = %parsed_sim,
        raw_sim = %raw_sim,
        "Medication similarity with raw text"
    );

    // If raw texts are very different, this is likely a false positive
    // from AI hallucination/mistransliteration
    if raw_sim < 0.3 {
        // Raw texts are completely different - this is NOT a match
        // Return a very low score regardless of parsed similarity
        tracing::warn!(
            parsed_a = %parsed_a,
            parsed_b = %parsed_b,
            raw_a = %raw_a,
            raw_b = %raw_b,
            parsed_sim = %parsed_sim,
            raw_sim = %raw_sim,
            "Rejecting match due to raw text mismatch (likely AI hallucination)"
        );
        return raw_sim * 0.5; // Heavily penalized
    }

    if raw_sim < 0.5 {
        // Raw texts are quite different - penalize but don't reject
        // Use geometric mean to balance both scores
        let combined = (parsed_sim * raw_sim).sqrt();
        return combined.min(raw_sim + 0.1); // Cap at raw_sim + small bonus
    }

    // Raw texts are similar enough - use weighted average
    // Give more weight to raw similarity to prevent AI hallucination issues
    parsed_sim * 0.4 + raw_sim * 0.6
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

    #[test]
    fn test_ngram_similarity() {
        // Exact match
        assert_eq!(ngram_similarity("test", "test", 2), 1.0);
        // No overlap
        assert_eq!(ngram_similarity("abc", "xyz", 2), 0.0);
        // Partial overlap
        let sim = ngram_similarity("testing", "tested", 2);
        assert!(sim > 0.3 && sim < 0.8, "Partial overlap: {}", sim);
    }

    #[test]
    fn test_soundex() {
        // Same Soundex code for similar sounding names
        assert_eq!(soundex("Robert"), soundex("Rupert"));
        assert_eq!(soundex("Smith"), soundex("Smythe"));
        // Different Soundex for different sounds
        assert_ne!(soundex("Aspirin"), soundex("Panadol"));
    }

    #[test]
    fn test_phonetic_similarity() {
        // Similar sounding should have high score
        let sim = phonetic_similarity("Aspirin", "Asprin");
        assert!(sim > 0.5, "Similar sounding: {}", sim);
    }

    #[test]
    fn test_phonetic_medication_matching() {
        // Common medication misspellings
        let sim = medication_similarity("Metformin", "Metformine");
        assert!(sim > 0.85, "Metformin variant: {}", sim);

        let sim = medication_similarity("Omeprazole", "Omeprazol");
        assert!(sim > 0.9, "Omeprazole variant: {}", sim);
    }

    #[test]
    fn test_medication_similarity_with_raw_different_meds() {
        // Kozentex vs Gonapure - completely different medications
        // AI might parse them similarly due to "150" but raw Arabic is different
        let sim = medication_similarity_with_raw(
            "Kozentex 150",
            "Gonapure 150",
            Some("كوزنتكس 150"),
            Some("جونابيور ١٥٠"),
        );
        // Should be penalized due to raw text mismatch
        // The score is reduced but not to zero because of the "150" in both
        assert!(
            sim < 0.6,
            "Different medications should have reduced similarity: {}",
            sim
        );
    }

    #[test]
    fn test_medication_similarity_with_raw_same_med() {
        // Same medication with slight variations
        let sim = medication_similarity_with_raw(
            "Augmentin 1g",
            "Augmentin 1000mg",
            Some("اوجمنتين 1 جرام"),
            Some("أوجمنتين ١٠٠٠"),
        );
        // Should be high because both parsed and raw are similar
        assert!(
            sim > 0.7,
            "Same medication should have high similarity: {}",
            sim
        );
    }

    #[test]
    fn test_medication_similarity_with_raw_no_raw() {
        // When raw text is not available, fall back to parsed similarity
        let sim = medication_similarity_with_raw("Panadol 500mg", "Panadol 500", None, None);
        // Should use parsed similarity
        assert!(
            sim > 0.9,
            "Same parsed name should have high similarity: {}",
            sim
        );
    }

    #[test]
    fn test_medication_similarity_with_raw_ai_hallucination() {
        // Simulating AI hallucination: parsed names look similar but raw is different
        let sim = medication_similarity_with_raw(
            "Metformin 500",
            "Metformine 500", // Slight variation in parsed
            Some("ميتفورمين"),
            Some("جلوكوفاج"), // Completely different raw (Glucophage)
        );
        // Should be penalized because raw texts are different
        // But not zero because parsed names are very similar
        assert!(sim < 0.6, "AI hallucination should be caught: {}", sim);
    }

    // Arabic-aware string distance tests
    #[test]
    fn test_arabic_edit_distance_identical() {
        assert_eq!(arabic_edit_distance("اوجمنتين", "اوجمنتين"), 0);
    }

    #[test]
    fn test_arabic_edit_distance_phonetically_similar() {
        // ص and س are phonetically similar, so distance should be 0
        let dist = arabic_edit_distance("صداع", "سداع");
        assert_eq!(dist, 0, "Phonetically similar should have 0 distance");
    }

    #[test]
    fn test_arabic_edit_distance_different() {
        // Completely different strings
        let dist = arabic_edit_distance("اوجمنتين", "بانادول");
        assert!(dist > 0, "Different strings should have positive distance");
    }

    #[test]
    fn test_are_phonetically_similar() {
        // Same phonetic group
        assert!(are_phonetically_similar('س', 'ص'));
        assert!(are_phonetically_similar('د', 'ض'));
        assert!(are_phonetically_similar('ت', 'ط'));
        assert!(are_phonetically_similar('ق', 'ك'));

        // Different phonetic groups
        assert!(!are_phonetically_similar('س', 'ب'));
        assert!(!are_phonetically_similar('ا', 'ب'));
    }

    #[test]
    fn test_arabic_string_similarity_identical() {
        let sim = arabic_string_similarity("اوجمنتين", "اوجمنتين");
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_arabic_string_similarity_phonetically_equivalent() {
        // كتافلام vs قتافلام (k vs q)
        let sim = arabic_string_similarity("كتافلام", "قتافلام");
        assert!(
            sim > 0.9,
            "Phonetically equivalent should have high similarity: {}",
            sim
        );
    }

    #[test]
    fn test_arabic_string_similarity_different() {
        let sim = arabic_string_similarity("اوجمنتين", "بانادول");
        assert!(
            sim < 0.7,
            "Different strings should have lower similarity: {}",
            sim
        );
    }

    #[test]
    fn test_fuzzy_string_strategy_detect() {
        // Arabic text should use Arabic strategy
        assert_eq!(
            FuzzyStringStrategy::detect("اوجمنتين", "بانادول"),
            FuzzyStringStrategy::Arabic
        );

        // English text should use Standard strategy
        assert_eq!(
            FuzzyStringStrategy::detect("Augmentin", "Panadol"),
            FuzzyStringStrategy::Standard
        );

        // Mixed text should use Arabic strategy
        assert_eq!(
            FuzzyStringStrategy::detect("Augmentin اوجمنتين", "Panadol"),
            FuzzyStringStrategy::Arabic
        );
    }

    #[test]
    fn test_fuzzy_string_strategy_similarity() {
        // Auto strategy should detect and use appropriate algorithm
        let auto = FuzzyStringStrategy::Auto;

        // Arabic text
        let sim = auto.similarity("كتافلام", "قتافلام");
        assert!(sim > 0.9, "Arabic similarity should be high: {}", sim);

        // English text
        let sim = auto.similarity("Aspirin", "Aspriin");
        assert!(sim > 0.9, "English similarity should be high: {}", sim);
    }

    #[test]
    fn test_medication_similarity_uses_arabic_for_arabic_text() {
        // Phonetically equivalent Arabic medications should have high similarity
        let sim = medication_similarity("كتافلام", "قتافلام");
        assert!(
            sim > 0.9,
            "Arabic medications with phonetic equivalence should match: {}",
            sim
        );
    }
}
