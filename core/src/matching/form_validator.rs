//! Pharmaceutical form validator for matching
//!
//! Validates pharmaceutical form compatibility between offers and requests.
//! Handles Arabic and English form names with configurable compatibility rules.

use std::collections::{HashMap, HashSet};

use super::arabic::normalize_arabic;

lazy_static::lazy_static! {
    /// Known pharmaceutical forms with their variants
    /// Format: (canonical_form, [variants...])
    static ref KNOWN_FORMS: Vec<(&'static str, Vec<&'static str>)> = vec![
        ("امبول", vec!["امبول", "أمبول", "ampoule", "amp"]),
        ("فايل", vec!["فايل", "vial"]),
        ("اقراص", vec!["اقراص", "أقراص", "قرص", "tablets", "tab", "tablet"]),
        ("كبسولات", vec!["كبسولات", "كبسولة", "capsules", "caps", "capsule"]),
        ("شراب", vec!["شراب", "syrup", "liquid", "سيرب"]),
        ("نقط", vec!["نقط", "نقطة", "drops", "drop"]),
        ("لاصقه", vec!["لاصقه", "لاصقة", "patch"]),
        ("حقن", vec!["حقن", "حقنة", "injection", "inj"]),
        ("لبوس", vec!["لبوس", "suppository", "supp"]),
        ("جل", vec!["جل", "gel"]),
        ("كريم", vec!["كريم", "cream"]),
        ("مرهم", vec!["مرهم", "ointment"]),
        ("بخاخ", vec!["بخاخ", "spray", "inhaler"]),
        ("طقم", vec!["طقم", "kit"]),
    ];

    /// Default form compatibility rules
    /// Format: (form_a, form_b, penalty)
    /// penalty: 0.0 = fully compatible, 1.0 = fully incompatible
    static ref DEFAULT_COMPATIBILITY_RULES: Vec<(&'static str, &'static str, f64)> = vec![
        // Compatible pairs (low penalty)
        ("امبول", "فايل", 0.1),  // Ampoule and vial are similar
        ("اقراص", "كبسولات", 0.2),  // Tablets and capsules are somewhat similar

        // Incompatible pairs (high penalty)
        ("امبول", "اقراص", 0.8),  // Injectable vs oral
        ("امبول", "شراب", 0.8),  // Injectable vs liquid oral
        ("امبول", "كبسولات", 0.8),  // Injectable vs oral capsules
        ("فايل", "اقراص", 0.8),  // Injectable vs oral
        ("فايل", "شراب", 0.8),  // Injectable vs liquid oral
        ("اقراص", "شراب", 0.7),  // Solid vs liquid oral (less severe)
        ("كبسولات", "شراب", 0.7),  // Solid vs liquid oral
        ("حقن", "اقراص", 0.8),  // Injectable vs oral
        ("حقن", "شراب", 0.8),  // Injectable vs liquid oral
        ("لبوس", "اقراص", 0.7),  // Rectal vs oral
        ("لبوس", "شراب", 0.7),  // Rectal vs liquid oral
        ("لاصقه", "اقراص", 0.7),  // Transdermal vs oral
        ("كريم", "اقراص", 0.8),  // Topical vs oral
        ("مرهم", "اقراص", 0.8),  // Topical vs oral
        ("جل", "اقراص", 0.8),  // Topical vs oral
    ];
}

/// Form compatibility rule
#[derive(Debug, Clone, PartialEq)]
pub struct FormCompatibilityRule {
    pub form_a: String,
    pub form_b: String,
    pub compatible: bool,
    pub penalty: f64,
}

impl FormCompatibilityRule {
    /// Create a new compatibility rule
    pub fn new(form_a: String, form_b: String, penalty: f64) -> Self {
        Self {
            form_a,
            form_b,
            compatible: penalty < 0.5, // Compatible if penalty < 50%
            penalty,
        }
    }
}

/// Form compatibility result
#[derive(Debug, Clone, PartialEq)]
pub struct FormCompatibility {
    pub compatible: bool,
    pub penalty: f64,
    pub reason: String,
}

impl FormCompatibility {
    /// Create a compatible result
    pub fn compatible(penalty: f64, reason: String) -> Self {
        Self {
            compatible: true,
            penalty,
            reason,
        }
    }

    /// Create an incompatible result
    pub fn incompatible(penalty: f64, reason: String) -> Self {
        Self {
            compatible: false,
            penalty,
            reason,
        }
    }
}

/// Form validator for pharmaceutical matching
#[derive(Debug, Clone)]
pub struct FormValidator {
    /// Compatibility rules (form_a, form_b) -> penalty
    rules: HashMap<(String, String), f64>,
    /// Known forms mapping (variant -> canonical)
    known_forms: HashMap<String, String>,
}

impl Default for FormValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl FormValidator {
    /// Create a new FormValidator with default rules
    pub fn new() -> Self {
        let mut known_forms = HashMap::new();

        // Build known forms mapping
        for (canonical, variants) in KNOWN_FORMS.iter() {
            for variant in variants {
                known_forms.insert(variant.to_lowercase(), canonical.to_string());
            }
        }

        // Build compatibility rules
        let mut rules = HashMap::new();
        for (form_a, form_b, penalty) in DEFAULT_COMPATIBILITY_RULES.iter() {
            let key1 = (form_a.to_string(), form_b.to_string());
            let key2 = (form_b.to_string(), form_a.to_string());
            rules.insert(key1, *penalty);
            rules.insert(key2, *penalty);
        }

        Self { rules, known_forms }
    }

    /// Create a new FormValidator with custom rules
    pub fn with_rules(rules: Vec<FormCompatibilityRule>) -> Self {
        let mut validator = Self::new();

        for rule in rules {
            let key1 = (rule.form_a.clone(), rule.form_b.clone());
            let key2 = (rule.form_b, rule.form_a);
            validator.rules.insert(key1, rule.penalty);
            validator.rules.insert(key2, rule.penalty);
        }

        validator
    }

    /// Normalize form text (handle Arabic/English, plurals, etc.)
    ///
    /// Returns the canonical form if recognized, otherwise returns normalized input
    pub fn normalize_form(&self, form: &str) -> Option<String> {
        if form.is_empty() {
            return None;
        }

        // Normalize Arabic text
        let normalized = normalize_arabic(form).to_lowercase().trim().to_string();

        // Check if empty after normalization
        if normalized.is_empty() {
            return None;
        }

        // Look up in known forms
        if let Some(canonical) = self.known_forms.get(&normalized) {
            return Some(canonical.clone());
        }

        // Return normalized form even if not in known list
        Some(normalized)
    }

    /// Check if two forms are compatible
    pub fn are_compatible(&self, form_a: &str, form_b: &str) -> FormCompatibility {
        // Normalize both forms
        let norm_a = match self.normalize_form(form_a) {
            Some(f) => f,
            None => {
                return FormCompatibility::compatible(0.0, "Form A is empty".to_string());
            }
        };

        let norm_b = match self.normalize_form(form_b) {
            Some(f) => f,
            None => {
                return FormCompatibility::compatible(0.0, "Form B is empty".to_string());
            }
        };

        // If identical, fully compatible
        if norm_a == norm_b {
            return FormCompatibility::compatible(0.0, format!("Identical forms: {}", norm_a));
        }

        // Look up compatibility rule
        let key = (norm_a.clone(), norm_b.clone());
        if let Some(&penalty) = self.rules.get(&key) {
            if penalty >= 0.5 {
                return FormCompatibility::incompatible(
                    penalty,
                    format!("Incompatible forms: {} vs {}", norm_a, norm_b),
                );
            } else {
                return FormCompatibility::compatible(
                    penalty,
                    format!("Compatible forms with penalty: {} vs {}", norm_a, norm_b),
                );
            }
        }

        // No rule found - assume moderate incompatibility for unknown forms
        FormCompatibility::compatible(
            0.3,
            format!("Unknown form compatibility: {} vs {}", norm_a, norm_b),
        )
    }

    /// Get penalty for form mismatch
    pub fn get_penalty(&self, form_a: &str, form_b: &str) -> f64 {
        self.are_compatible(form_a, form_b).penalty
    }

    /// Add a custom compatibility rule
    pub fn add_rule(&mut self, form_a: &str, form_b: &str, penalty: f64) {
        let norm_a = self
            .normalize_form(form_a)
            .unwrap_or_else(|| form_a.to_string());
        let norm_b = self
            .normalize_form(form_b)
            .unwrap_or_else(|| form_b.to_string());

        let key1 = (norm_a.clone(), norm_b.clone());
        let key2 = (norm_b, norm_a);

        self.rules.insert(key1, penalty);
        self.rules.insert(key2, penalty);
    }

    /// Get all known forms
    pub fn get_known_forms(&self) -> HashSet<String> {
        self.known_forms.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_form_arabic() {
        let validator = FormValidator::new();

        // Standard forms
        assert_eq!(validator.normalize_form("امبول"), Some("امبول".to_string()));
        assert_eq!(validator.normalize_form("أمبول"), Some("امبول".to_string()));
        assert_eq!(validator.normalize_form("اقراص"), Some("اقراص".to_string()));
        assert_eq!(validator.normalize_form("أقراص"), Some("اقراص".to_string()));
    }

    #[test]
    fn test_normalize_form_english() {
        let validator = FormValidator::new();

        // English forms
        assert_eq!(
            validator.normalize_form("ampoule"),
            Some("امبول".to_string())
        );
        assert_eq!(
            validator.normalize_form("tablets"),
            Some("اقراص".to_string())
        );
        assert_eq!(
            validator.normalize_form("capsules"),
            Some("كبسولات".to_string())
        );
    }

    #[test]
    fn test_normalize_form_case_insensitive() {
        let validator = FormValidator::new();

        // Case insensitive
        assert_eq!(
            validator.normalize_form("AMPOULE"),
            Some("امبول".to_string())
        );
        assert_eq!(
            validator.normalize_form("Tablets"),
            Some("اقراص".to_string())
        );
    }

    #[test]
    fn test_normalize_form_unknown() {
        let validator = FormValidator::new();

        // Unknown form - returns normalized input
        let result = validator.normalize_form("unknown_form");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "unknown_form");
    }

    #[test]
    fn test_normalize_form_empty() {
        let validator = FormValidator::new();

        // Empty form
        assert_eq!(validator.normalize_form(""), None);
        assert_eq!(validator.normalize_form("   "), None);
    }

    #[test]
    fn test_are_compatible_identical() {
        let validator = FormValidator::new();

        // Identical forms
        let result = validator.are_compatible("امبول", "امبول");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.0);
    }

    #[test]
    fn test_are_compatible_variants() {
        let validator = FormValidator::new();

        // Variants of same form
        let result = validator.are_compatible("امبول", "أمبول");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.0);

        let result = validator.are_compatible("امبول", "ampoule");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.0);
    }

    #[test]
    fn test_are_compatible_low_penalty() {
        let validator = FormValidator::new();

        // امبول and فايل are compatible (low penalty)
        let result = validator.are_compatible("امبول", "فايل");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.1);
    }

    #[test]
    fn test_are_compatible_high_penalty() {
        let validator = FormValidator::new();

        // امبول and اقراص are incompatible (high penalty)
        let result = validator.are_compatible("امبول", "اقراص");
        assert!(!result.compatible);
        assert_eq!(result.penalty, 0.8);

        // امبول and شراب are incompatible
        let result = validator.are_compatible("امبول", "شراب");
        assert!(!result.compatible);
        assert_eq!(result.penalty, 0.8);
    }

    #[test]
    fn test_are_compatible_moderate_penalty() {
        let validator = FormValidator::new();

        // اقراص and شراب are somewhat incompatible (moderate penalty)
        let result = validator.are_compatible("اقراص", "شراب");
        assert!(!result.compatible);
        assert_eq!(result.penalty, 0.7);
    }

    #[test]
    fn test_are_compatible_unknown_forms() {
        let validator = FormValidator::new();

        // Unknown forms - moderate penalty
        let result = validator.are_compatible("unknown1", "unknown2");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.3);
    }

    #[test]
    fn test_get_penalty() {
        let validator = FormValidator::new();

        // Test penalty getter
        assert_eq!(validator.get_penalty("امبول", "امبول"), 0.0);
        assert_eq!(validator.get_penalty("امبول", "فايل"), 0.1);
        assert_eq!(validator.get_penalty("امبول", "اقراص"), 0.8);
    }

    #[test]
    fn test_add_custom_rule() {
        let mut validator = FormValidator::new();

        // Add custom rule
        validator.add_rule("امبول", "اقراص", 0.5);

        // Should use new rule
        let result = validator.are_compatible("امبول", "اقراص");
        assert!(!result.compatible); // 0.5 is at the boundary
        assert_eq!(result.penalty, 0.5);
    }

    #[test]
    fn test_with_custom_rules() {
        let rules = vec![FormCompatibilityRule::new(
            "امبول".to_string(),
            "اقراص".to_string(),
            0.2,
        )];

        let validator = FormValidator::with_rules(rules);

        // Should use custom rule
        let result = validator.are_compatible("امبول", "اقراص");
        assert!(result.compatible);
        assert_eq!(result.penalty, 0.2);
    }

    #[test]
    fn test_get_known_forms() {
        let validator = FormValidator::new();

        let known = validator.get_known_forms();

        // Should contain canonical forms
        assert!(known.contains("امبول"));
        assert!(known.contains("اقراص"));
        assert!(known.contains("كبسولات"));
        assert!(known.contains("شراب"));
    }

    #[test]
    fn test_symmetry() {
        let validator = FormValidator::new();

        // Compatibility should be symmetric
        let result1 = validator.are_compatible("امبول", "فايل");
        let result2 = validator.are_compatible("فايل", "امبول");

        assert_eq!(result1.penalty, result2.penalty);
        assert_eq!(result1.compatible, result2.compatible);
    }

    #[test]
    fn test_real_world_examples() {
        let validator = FormValidator::new();

        // Real examples from database
        // كالسيوم كلورايد امبول vs كالسيوم شراب
        let result = validator.are_compatible("امبول", "شراب");
        assert!(!result.compatible);
        assert!(result.penalty >= 0.7);

        // كالسيوم كلورايد امبول vs كالسيوم اقراص
        let result = validator.are_compatible("امبول", "اقراص");
        assert!(!result.compatible);
        assert!(result.penalty >= 0.7);
    }
}
