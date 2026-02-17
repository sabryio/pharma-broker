//! Concentration parser for pharmaceutical matching
//!
//! Extracts and normalizes medication concentration values from text.
//! Handles Arabic numerals, fractional text, and various unit formats.

use std::collections::HashMap;

lazy_static::lazy_static! {
    /// Regex for extracting Western numerals with optional decimal point
    static ref WESTERN_NUMERIC: regex::Regex = regex::Regex::new(
        r"\d+\.?\d*"
    ).unwrap();

    /// Regex for extracting Arabic numerals with optional decimal point
    static ref ARABIC_NUMERIC: regex::Regex = regex::Regex::new(
        r"[٠-٩]+\.?[٠-٩]*"
    ).unwrap();

    /// Regex for extracting units
    static ref UNIT_PATTERN: regex::Regex = regex::Regex::new(
        r"(?i)(mg|mcg|μg|ug|g|ml|iu|ui|ميكروغرام|مايكروجرام|جرام|غرام|ملغ|ملجم|وحدة|مل|ج)"
    ).unwrap();

    /// Arabic numeral to Western numeral mapping
    static ref ARABIC_TO_WESTERN: HashMap<char, char> = {
        let mut m = HashMap::new();
        m.insert('٠', '0');
        m.insert('١', '1');
        m.insert('٢', '2');
        m.insert('٣', '3');
        m.insert('٤', '4');
        m.insert('٥', '5');
        m.insert('٦', '6');
        m.insert('٧', '7');
        m.insert('٨', '8');
        m.insert('٩', '9');
        m
    };

    /// Arabic fractional text to numeric mapping
    static ref ARABIC_FRACTIONS: HashMap<&'static str, f64> = {
        let mut m = HashMap::new();
        m.insert("واحد ونص", 1.5);
        m.insert("واحد ونصف", 1.5);
        m.insert("نص", 0.5);
        m.insert("نصف", 0.5);
        m.insert("ربع", 0.25);
        m.insert("تلت", 0.33);
        m.insert("ثلث", 0.33);
        m.insert("اتنين", 2.0);
        m.insert("اثنين", 2.0);
        m.insert("تلاتة", 3.0);
        m.insert("ثلاثة", 3.0);
        m.insert("اربعة", 4.0);
        m.insert("أربعة", 4.0);
        m.insert("خمسة", 5.0);
        m
    };

    /// Unit normalization factors (to mg)
    static ref UNIT_TO_MG: HashMap<&'static str, f64> = {
        let mut m = HashMap::new();
        // Base unit
        m.insert("mg", 1.0);
        m.insert("ملغ", 1.0);
        m.insert("ملجم", 1.0);
        // Micrograms to mg
        m.insert("mcg", 0.001);
        m.insert("μg", 0.001);
        m.insert("ug", 0.001);
        m.insert("ميكروغرام", 0.001);
        m.insert("مايكروجرام", 0.001);
        // Grams to mg
        m.insert("g", 1000.0);
        m.insert("ج", 1000.0);
        m.insert("جرام", 1000.0);
        m.insert("غرام", 1000.0);
        // ML (volume - treat as 1:1 for simplicity)
        m.insert("ml", 1.0);
        m.insert("مل", 1.0);
        // IU (international units - no conversion, treat as separate)
        m.insert("iu", 1.0);
        m.insert("ui", 1.0);
        m.insert("وحدة", 1.0);
        m
    };
}

/// Parsed concentration value with numeric value, unit, and original text
#[derive(Debug, Clone, PartialEq)]
pub struct ConcentrationValue {
    /// Numeric value (normalized to mg if possible)
    pub numeric: f64,
    /// Unit (if extracted)
    pub unit: Option<String>,
    /// Original text
    pub original: String,
}

impl ConcentrationValue {
    /// Create a new ConcentrationValue
    pub fn new(numeric: f64, unit: Option<String>, original: String) -> Self {
        Self {
            numeric,
            unit,
            original,
        }
    }

    /// Get the normalized value in mg (if unit conversion is possible)
    pub fn normalized_mg(&self) -> Option<f64> {
        if let Some(ref unit) = self.unit {
            let unit_lower = unit.to_lowercase();
            if let Some(&factor) = UNIT_TO_MG.get(unit_lower.as_str()) {
                return Some(self.numeric * factor);
            }
        }
        // If no unit or unknown unit, return the numeric value as-is
        Some(self.numeric)
    }
}

/// Concentration parser for extracting and comparing medication concentrations
#[derive(Debug, Clone, Default)]
pub struct ConcentrationParser;

impl ConcentrationParser {
    /// Create a new ConcentrationParser
    pub fn new() -> Self {
        Self
    }

    /// Convert Arabic numerals to Western numerals
    fn arabic_to_western(text: &str) -> String {
        text.chars()
            .map(|c| ARABIC_TO_WESTERN.get(&c).copied().unwrap_or(c))
            .collect()
    }

    /// Try to parse Arabic fractional text
    fn parse_arabic_fraction(text: &str) -> Option<f64> {
        let normalized = text.trim().to_lowercase();
        ARABIC_FRACTIONS.get(normalized.as_str()).copied()
    }

    /// Extract unit from text
    fn extract_unit(text: &str) -> Option<String> {
        UNIT_PATTERN.find(text).map(|m| m.as_str().to_lowercase())
    }

    /// Parse concentration from text
    ///
    /// Handles:
    /// - Western numerals: "150", "150mg", "1.5"
    /// - Arabic numerals: "١٥٠", "٣٦"
    /// - Fractional text: "واحد ونص", "نص", "ربع"
    /// - Units: mg, mcg, g, ml, IU, etc.
    ///
    /// Returns None if no numeric value can be extracted
    pub fn parse(&self, text: &str) -> Option<ConcentrationValue> {
        if text.is_empty() {
            return None;
        }

        let text = text.trim();

        // Try to parse Arabic fractional text first
        if let Some(fraction) = Self::parse_arabic_fraction(text) {
            return Some(ConcentrationValue::new(fraction, None, text.to_string()));
        }

        // Convert Arabic numerals to Western
        let western_text = Self::arabic_to_western(text);

        // Extract numeric value
        let numeric = if let Some(mat) = WESTERN_NUMERIC.find(&western_text) {
            mat.as_str().parse::<f64>().ok()?
        } else {
            return None;
        };

        // Extract unit
        let unit = Self::extract_unit(&western_text);

        Some(ConcentrationValue::new(numeric, unit, text.to_string()))
    }

    /// Calculate percentage difference between two concentrations
    ///
    /// Returns the percentage difference as a positive value.
    /// Formula: |a - b| / min(a, b) * 100
    ///
    /// This formula shows how much larger the bigger value is compared to the smaller.
    /// For example: 150 vs 15 = (150-15)/15 * 100 = 900%
    ///
    /// Returns 0.0 if both values are identical.
    pub fn difference_percent(&self, a: &ConcentrationValue, b: &ConcentrationValue) -> f64 {
        let a_val = a.normalized_mg().unwrap_or(a.numeric);
        let b_val = b.normalized_mg().unwrap_or(b.numeric);

        if (a_val - b_val).abs() < 0.001 {
            return 0.0;
        }

        let min_val = a_val.min(b_val);
        if min_val == 0.0 {
            return 0.0;
        }

        ((a_val - b_val).abs() / min_val) * 100.0
    }

    /// Check if two concentrations are compatible within tolerance
    ///
    /// Returns true if the percentage difference is within the tolerance.
    pub fn are_compatible(
        &self,
        a: &ConcentrationValue,
        b: &ConcentrationValue,
        tolerance_percent: f64,
    ) -> bool {
        self.difference_percent(a, b) <= tolerance_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_western_numerals() {
        let parser = ConcentrationParser::new();

        // Simple number
        let result = parser.parse("150").unwrap();
        assert_eq!(result.numeric, 150.0);
        assert_eq!(result.unit, None);

        // Number with mg
        let result = parser.parse("150mg").unwrap();
        assert_eq!(result.numeric, 150.0);
        assert_eq!(result.unit, Some("mg".to_string()));

        // Decimal
        let result = parser.parse("1.5").unwrap();
        assert_eq!(result.numeric, 1.5);
        assert_eq!(result.unit, None);

        // Number with space and unit
        let result = parser.parse("500 mg").unwrap();
        assert_eq!(result.numeric, 500.0);
        assert_eq!(result.unit, Some("mg".to_string()));
    }

    #[test]
    fn test_parse_arabic_numerals() {
        let parser = ConcentrationParser::new();

        // Arabic numerals
        let result = parser.parse("١٥٠").unwrap();
        assert_eq!(result.numeric, 150.0);

        // Arabic numerals with unit
        let result = parser.parse("٣٦ملغ").unwrap();
        assert_eq!(result.numeric, 36.0);
        assert_eq!(result.unit, Some("ملغ".to_string()));
    }

    #[test]
    fn test_parse_arabic_fractions() {
        let parser = ConcentrationParser::new();

        // واحد ونص = 1.5
        let result = parser.parse("واحد ونص").unwrap();
        assert_eq!(result.numeric, 1.5);

        // نص = 0.5
        let result = parser.parse("نص").unwrap();
        assert_eq!(result.numeric, 0.5);

        // ربع = 0.25
        let result = parser.parse("ربع").unwrap();
        assert_eq!(result.numeric, 0.25);
    }

    #[test]
    fn test_parse_units() {
        let parser = ConcentrationParser::new();

        // mg
        let result = parser.parse("150mg").unwrap();
        assert_eq!(result.unit, Some("mg".to_string()));

        // mcg
        let result = parser.parse("500mcg").unwrap();
        assert_eq!(result.unit, Some("mcg".to_string()));

        // g
        let result = parser.parse("1g").unwrap();
        assert_eq!(result.unit, Some("g".to_string()));

        // Arabic unit
        let result = parser.parse("١٥٠ملغ").unwrap();
        assert_eq!(result.unit, Some("ملغ".to_string()));
    }

    #[test]
    fn test_parse_empty_or_invalid() {
        let parser = ConcentrationParser::new();

        // Empty string
        assert!(parser.parse("").is_none());

        // No numbers
        assert!(parser.parse("tablets").is_none());

        // Only unit
        assert!(parser.parse("mg").is_none());
    }

    #[test]
    fn test_normalized_mg() {
        let parser = ConcentrationParser::new();

        // mg to mg (no conversion)
        let result = parser.parse("150mg").unwrap();
        assert_eq!(result.normalized_mg(), Some(150.0));

        // mcg to mg
        let result = parser.parse("500mcg").unwrap();
        assert_eq!(result.normalized_mg(), Some(0.5));

        // g to mg
        let result = parser.parse("1g").unwrap();
        assert_eq!(result.normalized_mg(), Some(1000.0));

        // No unit (return as-is)
        let result = parser.parse("150").unwrap();
        assert_eq!(result.normalized_mg(), Some(150.0));
    }

    #[test]
    fn test_difference_percent_identical() {
        let parser = ConcentrationParser::new();

        let a = parser.parse("150mg").unwrap();
        let b = parser.parse("150mg").unwrap();

        assert_eq!(parser.difference_percent(&a, &b), 0.0);
    }

    #[test]
    fn test_difference_percent_different() {
        let parser = ConcentrationParser::new();

        // 150mg vs 15mg = 900% difference
        let a = parser.parse("150mg").unwrap();
        let b = parser.parse("15mg").unwrap();

        let diff = parser.difference_percent(&a, &b);
        assert!((diff - 900.0).abs() < 1.0, "Expected ~900%, got {}", diff);
    }

    #[test]
    fn test_difference_percent_with_unit_conversion() {
        let parser = ConcentrationParser::new();

        // 1g vs 500mg = 100% difference
        let a = parser.parse("1g").unwrap();
        let b = parser.parse("500mg").unwrap();

        let diff = parser.difference_percent(&a, &b);
        assert!((diff - 100.0).abs() < 1.0, "Expected ~100%, got {}", diff);
    }

    #[test]
    fn test_are_compatible_within_tolerance() {
        let parser = ConcentrationParser::new();

        let a = parser.parse("150mg").unwrap();
        let b = parser.parse("160mg").unwrap();

        // ~6.7% difference, should be compatible with 20% tolerance
        assert!(parser.are_compatible(&a, &b, 20.0));

        // Should not be compatible with 5% tolerance
        assert!(!parser.are_compatible(&a, &b, 5.0));
    }

    #[test]
    fn test_are_compatible_large_difference() {
        let parser = ConcentrationParser::new();

        let a = parser.parse("150mg").unwrap();
        let b = parser.parse("15mg").unwrap();

        // 900% difference, should not be compatible
        assert!(!parser.are_compatible(&a, &b, 20.0));
        assert!(!parser.are_compatible(&a, &b, 50.0));
    }

    #[test]
    fn test_arabic_to_western_conversion() {
        assert_eq!(ConcentrationParser::arabic_to_western("١٥٠"), "150");
        assert_eq!(ConcentrationParser::arabic_to_western("٣٦"), "36");
        assert_eq!(ConcentrationParser::arabic_to_western("١.٥"), "1.5");
    }

    #[test]
    fn test_parse_multiple_numbers_takes_first() {
        let parser = ConcentrationParser::new();

        // Should take the first number
        let result = parser.parse("150/300").unwrap();
        assert_eq!(result.numeric, 150.0);
    }

    #[test]
    fn test_real_world_examples() {
        let parser = ConcentrationParser::new();

        // جونابيور 150
        let result = parser.parse("150").unwrap();
        assert_eq!(result.numeric, 150.0);

        // اسيكلوفير 500mg
        let result = parser.parse("500").unwrap();
        assert_eq!(result.numeric, 500.0);

        // لنفيما 4
        let result = parser.parse("4").unwrap();
        assert_eq!(result.numeric, 4.0);

        // برولانت 75
        let result = parser.parse("75").unwrap();
        assert_eq!(result.numeric, 75.0);
    }

    #[test]
    fn test_concentration_value_equality() {
        let a = ConcentrationValue::new(150.0, Some("mg".to_string()), "150mg".to_string());
        let b = ConcentrationValue::new(150.0, Some("mg".to_string()), "150mg".to_string());
        assert_eq!(a, b);
    }
}
