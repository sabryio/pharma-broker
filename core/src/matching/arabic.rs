//! Arabic text normalization utilities
//!
//! Ported from legacy/pkg/arabic/normalizer.go
//!
//! Enhanced with phonetic grouping for Arabic medication matching.
//! Supports:
//! - Standard Arabic normalization (diacritics, alef variants, etc.)
//! - Phonetic key generation for similar-sounding Arabic letters
//! - Common medication spelling variations mapping

use std::collections::HashMap;

// Use a regex to strip trailing dosage info (numbers + units)
// Supports both English and Arabic units/numerals
// Matches patterns like " 500mg", " 0.5 g", " ١٠٠ملغ", etc. at the end of the string
lazy_static::lazy_static! {
    static ref DOSAGE_STRIP: regex::Regex = regex::Regex::new(
        r"(?i)\s+[\d٠-٩.,/]+\s*(ميكروغرام|مايكروجرام|جرام|غرام|ملغ|ملجم|وحدة|mcg|μg|ug|mg|ml|iu|ui|مل|ج|g|vial|tablet|ampoule)?\s*$"
    ).unwrap();

    /// Phonetic group mappings for similar-sounding Arabic letters
    /// Maps letters to their phonetic group representative
    static ref PHONETIC_GROUPS: HashMap<char, char> = {
        let mut m = HashMap::new();
        // Group 1: Emphatic vs non-emphatic S sounds
        // س (sin) and ص (sad) - both S-like sounds
        m.insert('ص', 'س');

        // Group 2: Emphatic vs non-emphatic D sounds
        // د (dal) and ض (dad) - both D-like sounds
        m.insert('ض', 'د');

        // Group 3: Emphatic vs non-emphatic T sounds
        // ت (ta) and ط (ta emphatic) - both T-like sounds
        m.insert('ط', 'ت');

        // Group 4: Emphatic vs non-emphatic TH/Z sounds
        // ذ (dhal) and ظ (za emphatic) - both TH/Z-like sounds
        m.insert('ظ', 'ذ');

        // Group 5: Guttural sounds
        // ح (ha) and ه (ha) - both H-like sounds
        m.insert('ح', 'ه');

        // Group 6: Glottal sounds
        // ع (ain) and ء (hamza) - both glottal sounds
        m.insert('ع', 'ء');

        // Group 7: Q/K sounds (common confusion in dialects)
        // ق (qaf) and ك (kaf) - often confused in Egyptian/Levantine dialects
        m.insert('ق', 'ك');

        // Group 8: Alef variants (already normalized, but include for completeness)
        m.insert('أ', 'ا');
        m.insert('إ', 'ا');
        m.insert('آ', 'ا');
        m.insert('ٱ', 'ا');

        // Group 9: Taa marbuta to ha
        m.insert('ة', 'ه');

        // Group 10: Alef maksura to ya
        m.insert('ى', 'ي');

        m
    };

    /// Common medication spelling variations mapping
    /// Maps variant spellings to canonical forms
    static ref MEDICATION_VARIATIONS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        // Augmentin variations
        m.insert("اوجمنتين", "اوجمنتين");
        m.insert("اوغمنتين", "اوجمنتين");
        m.insert("اوقمنتين", "اوجمنتين");
        m.insert("اجمنتين", "اوجمنتين");

        // Panadol variations
        m.insert("بانادول", "بانادول");
        m.insert("باندول", "بانادول");
        m.insert("بنادول", "بانادول");

        // Brufen variations
        m.insert("بروفين", "بروفين");
        m.insert("بروفن", "بروفين");
        m.insert("برفين", "بروفين");

        // Amoxicillin variations
        m.insert("اموكسيسيلين", "اموكسيسيلين");
        m.insert("اموكسسيلين", "اموكسيسيلين");
        m.insert("اموكسيلين", "اموكسيسيلين");

        // Metformin variations
        m.insert("ميتفورمين", "ميتفورمين");
        m.insert("متفورمين", "ميتفورمين");
        m.insert("ميتفرمين", "ميتفورمين");

        // Omeprazole variations
        m.insert("اوميبرازول", "اوميبرازول");
        m.insert("اومبرازول", "اوميبرازول");
        m.insert("اوميبرزول", "اوميبرازول");

        // Aspirin variations
        m.insert("اسبرين", "اسبرين");
        m.insert("اسبيرين", "اسبرين");
        m.insert("اصبرين", "اسبرين");

        // Cataflam variations
        m.insert("كتافلام", "كتافلام");
        m.insert("كاتافلام", "كتافلام");
        m.insert("قتافلام", "كتافلام");

        // Voltaren variations
        m.insert("فولتارين", "فولتارين");
        m.insert("فلتارين", "فولتارين");
        m.insert("فولترين", "فولتارين");

        // Concor variations
        m.insert("كونكور", "كونكور");
        m.insert("كنكور", "كونكور");
        m.insert("قونكور", "كونكور");

        m
    };
}

/// Arabic diacritics (tashkeel) to remove
const DIACRITICS: &[char] = &[
    '\u{064B}', // Fathatan
    '\u{064C}', // Dammatan
    '\u{064D}', // Kasratan
    '\u{064E}', // Fatha
    '\u{064F}', // Damma
    '\u{0650}', // Kasra
    '\u{0651}', // Shadda
    '\u{0652}', // Sukun
    '\u{0670}', // Superscript Alef
];

/// Check if a character is Arabic (Unicode range 0x0600-0x06FF)
pub fn is_arabic_char(c: char) -> bool {
    let code = c as u32;
    (0x0600..=0x06FF).contains(&code)
}

/// Check if a string contains Arabic characters
pub fn contains_arabic(text: &str) -> bool {
    text.chars().any(is_arabic_char)
}

/// Normalize Arabic text for better matching:
/// - Removes diacritics (tashkeel)
/// - Normalizes Alef variants (أ إ آ ٱ → ا)
/// - Normalizes Taa Marbuta (ة → ه)
/// - Normalizes Alef Maksura (ى → ي)
/// - Removes Tatweel (ـ)
pub fn normalize_arabic(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for c in text.chars() {
        // Skip diacritics
        if DIACRITICS.contains(&c) {
            continue;
        }

        // Skip Tatweel
        if c == 'ـ' {
            continue;
        }

        // Normalize variants
        match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => result.push('ا'),
            'ة' => result.push('ه'),
            'ى' => result.push('ي'),
            _ => result.push(c),
        }
    }

    result
}

/// Generate a phonetic key for Arabic text
///
/// The phonetic key groups similar-sounding Arabic letters together,
/// making it easier to match medication names with common spelling variations.
///
/// This is similar to Soundex but adapted for Arabic phonetics.
///
/// # Example
/// ```
/// use pharma_core::matching::arabic::phonetic_key;
/// // صداع and سداع would have the same phonetic key
/// // because ص and س are in the same phonetic group
/// ```
pub fn phonetic_key(text: &str) -> String {
    // First normalize the text
    let normalized = normalize_arabic(text);

    let mut result = String::with_capacity(normalized.len());

    for c in normalized.chars() {
        // Apply phonetic grouping
        if let Some(&group_char) = PHONETIC_GROUPS.get(&c) {
            result.push(group_char);
        } else {
            result.push(c);
        }
    }

    result
}

/// Get the canonical form of a medication name if it's a known variation
///
/// Returns the canonical form if found, otherwise returns None
pub fn get_canonical_medication(text: &str) -> Option<&'static str> {
    let normalized = normalize_arabic(text);
    // Remove spaces for lookup
    let key: String = normalized.split_whitespace().collect();
    MEDICATION_VARIATIONS.get(key.as_str()).copied()
}

/// Normalize text for medication matching
/// Applies Arabic normalization, cleanups units, and lowercases
pub fn normalize_for_matching(text: &str) -> String {
    let mut normalized = normalize_arabic(text).to_lowercase();

    normalized = DOSAGE_STRIP.replace_all(&normalized, "").to_string();

    // Collapse multiple spaces
    normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    normalized.trim().to_string()
}

/// Normalize text for phonetic matching
/// Applies full phonetic normalization for maximum matching flexibility
pub fn normalize_for_phonetic_matching(text: &str) -> String {
    let base_normalized = normalize_for_matching(text);
    phonetic_key(&base_normalized)
}

/// Arabic phonetic matcher for medication names
///
/// This struct provides phonetic matching capabilities specifically designed
/// for Arabic medication names, handling common spelling variations and
/// transliteration differences.
///
/// # Example
/// ```
/// use pharma_core::matching::arabic::ArabicPhoneticMatcher;
///
/// let matcher = ArabicPhoneticMatcher::new();
/// let sim = matcher.phonetic_similarity("كتافلام", "قتافلام");
/// assert!(sim > 0.9); // High similarity due to phonetic grouping
/// ```
#[derive(Debug, Clone)]
pub struct ArabicPhoneticMatcher {
    /// Common spelling variations mapping (variant -> canonical)
    variations: HashMap<String, String>,
    /// Phonetic group mappings (letter -> group representative)
    phonetic_groups: HashMap<char, char>,
}

impl Default for ArabicPhoneticMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ArabicPhoneticMatcher {
    /// Create a new ArabicPhoneticMatcher with default phonetic groups and variations
    pub fn new() -> Self {
        // Copy phonetic groups from static
        let phonetic_groups: HashMap<char, char> = PHONETIC_GROUPS.clone();

        // Copy medication variations from static
        let variations: HashMap<String, String> = MEDICATION_VARIATIONS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Self {
            variations,
            phonetic_groups,
        }
    }

    /// Create a new ArabicPhoneticMatcher with custom phonetic groups and variations
    pub fn with_custom(
        phonetic_groups: HashMap<char, char>,
        variations: HashMap<String, String>,
    ) -> Self {
        Self {
            variations,
            phonetic_groups,
        }
    }

    /// Add a custom phonetic group mapping
    pub fn add_phonetic_group(&mut self, from: char, to: char) {
        self.phonetic_groups.insert(from, to);
    }

    /// Add a custom medication variation mapping
    pub fn add_variation(&mut self, variant: &str, canonical: &str) {
        let normalized_variant = normalize_arabic(variant);
        self.variations
            .insert(normalized_variant, canonical.to_string());
    }

    /// Generate a phonetic key for the given text
    ///
    /// The phonetic key groups similar-sounding Arabic letters together,
    /// making it easier to match medication names with common spelling variations.
    pub fn phonetic_key(&self, text: &str) -> String {
        let normalized = normalize_arabic(text);

        let mut result = String::with_capacity(normalized.len());

        for c in normalized.chars() {
            if let Some(&group_char) = self.phonetic_groups.get(&c) {
                result.push(group_char);
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Normalize text for matching (lowercase, strip dosage, normalize Arabic)
    pub fn normalize(&self, text: &str) -> String {
        normalize_for_matching(text)
    }

    /// Calculate phonetic similarity between two Arabic strings (0.0 to 1.0)
    ///
    /// This method:
    /// 1. Normalizes both strings
    /// 2. Generates phonetic keys
    /// 3. Compares the phonetic keys using Jaro-Winkler similarity
    ///
    /// Returns 1.0 for identical phonetic keys, lower values for less similar strings.
    pub fn phonetic_similarity(&self, a: &str, b: &str) -> f64 {
        let key_a = self.phonetic_key(&normalize_for_matching(a));
        let key_b = self.phonetic_key(&normalize_for_matching(b));

        // If phonetic keys are identical, perfect match
        if key_a == key_b {
            return 1.0;
        }

        // Use Jaro-Winkler for comparing phonetic keys
        strsim::jaro_winkler(&key_a, &key_b)
    }

    /// Check if two Arabic medication names are phonetic matches
    ///
    /// Returns true if the phonetic similarity is above the threshold (default 0.85)
    pub fn is_phonetic_match(&self, a: &str, b: &str) -> bool {
        self.is_phonetic_match_with_threshold(a, b, 0.85)
    }

    /// Check if two Arabic medication names are phonetic matches with custom threshold
    pub fn is_phonetic_match_with_threshold(&self, a: &str, b: &str, threshold: f64) -> bool {
        self.phonetic_similarity(a, b) >= threshold
    }

    /// Get the canonical form of a medication name if it's a known variation
    pub fn get_canonical(&self, text: &str) -> Option<&str> {
        let normalized = normalize_arabic(text);
        let key: String = normalized.split_whitespace().collect();
        self.variations.get(&key).map(|s| s.as_str())
    }

    /// Check if the text contains Arabic characters
    pub fn is_arabic(&self, text: &str) -> bool {
        contains_arabic(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_diacritics() {
        assert_eq!(normalize_arabic("أُوجْمَنْتِين"), "اوجمنتين");
    }

    #[test]
    fn test_normalize_alef_variants() {
        assert_eq!(normalize_arabic("أحمد"), "احمد");
        assert_eq!(normalize_arabic("إبراهيم"), "ابراهيم");
        assert_eq!(normalize_arabic("آدم"), "ادم");
    }

    #[test]
    fn test_normalize_taa_marbuta() {
        assert_eq!(normalize_arabic("علبة"), "علبه");
    }

    #[test]
    fn test_normalize_alef_maksura() {
        assert_eq!(normalize_arabic("مستشفى"), "مستشفي");
    }

    #[test]
    fn test_normalize_tatweel() {
        assert_eq!(normalize_arabic("أوجمـنتين"), "اوجمنتين");
    }

    #[test]
    fn test_normalize_for_matching() {
        assert_eq!(normalize_for_matching("أُوجْمَنْتِين"), "اوجمنتين");
        // Dosage is stripped for matching purposes
        assert_eq!(normalize_for_matching("Brufen 400mg"), "brufen");
        assert_eq!(normalize_for_matching("Augmentin 1g"), "augmentin");
        // Without dosage, just lowercased
        assert_eq!(normalize_for_matching("Panadol"), "panadol");
    }

    #[test]
    fn test_is_arabic_char() {
        assert!(is_arabic_char('ا'));
        assert!(is_arabic_char('ب'));
        assert!(is_arabic_char('ت'));
        assert!(!is_arabic_char('a'));
        assert!(!is_arabic_char('1'));
    }

    #[test]
    fn test_contains_arabic() {
        assert!(contains_arabic("اوجمنتين"));
        assert!(contains_arabic("Augmentin اوجمنتين"));
        assert!(!contains_arabic("Augmentin"));
        assert!(!contains_arabic("123"));
    }

    #[test]
    fn test_phonetic_key_emphatic_s() {
        // ص (sad) should map to س (sin)
        let key1 = phonetic_key("صداع");
        let key2 = phonetic_key("سداع");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_phonetic_key_emphatic_d() {
        // ض (dad) should map to د (dal)
        let key1 = phonetic_key("ضرب");
        let key2 = phonetic_key("درب");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_phonetic_key_emphatic_t() {
        // ط (ta emphatic) should map to ت (ta)
        let key1 = phonetic_key("طبيب");
        let key2 = phonetic_key("تبيب");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_phonetic_key_q_k_confusion() {
        // ق (qaf) should map to ك (kaf) - common in Egyptian dialect
        let key1 = phonetic_key("قلب");
        let key2 = phonetic_key("كلب");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_phonetic_key_preserves_normalization() {
        // Phonetic key should also normalize alef variants
        let key1 = phonetic_key("أوجمنتين");
        let key2 = phonetic_key("اوجمنتين");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_phonetic_key_medication_variations() {
        // Common medication spelling variations should have same phonetic key
        // Cataflam: كتافلام vs قتافلام (k vs q)
        let key1 = phonetic_key("كتافلام");
        let key2 = phonetic_key("قتافلام");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_get_canonical_medication() {
        // Known variations should return canonical form
        assert_eq!(get_canonical_medication("اوغمنتين"), Some("اوجمنتين"));
        assert_eq!(get_canonical_medication("باندول"), Some("بانادول"));
        assert_eq!(get_canonical_medication("برفين"), Some("بروفين"));

        // Unknown medications should return None
        assert_eq!(get_canonical_medication("دواء غير معروف"), None);
    }

    #[test]
    fn test_normalize_for_phonetic_matching() {
        // Should apply both normalization and phonetic grouping
        let result = normalize_for_phonetic_matching("قتافلام 50mg");
        // Should be lowercase, dosage stripped, and phonetic normalized
        assert!(!result.contains("50"));
        assert!(!result.contains("mg"));
        // ق should be mapped to ك
        assert!(!result.contains('ق'));
    }

    // ArabicPhoneticMatcher tests
    #[test]
    fn test_arabic_phonetic_matcher_new() {
        let matcher = ArabicPhoneticMatcher::new();
        // Should have phonetic groups loaded
        assert!(!matcher.phonetic_groups.is_empty());
        // Should have variations loaded
        assert!(!matcher.variations.is_empty());
    }

    #[test]
    fn test_arabic_phonetic_matcher_phonetic_key() {
        let matcher = ArabicPhoneticMatcher::new();

        // Same phonetic key for emphatic vs non-emphatic
        let key1 = matcher.phonetic_key("صداع");
        let key2 = matcher.phonetic_key("سداع");
        assert_eq!(key1, key2);

        // Q/K confusion
        let key3 = matcher.phonetic_key("قلب");
        let key4 = matcher.phonetic_key("كلب");
        assert_eq!(key3, key4);
    }

    #[test]
    fn test_arabic_phonetic_matcher_similarity() {
        let matcher = ArabicPhoneticMatcher::new();

        // Identical strings should have similarity 1.0
        let sim = matcher.phonetic_similarity("اوجمنتين", "اوجمنتين");
        assert!((sim - 1.0).abs() < 0.001);

        // Phonetically equivalent strings should have high similarity
        let sim = matcher.phonetic_similarity("كتافلام", "قتافلام");
        assert!(sim > 0.9, "Expected high similarity, got {}", sim);

        // Different strings should have lower similarity
        let sim = matcher.phonetic_similarity("اوجمنتين", "بانادول");
        assert!(sim < 0.6, "Expected low similarity, got {}", sim);
    }

    #[test]
    fn test_arabic_phonetic_matcher_is_phonetic_match() {
        let matcher = ArabicPhoneticMatcher::new();

        // Phonetically equivalent should match
        assert!(matcher.is_phonetic_match("كتافلام", "قتافلام"));

        // Same medication with diacritics should match
        assert!(matcher.is_phonetic_match("أُوجْمَنْتِين", "اوجمنتين"));

        // Different medications should not match
        assert!(!matcher.is_phonetic_match("اوجمنتين", "بانادول"));
    }

    #[test]
    fn test_arabic_phonetic_matcher_get_canonical() {
        let matcher = ArabicPhoneticMatcher::new();

        // Known variations should return canonical form
        assert_eq!(matcher.get_canonical("اوغمنتين"), Some("اوجمنتين"));
        assert_eq!(matcher.get_canonical("باندول"), Some("بانادول"));

        // Unknown medications should return None
        assert_eq!(matcher.get_canonical("دواء غير معروف"), None);
    }

    #[test]
    fn test_arabic_phonetic_matcher_is_arabic() {
        let matcher = ArabicPhoneticMatcher::new();

        assert!(matcher.is_arabic("اوجمنتين"));
        assert!(matcher.is_arabic("Augmentin اوجمنتين"));
        assert!(!matcher.is_arabic("Augmentin"));
    }

    #[test]
    fn test_arabic_phonetic_matcher_custom_variation() {
        let mut matcher = ArabicPhoneticMatcher::new();

        // Add custom variation
        matcher.add_variation("زيرتك", "زيرتيك");

        // Should now recognize the variation
        assert_eq!(matcher.get_canonical("زيرتك"), Some("زيرتيك"));
    }

    #[test]
    fn test_arabic_phonetic_matcher_custom_phonetic_group() {
        let mut matcher = ArabicPhoneticMatcher::new();

        // Add custom phonetic group (e.g., ث -> ت for some dialects)
        matcher.add_phonetic_group('ث', 'ت');

        // Now ث and ت should produce same phonetic key
        let key1 = matcher.phonetic_key("ثلاثة");
        let key2 = matcher.phonetic_key("تلاتة");
        assert_eq!(key1, key2);
    }
}
