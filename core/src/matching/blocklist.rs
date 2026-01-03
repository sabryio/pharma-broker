//! Medication Blocklist - Prevents matching of dangerous medication pairs
//!
//! Maintains a list of medication pairs that must never be matched together
//! due to potential confusion or dangerous interactions. This is a critical
//! safety component that returns a score of 0.0 for any blocked pair.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Severity level for blocklist entries
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlocklistSeverity {
    /// Potentially fatal confusion (e.g., Metformin/Metoprolol)
    Critical,
    /// Serious adverse effects possible
    #[default]
    High,
    /// Significant clinical difference
    Medium,
}

/// A blocklist entry representing a pair of medications that must not be matched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistEntry {
    /// First medication name (normalized)
    pub medication_a: String,
    /// Second medication name (normalized)
    pub medication_b: String,
    /// Reason for blocking this pair
    pub reason: String,
    /// Severity level of the potential confusion
    pub severity: BlocklistSeverity,
}

impl BlocklistEntry {
    /// Create a new blocklist entry
    pub fn new(
        medication_a: impl Into<String>,
        medication_b: impl Into<String>,
        reason: impl Into<String>,
        severity: BlocklistSeverity,
    ) -> Self {
        Self {
            medication_a: medication_a.into(),
            medication_b: medication_b.into(),
            reason: reason.into(),
            severity,
        }
    }

    /// Create a critical severity entry
    pub fn critical(
        medication_a: impl Into<String>,
        medication_b: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::new(
            medication_a,
            medication_b,
            reason,
            BlocklistSeverity::Critical,
        )
    }

    /// Create a high severity entry
    pub fn high(
        medication_a: impl Into<String>,
        medication_b: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::new(medication_a, medication_b, reason, BlocklistSeverity::High)
    }

    /// Create a medium severity entry
    pub fn medium(
        medication_a: impl Into<String>,
        medication_b: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::new(
            medication_a,
            medication_b,
            reason,
            BlocklistSeverity::Medium,
        )
    }
}

/// Error type for blocklist operations
#[derive(Debug)]
pub enum BlocklistError {
    /// IO error when loading/saving blocklist
    Io(io::Error),
    /// JSON parsing error
    Json(serde_json::Error),
}

impl std::fmt::Display for BlocklistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlocklistError::Io(e) => write!(f, "IO error: {}", e),
            BlocklistError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for BlocklistError {}

impl From<io::Error> for BlocklistError {
    fn from(e: io::Error) -> Self {
        BlocklistError::Io(e)
    }
}

impl From<serde_json::Error> for BlocklistError {
    fn from(e: serde_json::Error) -> Self {
        BlocklistError::Json(e)
    }
}

/// Medication blocklist for preventing dangerous medication pair matches
#[derive(Debug, Clone)]
pub struct MedicationBlocklist {
    /// Storage for blocklist entries, keyed by normalized medication pair
    entries: HashMap<String, BlocklistEntry>,
}

impl Default for MedicationBlocklist {
    fn default() -> Self {
        Self::new()
    }
}

impl MedicationBlocklist {
    /// Create a new empty blocklist
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Create a blocklist with default dangerous pairs pre-loaded
    pub fn with_defaults() -> Self {
        let mut blocklist = Self::new();
        blocklist.load_default_entries();
        blocklist
    }

    /// Normalize a medication name for consistent lookup
    fn normalize(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate a canonical key for a medication pair (order-independent)
    fn make_key(med_a: &str, med_b: &str) -> String {
        let a = Self::normalize(med_a);
        let b = Self::normalize(med_b);

        // Sort alphabetically to ensure consistent key regardless of order
        if a <= b {
            format!("{}|{}", a, b)
        } else {
            format!("{}|{}", b, a)
        }
    }

    /// Check if a medication pair is blocked
    /// Returns the blocklist entry if blocked, None otherwise
    pub fn is_blocked(&self, med_a: &str, med_b: &str) -> Option<&BlocklistEntry> {
        let key = Self::make_key(med_a, med_b);
        self.entries.get(&key)
    }

    /// Add an entry to the blocklist
    pub fn add_entry(&mut self, entry: BlocklistEntry) {
        let key = Self::make_key(&entry.medication_a, &entry.medication_b);
        self.entries.insert(key, entry);
    }

    /// Remove an entry from the blocklist
    /// Returns true if an entry was removed, false if not found
    pub fn remove_entry(&mut self, med_a: &str, med_b: &str) -> bool {
        let key = Self::make_key(med_a, med_b);
        self.entries.remove(&key).is_some()
    }

    /// Get the number of entries in the blocklist
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the blocklist is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all entries in the blocklist
    pub fn entries(&self) -> impl Iterator<Item = &BlocklistEntry> {
        self.entries.values()
    }

    /// Clear all entries from the blocklist
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Load blocklist entries from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<usize, BlocklistError> {
        let content = fs::read_to_string(path)?;
        let entries: Vec<BlocklistEntry> = serde_json::from_str(&content)?;

        let count = entries.len();
        for entry in entries {
            self.add_entry(entry);
        }

        Ok(count)
    }

    /// Save blocklist entries to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), BlocklistError> {
        let entries: Vec<&BlocklistEntry> = self.entries.values().collect();
        let content = serde_json::to_string_pretty(&entries)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Load default dangerous medication pairs
    fn load_default_entries(&mut self) {
        // Critical: Sound-alike medications with very different therapeutic uses
        self.add_entry(BlocklistEntry::critical(
            "Metformin",
            "Metoprolol",
            "Sound-alike: Metformin (antidiabetic) vs Metoprolol (beta-blocker). Confusion could cause serious harm.",
        ));

        self.add_entry(BlocklistEntry::critical(
            "Hydroxyzine",
            "Hydralazine",
            "Sound-alike: Hydroxyzine (antihistamine) vs Hydralazine (antihypertensive). Different therapeutic classes.",
        ));

        self.add_entry(BlocklistEntry::critical(
            "Prednisone",
            "Prednisolone",
            "Sound-alike: Different corticosteroids with different potencies and dosing.",
        ));

        self.add_entry(BlocklistEntry::critical(
            "Clonidine",
            "Clonazepam",
            "Sound-alike: Clonidine (antihypertensive) vs Clonazepam (benzodiazepine). Very different uses.",
        ));

        self.add_entry(BlocklistEntry::critical(
            "Celebrex",
            "Celexa",
            "Sound-alike: Celebrex (NSAID) vs Celexa (antidepressant). Brand name confusion.",
        ));

        // High: Similar names but different medications
        self.add_entry(BlocklistEntry::high(
            "Losartan",
            "Lisinopril",
            "Both antihypertensives but different classes (ARB vs ACE inhibitor). Should not be substituted.",
        ));

        self.add_entry(BlocklistEntry::high(
            "Omeprazole",
            "Esomeprazole",
            "Related PPIs but different dosing and formulations. Not directly interchangeable.",
        ));

        self.add_entry(BlocklistEntry::high(
            "Tramadol",
            "Trazodone",
            "Sound-alike: Tramadol (opioid analgesic) vs Trazodone (antidepressant). Different uses.",
        ));

        self.add_entry(BlocklistEntry::high(
            "Lamictal",
            "Lamisil",
            "Sound-alike: Lamictal (anticonvulsant) vs Lamisil (antifungal). Brand name confusion.",
        ));

        self.add_entry(BlocklistEntry::high(
            "Zyrtec",
            "Zyprexa",
            "Sound-alike: Zyrtec (antihistamine) vs Zyprexa (antipsychotic). Brand name confusion.",
        ));

        // Medium: Similar therapeutic class but not interchangeable
        self.add_entry(BlocklistEntry::medium(
            "Atorvastatin",
            "Rosuvastatin",
            "Both statins but different potencies. Dose conversion required, not direct substitution.",
        ));

        self.add_entry(BlocklistEntry::medium(
            "Amlodipine",
            "Nifedipine",
            "Both calcium channel blockers but different release profiles and dosing.",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // =========================================================================
    // BlocklistEntry Tests
    // =========================================================================

    #[test]
    fn test_blocklist_entry_new() {
        let entry = BlocklistEntry::new("MedA", "MedB", "Test reason", BlocklistSeverity::High);

        assert_eq!(entry.medication_a, "MedA");
        assert_eq!(entry.medication_b, "MedB");
        assert_eq!(entry.reason, "Test reason");
        assert_eq!(entry.severity, BlocklistSeverity::High);
    }

    #[test]
    fn test_blocklist_entry_critical() {
        let entry = BlocklistEntry::critical("MedA", "MedB", "Critical reason");
        assert_eq!(entry.severity, BlocklistSeverity::Critical);
    }

    #[test]
    fn test_blocklist_entry_high() {
        let entry = BlocklistEntry::high("MedA", "MedB", "High reason");
        assert_eq!(entry.severity, BlocklistSeverity::High);
    }

    #[test]
    fn test_blocklist_entry_medium() {
        let entry = BlocklistEntry::medium("MedA", "MedB", "Medium reason");
        assert_eq!(entry.severity, BlocklistSeverity::Medium);
    }

    // =========================================================================
    // MedicationBlocklist Basic Tests
    // =========================================================================

    #[test]
    fn test_blocklist_new_empty() {
        let blocklist = MedicationBlocklist::new();
        assert!(blocklist.is_empty());
        assert_eq!(blocklist.len(), 0);
    }

    #[test]
    fn test_blocklist_with_defaults() {
        let blocklist = MedicationBlocklist::with_defaults();
        assert!(!blocklist.is_empty());
        // Should have at least the critical pairs
        assert!(blocklist.len() >= 5);
    }

    #[test]
    fn test_blocklist_add_entry() {
        let mut blocklist = MedicationBlocklist::new();
        let entry = BlocklistEntry::high("Aspirin", "Ibuprofen", "Test");

        blocklist.add_entry(entry);

        assert_eq!(blocklist.len(), 1);
        assert!(!blocklist.is_empty());
    }

    #[test]
    fn test_blocklist_remove_entry() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Aspirin", "Ibuprofen", "Test"));

        assert!(blocklist.remove_entry("Aspirin", "Ibuprofen"));
        assert!(blocklist.is_empty());
    }

    #[test]
    fn test_blocklist_remove_nonexistent() {
        let mut blocklist = MedicationBlocklist::new();
        assert!(!blocklist.remove_entry("Aspirin", "Ibuprofen"));
    }

    #[test]
    fn test_blocklist_clear() {
        let mut blocklist = MedicationBlocklist::with_defaults();
        assert!(!blocklist.is_empty());

        blocklist.clear();
        assert!(blocklist.is_empty());
    }

    // =========================================================================
    // is_blocked Tests
    // =========================================================================

    #[test]
    fn test_is_blocked_exact_match() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Metformin", "Metoprolol", "Test"));

        let result = blocklist.is_blocked("Metformin", "Metoprolol");
        assert!(result.is_some());
        assert_eq!(result.unwrap().medication_a, "Metformin");
    }

    #[test]
    fn test_is_blocked_reverse_order() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Metformin", "Metoprolol", "Test"));

        // Should find the entry regardless of order
        let result = blocklist.is_blocked("Metoprolol", "Metformin");
        assert!(result.is_some());
    }

    #[test]
    fn test_is_blocked_case_insensitive() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Metformin", "Metoprolol", "Test"));

        // Should match regardless of case
        let result = blocklist.is_blocked("METFORMIN", "metoprolol");
        assert!(result.is_some());
    }

    #[test]
    fn test_is_blocked_with_extra_whitespace() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Metformin", "Metoprolol", "Test"));

        // Should normalize whitespace
        let result = blocklist.is_blocked("  Metformin  ", "  Metoprolol  ");
        assert!(result.is_some());
    }

    #[test]
    fn test_is_blocked_not_found() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("Metformin", "Metoprolol", "Test"));

        let result = blocklist.is_blocked("Aspirin", "Ibuprofen");
        assert!(result.is_none());
    }

    #[test]
    fn test_is_blocked_same_medication() {
        let blocklist = MedicationBlocklist::new();

        // Same medication should not be blocked (it's not in the list)
        let result = blocklist.is_blocked("Aspirin", "Aspirin");
        assert!(result.is_none());
    }

    // =========================================================================
    // Default Entries Tests
    // =========================================================================

    #[test]
    fn test_default_metformin_metoprolol_blocked() {
        let blocklist = MedicationBlocklist::with_defaults();

        let result = blocklist.is_blocked("Metformin", "Metoprolol");
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, BlocklistSeverity::Critical);
    }

    #[test]
    fn test_default_hydroxyzine_hydralazine_blocked() {
        let blocklist = MedicationBlocklist::with_defaults();

        let result = blocklist.is_blocked("Hydroxyzine", "Hydralazine");
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, BlocklistSeverity::Critical);
    }

    #[test]
    fn test_default_celebrex_celexa_blocked() {
        let blocklist = MedicationBlocklist::with_defaults();

        let result = blocklist.is_blocked("Celebrex", "Celexa");
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, BlocklistSeverity::Critical);
    }

    // =========================================================================
    // Normalization Tests
    // =========================================================================

    #[test]
    fn test_normalize_lowercase() {
        assert_eq!(MedicationBlocklist::normalize("ASPIRIN"), "aspirin");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(
            MedicationBlocklist::normalize("  Aspirin  100mg  "),
            "aspirin 100mg"
        );
    }

    #[test]
    fn test_normalize_special_chars() {
        assert_eq!(
            MedicationBlocklist::normalize("Aspirin-100mg"),
            "aspirin100mg"
        );
    }

    #[test]
    fn test_make_key_order_independent() {
        let key1 = MedicationBlocklist::make_key("Aspirin", "Ibuprofen");
        let key2 = MedicationBlocklist::make_key("Ibuprofen", "Aspirin");
        assert_eq!(key1, key2);
    }

    // =========================================================================
    // File I/O Tests
    // =========================================================================

    #[test]
    fn test_save_and_load_file() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("MedA", "MedB", "Test reason"));
        blocklist.add_entry(BlocklistEntry::critical("MedC", "MedD", "Critical reason"));

        // Create a temp file path
        let temp_dir = env::temp_dir();
        let temp_path = temp_dir.join(format!("blocklist_test_{}.json", uuid::Uuid::new_v4()));

        // Save to temp file
        blocklist.save_to_file(&temp_path).unwrap();

        // Load into new blocklist
        let mut loaded = MedicationBlocklist::new();
        let count = loaded.load_from_file(&temp_path).unwrap();

        // Cleanup
        let _ = fs::remove_file(&temp_path);

        assert_eq!(count, 2);
        assert_eq!(loaded.len(), 2);
        assert!(loaded.is_blocked("MedA", "MedB").is_some());
        assert!(loaded.is_blocked("MedC", "MedD").is_some());
    }

    #[test]
    fn test_load_file_not_found() {
        let mut blocklist = MedicationBlocklist::new();
        let result = blocklist.load_from_file("/nonexistent/path/blocklist.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json() {
        let temp_dir = env::temp_dir();
        let temp_path = temp_dir.join(format!("blocklist_invalid_{}.json", uuid::Uuid::new_v4()));

        fs::write(&temp_path, "not valid json").unwrap();

        let mut blocklist = MedicationBlocklist::new();
        let result = blocklist.load_from_file(&temp_path);

        // Cleanup
        let _ = fs::remove_file(&temp_path);

        assert!(result.is_err());
    }

    // =========================================================================
    // Iterator Tests
    // =========================================================================

    #[test]
    fn test_entries_iterator() {
        let mut blocklist = MedicationBlocklist::new();
        blocklist.add_entry(BlocklistEntry::high("MedA", "MedB", "Test1"));
        blocklist.add_entry(BlocklistEntry::high("MedC", "MedD", "Test2"));

        let entries: Vec<_> = blocklist.entries().collect();
        assert_eq!(entries.len(), 2);
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_entry_serialization() {
        let entry = BlocklistEntry::critical("MedA", "MedB", "Test reason");
        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("MedA"));
        assert!(json.contains("MedB"));
        assert!(json.contains("Critical"));
    }

    #[test]
    fn test_entry_deserialization() {
        let json = r#"{
            "medication_a": "MedA",
            "medication_b": "MedB",
            "reason": "Test reason",
            "severity": "High"
        }"#;

        let entry: BlocklistEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.medication_a, "MedA");
        assert_eq!(entry.medication_b, "MedB");
        assert_eq!(entry.severity, BlocklistSeverity::High);
    }

    #[test]
    fn test_severity_serialization() {
        assert_eq!(
            serde_json::to_string(&BlocklistSeverity::Critical).unwrap(),
            "\"Critical\""
        );
        assert_eq!(
            serde_json::to_string(&BlocklistSeverity::High).unwrap(),
            "\"High\""
        );
        assert_eq!(
            serde_json::to_string(&BlocklistSeverity::Medium).unwrap(),
            "\"Medium\""
        );
    }
}
