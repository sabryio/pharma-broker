//! Arabic text normalization utilities
//!
//! Ported from legacy/pkg/arabic/normalizer.go

// Use a regex to strip trailing dosage info (numbers + units)
// Supports both English and Arabic units/numerals
// Matches patterns like " 500mg", " 0.5 g", " ١٠٠ملغ", etc. at the end of the string
lazy_static::lazy_static! {
    static ref DOSAGE_STRIP: regex::Regex = regex::Regex::new(
        r"(?i)\s+[\d٠-٩.,/]+\s*(ميكروغرام|مايكروجرام|جرام|غرام|ملغ|ملجم|وحدة|mcg|μg|ug|mg|ml|iu|ui|مل|ج|g|vial|tablet|ampoule)?\s*$"
    ).unwrap();
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

/// Normalize text for medication matching
/// Applies Arabic normalization, cleanups units, and lowercases
pub fn normalize_for_matching(text: &str) -> String {
    let mut normalized = normalize_arabic(text).to_lowercase();

    normalized = DOSAGE_STRIP.replace_all(&normalized, "").to_string();

    // Collapse multiple spaces
    normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    normalized.trim().to_string()
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
        assert_eq!(normalize_for_matching("Brufen 400mg"), "brufen 400mg");
    }
}
