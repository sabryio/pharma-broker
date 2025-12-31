//! Dosage parsing and comparison module
//!
//! Ported from legacy/pkg/dosage/dosage.go
//! Enhanced with medication-specific IU conversions

use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

/// Unit conversion constants (all to mg as base unit)
const MG_PER_G: f64 = 1000.0; // 1 gram = 1000 milligrams
const MCG_PER_MG: f64 = 1000.0; // 1 milligram = 1000 micrograms

lazy_static! {
    /// Arabic numeral to Western numeral mapping
    static ref ARABIC_NUMERALS: HashMap<char, char> = {
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

    /// Dosage pattern regex
    /// Matches patterns like: "100mg", "0.5g", "500 mcg", "2mg/ml", etc.
    /// Also matches Arabic numerals and unit names
    static ref DOSAGE_PATTERN: Regex = Regex::new(
        r"(?i)([\d٠-٩]+(?:[.,]?[\d٠-٩]+)?)\s*(ميكروغرام|مايكروجرام|جرام|غرام|ملغ|ملجم|وحدة|mcg|μg|ug|mg|ml|iu|ui|مل|ج|g)(?:/\w+)?"
    ).unwrap();

    /// Medication-specific IU to mg conversion factors
    /// IU (International Units) vary by medication type
    static ref IU_CONVERSIONS: HashMap<&'static str, f64> = {
        let mut m = HashMap::new();
        // Insulin: approximately 27.5 IU per mg (varies by type)
        m.insert("insulin", 0.0364); // 1 IU = 0.0364 mg (1/27.5)
        m.insert("lantus", 0.0364);
        m.insert("humalog", 0.0364);
        m.insert("novolog", 0.0364);
        m.insert("novorapid", 0.0364);
        m.insert("levemir", 0.0364);
        m.insert("tresiba", 0.0364);
        m.insert("toujeo", 0.0364);
        m.insert("fiasp", 0.0364);
        m.insert("apidra", 0.0364);

        // Vitamin D: 40 IU = 1 mcg = 0.001 mg
        m.insert("vitamin d", 0.000025); // 1 IU = 0.025 mcg
        m.insert("d3", 0.000025);
        m.insert("cholecalciferol", 0.000025);
        m.insert("ergocalciferol", 0.000025);

        // Vitamin E: 1 IU = 0.67 mg (d-alpha-tocopherol) or 0.45 mg (dl-alpha)
        m.insert("vitamin e", 0.67);
        m.insert("tocopherol", 0.67);

        // Vitamin A: 1 IU = 0.3 mcg retinol = 0.0003 mg
        m.insert("vitamin a", 0.0003);
        m.insert("retinol", 0.0003);

        // Heparin: approximately 100-200 IU per mg (varies)
        m.insert("heparin", 0.007); // ~1 IU = 0.007 mg (average)
        m.insert("لوفنوكس", 0.01); // Lovenox
        m.insert("enoxaparin", 0.01);
        m.insert("clexane", 0.01);

        // Penicillin: 1 mg = 1667 IU for penicillin G
        m.insert("penicillin", 0.0006); // 1 IU = 0.0006 mg

        // EPO (Erythropoietin): varies significantly
        m.insert("epo", 0.0084);
        m.insert("erythropoietin", 0.0084);
        m.insert("epoetin", 0.0084);

        m
    };
}

/// Represents a medication dosage with value and unit
#[derive(Debug, Clone, PartialEq)]
pub struct Dosage {
    pub value: f64,
    pub unit: String,
}

impl Dosage {
    /// Create a new dosage
    pub fn new(value: f64, unit: &str) -> Self {
        Self {
            value,
            unit: unit.to_string(),
        }
    }

    /// Convert dosage to base unit (mg) without medication context
    pub fn to_base_unit(&self) -> f64 {
        self.to_base_unit_with_medication(None)
    }

    /// Convert dosage to base unit (mg) with medication context for IU conversion
    pub fn to_base_unit_with_medication(&self, medication_name: Option<&str>) -> f64 {
        match self.unit.as_str() {
            "g" => self.value * MG_PER_G,
            "mcg" => self.value / MCG_PER_MG,
            "mg" => self.value,
            "ml" => self.value, // ml to mg is density-dependent, keep as-is
            "iu" => {
                // Use medication-specific conversion if available
                if let Some(med_name) = medication_name {
                    let med_lower = med_name.to_lowercase();
                    for (key, factor) in IU_CONVERSIONS.iter() {
                        if med_lower.contains(key) {
                            return self.value * factor;
                        }
                    }
                }
                // Default: 1 IU = 1 arbitrary unit (compare IU to IU directly)
                self.value
            }
            _ => self.value, // Unknown unit, return as-is
        }
    }
}

impl std::fmt::Display for Dosage {
    /// Human-readable string
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.value == self.value.floor() {
            write!(f, "{}{}", self.value as i64, self.unit)
        } else {
            write!(f, "{}{}", self.value, self.unit)
        }
    }
}

/// Convert Arabic numerals (٠-٩) to Western (0-9)
fn convert_arabic_numerals(s: &str) -> String {
    s.chars()
        .map(|c| *ARABIC_NUMERALS.get(&c).unwrap_or(&c))
        .collect()
}

/// Normalize unit representation (supports Arabic and English)
fn normalize_unit(unit: &str) -> String {
    let unit = unit.to_lowercase().trim().to_string();

    match unit.as_str() {
        // English variations
        "μg" | "ug" => "mcg".to_string(),
        "ui" => "iu".to_string(),

        // Arabic unit names
        "ملغ" | "ملجم" => "mg".to_string(),       // milligram
        "جرام" | "غرام" | "ج" => "g".to_string(), // gram
        "ميكروغرام" | "مايكروجرام" => "mcg".to_string(), // microgram
        "مل" => "ml".to_string(),                 // milliliter
        "وحدة" => "iu".to_string(),               // international unit

        _ => unit,
    }
}

/// Parse dosage from a medication string
/// Supports both Western and Arabic numerals and unit names
/// Returns None if no dosage found
pub fn parse_dosage(medication: &str) -> Option<Dosage> {
    if medication.is_empty() {
        return None;
    }

    let captures = DOSAGE_PATTERN.captures(medication)?;

    let value_str = convert_arabic_numerals(captures.get(1)?.as_str());
    let value: f64 = value_str.replace(',', ".").parse().ok()?;

    let unit = normalize_unit(captures.get(2)?.as_str());

    Some(Dosage::new(value, &unit))
}

/// Compare dosages and return similarity score (0.0-1.0)
/// Returns 1.0 for exact match, decreasing as difference increases
pub fn compare_dosages(a: &Option<Dosage>, b: &Option<Dosage>) -> f64 {
    match (a, b) {
        (None, None) => 0.0,
        (None, Some(_)) | (Some(_), None) => 0.0,
        (Some(a), Some(b)) => {
            let a_base = a.to_base_unit();
            let b_base = b.to_base_unit();

            // Handle zero values
            if a_base == 0.0 && b_base == 0.0 {
                return 1.0;
            }
            if a_base == 0.0 || b_base == 0.0 {
                return 0.0;
            }

            // Calculate percentage difference
            let diff = (a_base - b_base).abs();
            let avg = (a_base + b_base) / 2.0;
            let percent_diff = diff / avg;

            // 10% tolerance for "perfect" match
            if percent_diff <= 0.1 {
                return 1.0;
            }

            // Linear decay: 50% diff = 0.0 score
            (1.0 - percent_diff * 2.0).max(0.0)
        }
    }
}

/// Check if dosages are equivalent (within 10% tolerance)
pub fn is_same_dosage(a: &Option<Dosage>, b: &Option<Dosage>) -> bool {
    compare_dosages(a, b) >= 0.9
}

// ============================================================================
// Tests - Ported from Go dosage_test.go
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // TestParseDosage - Basic patterns (Go lines 17-21)
    #[rstest]
    #[case("100mg", 100.0, "mg")]
    #[case("0.5g", 0.5, "g")]
    #[case("500mcg", 500.0, "mcg")]
    #[case("10ml", 10.0, "ml")]
    #[case("50iu", 50.0, "iu")]
    fn test_parse_dosage_basic(#[case] input: &str, #[case] value: f64, #[case] unit: &str) {
        let result = parse_dosage(input).expect("should parse");
        assert!((result.value - value).abs() < 0.001, "value mismatch");
        assert_eq!(result.unit, unit);
    }

    // TestParseDosage - With spaces (Go lines 24-25)
    #[rstest]
    #[case("100 mg", 100.0, "mg")]
    #[case("0.5 g", 0.5, "g")]
    fn test_parse_dosage_spaces(#[case] input: &str, #[case] value: f64, #[case] unit: &str) {
        let result = parse_dosage(input).expect("should parse");
        assert!((result.value - value).abs() < 0.001);
        assert_eq!(result.unit, unit);
    }

    // TestParseDosage - In medication names (Go lines 32-34)
    #[rstest]
    #[case("Ozempic 2mg", 2.0, "mg")]
    #[case("Concor 5mg", 5.0, "mg")]
    #[case("Augmentin 1g", 1.0, "g")]
    fn test_parse_dosage_in_name(#[case] input: &str, #[case] value: f64, #[case] unit: &str) {
        let result = parse_dosage(input).expect("should parse");
        assert!((result.value - value).abs() < 0.001);
        assert_eq!(result.unit, unit);
    }

    // TestParseDosage - Unit variations (Go lines 37-39)
    #[rstest]
    #[case("500μg", 500.0, "mcg")]
    #[case("500ug", 500.0, "mcg")]
    #[case("100ui", 100.0, "iu")]
    fn test_parse_dosage_unit_variants(
        #[case] input: &str,
        #[case] value: f64,
        #[case] unit: &str,
    ) {
        let result = parse_dosage(input).expect("should parse");
        assert!((result.value - value).abs() < 0.001);
        assert_eq!(result.unit, unit);
    }

    // TestParseDosage - Arabic numerals (Go lines 50-52)
    #[rstest]
    #[case("١٠٠ملغ", 100.0, "mg")]
    #[case("٥٠٠جرام", 500.0, "g")]
    fn test_parse_dosage_arabic(#[case] input: &str, #[case] value: f64, #[case] unit: &str) {
        let result = parse_dosage(input).expect("should parse");
        assert!((result.value - value).abs() < 0.001);
        assert_eq!(result.unit, unit);
    }

    // TestParseDosage - Edge cases (Go lines 67-69)
    #[rstest]
    #[case("Ozempic")]
    #[case("")]
    #[case("medication name")]
    fn test_parse_dosage_no_dosage(#[case] input: &str) {
        assert!(parse_dosage(input).is_none());
    }

    // TestDosage_ToBaseUnit (Go lines 98-121)
    #[rstest]
    #[case(100.0, "mg", 100.0)]
    #[case(1.0, "g", 1000.0)]
    #[case(0.5, "g", 500.0)]
    #[case(500.0, "mcg", 0.5)]
    #[case(1000.0, "mcg", 1.0)]
    #[case(10.0, "ml", 10.0)]
    #[case(50.0, "iu", 50.0)]
    fn test_to_base_unit(#[case] value: f64, #[case] unit: &str, #[case] expected: f64) {
        let d = Dosage::new(value, unit);
        let result = d.to_base_unit();
        assert!(
            (result - expected).abs() < 0.001,
            "got {} expected {}",
            result,
            expected
        );
    }

    // TestCompareDosages - Exact matches (Go lines 133-134)
    #[rstest]
    #[case(100.0, "mg", 100.0, "mg", 1.0)]
    #[case(0.5, "g", 0.5, "g", 1.0)]
    fn test_compare_dosages_exact(
        #[case] v1: f64,
        #[case] u1: &str,
        #[case] v2: f64,
        #[case] u2: &str,
        #[case] expected: f64,
    ) {
        let a = Some(Dosage::new(v1, u1));
        let b = Some(Dosage::new(v2, u2));
        assert_eq!(compare_dosages(&a, &b), expected);
    }

    // TestCompareDosages - Unit conversion (Go lines 137-139)
    #[rstest]
    #[case(1.0, "g", 1000.0, "mg", 1.0)]
    #[case(500.0, "mcg", 0.5, "mg", 1.0)]
    #[case(0.5, "g", 500.0, "mg", 1.0)]
    fn test_compare_dosages_conversion(
        #[case] v1: f64,
        #[case] u1: &str,
        #[case] v2: f64,
        #[case] u2: &str,
        #[case] expected: f64,
    ) {
        let a = Some(Dosage::new(v1, u1));
        let b = Some(Dosage::new(v2, u2));
        assert_eq!(compare_dosages(&a, &b), expected);
    }

    // TestCompareDosages - Tolerance (Go lines 142-143)
    #[rstest]
    #[case(105.0, "mg", 100.0, "mg", 1.0)]
    #[case(95.0, "mg", 100.0, "mg", 1.0)]
    fn test_compare_dosages_tolerance(
        #[case] v1: f64,
        #[case] u1: &str,
        #[case] v2: f64,
        #[case] u2: &str,
        #[case] expected: f64,
    ) {
        let a = Some(Dosage::new(v1, u1));
        let b = Some(Dosage::new(v2, u2));
        assert_eq!(compare_dosages(&a, &b), expected);
    }

    // TestCompareDosages - nil cases (Go lines 155-157)
    #[test]
    fn test_compare_dosages_none() {
        let valid = Some(Dosage::new(100.0, "mg"));
        assert_eq!(compare_dosages(&None, &valid), 0.0);
        assert_eq!(compare_dosages(&valid, &None), 0.0);
        assert_eq!(compare_dosages(&None, &None), 0.0);
    }

    // TestConvertArabicNumerals (Go lines 267-292)
    #[rstest]
    #[case("123.45", "123.45")]
    #[case("٠", "0")]
    #[case("٥", "5")]
    #[case("١٠", "10")]
    #[case("١٠٠", "100")]
    #[case("٢.٥", "2.5")]
    #[case("٠١٢٣٤٥٦٧٨٩", "0123456789")]
    #[case("", "")]
    fn test_convert_arabic_numerals(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(convert_arabic_numerals(input), expected);
    }

    // TestNormalizeUnit (Go lines 228-264)
    #[rstest]
    #[case("mcg", "mcg")]
    #[case("μg", "mcg")]
    #[case("ug", "mcg")]
    #[case("mg", "mg")]
    #[case("g", "g")]
    #[case("ui", "iu")]
    #[case("iu", "iu")]
    #[case("ml", "ml")]
    #[case("ملغ", "mg")]
    #[case("ملجم", "mg")]
    #[case("جرام", "g")]
    #[case("غرام", "g")]
    #[case("ج", "g")]
    #[case("مل", "ml")]
    #[case("ميكروغرام", "mcg")]
    #[case("وحدة", "iu")]
    fn test_normalize_unit(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_unit(input), expected);
    }

    // TestIsSameDosage (Go lines 181-203)
    #[rstest]
    #[case(100.0, "mg", 100.0, "mg", true)]
    #[case(105.0, "mg", 100.0, "mg", true)]
    #[case(1.0, "g", 1000.0, "mg", true)]
    #[case(120.0, "mg", 100.0, "mg", false)]
    #[case(1000.0, "mg", 100.0, "mg", false)]
    fn test_is_same_dosage(
        #[case] v1: f64,
        #[case] u1: &str,
        #[case] v2: f64,
        #[case] u2: &str,
        #[case] expected: bool,
    ) {
        let a = Some(Dosage::new(v1, u1));
        let b = Some(Dosage::new(v2, u2));
        assert_eq!(is_same_dosage(&a, &b), expected);
    }
}
