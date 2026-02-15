//! LLM Feedback Loop Module
//!
//! Collects confirmed matches and extraction corrections to improve future AI parsing.
//! Implements Task 12 from advanced_matching_tasks.md.
//!
//! Key features:
//! - Tracks "hard matches" discovered by algorithms but missed by AI
//! - Stores few-shot examples for dynamic prompt enhancement
//! - Records extraction corrections (medication name mappings)
//! - Provides statistics on AI accuracy and improvement

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for the LLM feedback loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackLoopConfig {
    /// Enable feedback collection
    pub enabled: bool,
    /// Maximum number of few-shot examples to store
    pub max_examples: usize,
    /// Maximum number of medication corrections to store
    pub max_corrections: usize,
    /// Minimum confidence threshold for auto-adding examples
    pub min_confidence_for_example: f64,
    /// Number of examples to include in prompts
    pub examples_per_prompt: usize,
    /// Enable automatic prompt enhancement
    pub auto_enhance_prompts: bool,
    /// Days after which examples are considered stale
    pub example_staleness_days: i64,
}

impl Default for FeedbackLoopConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_examples: 100,
            max_corrections: 500,
            min_confidence_for_example: 0.85,
            examples_per_prompt: 3,
            auto_enhance_prompts: true,
            example_staleness_days: 90,
        }
    }
}

// =============================================================================
// Few-Shot Example
// =============================================================================

/// A few-shot example for prompt enhancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FewShotExample {
    /// Unique identifier
    pub id: String,
    /// Original message content
    pub input: String,
    /// Expected output (JSON string)
    pub expected_output: String,
    /// Category of the example
    pub category: ExampleCategory,
    /// Source of this example
    pub source: ExampleSource,
    /// Confidence score when this was confirmed
    pub confidence: f64,
    /// Number of times this pattern was confirmed
    pub confirmation_count: u32,
    /// When this example was created
    pub created_at: DateTime<Utc>,
    /// When this example was last used
    pub last_used: DateTime<Utc>,
    /// Usage count
    pub usage_count: u32,
}

/// Category of few-shot example
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExampleCategory {
    /// Arabic medication offer
    ArabicOffer,
    /// Arabic medication request
    ArabicRequest,
    /// English medication offer
    EnglishOffer,
    /// English medication request
    EnglishRequest,
    /// Multi-item message
    MultiItem,
    /// Edge case (noise, phone numbers, etc.)
    EdgeCase,
    /// Correction example (AI got it wrong)
    Correction,
}

impl ExampleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ArabicOffer => "arabic_offer",
            Self::ArabicRequest => "arabic_request",
            Self::EnglishOffer => "english_offer",
            Self::EnglishRequest => "english_request",
            Self::MultiItem => "multi_item",
            Self::EdgeCase => "edge_case",
            Self::Correction => "correction",
        }
    }
}

/// Source of the example
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExampleSource {
    /// Manually added by operator
    Manual,
    /// Auto-collected from confirmed matches
    AutoConfirmed,
    /// From operator correction
    OperatorCorrection,
    /// From algorithm discovery (hard match)
    AlgorithmDiscovery,
}

// =============================================================================
// Medication Correction
// =============================================================================

/// A medication name correction learned from feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationCorrection {
    /// Original text extracted by AI
    pub original: String,
    /// Corrected/canonical medication name
    pub corrected: String,
    /// Language of the original text
    pub language: Language,
    /// Number of times this correction was applied
    pub correction_count: u32,
    /// When this correction was first recorded
    pub created_at: DateTime<Utc>,
    /// When this correction was last used
    pub last_used: DateTime<Utc>,
}

/// Language indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Arabic,
    English,
    Mixed,
    Unknown,
}

impl Language {
    /// Detect language from text
    pub fn detect(text: &str) -> Self {
        let arabic_chars = text.chars().filter(|c| is_arabic_char(*c)).count();
        let total_chars = text.chars().filter(|c| c.is_alphabetic()).count();

        if total_chars == 0 {
            return Self::Unknown;
        }

        let arabic_ratio = arabic_chars as f64 / total_chars as f64;

        if arabic_ratio > 0.7 {
            Self::Arabic
        } else if arabic_ratio < 0.3 {
            Self::English
        } else {
            Self::Mixed
        }
    }
}

fn is_arabic_char(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}')
}

// =============================================================================
// Extraction Feedback
// =============================================================================

/// Feedback on an AI extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionFeedback {
    /// Original message ID
    pub message_id: String,
    /// Original message content
    pub message_content: String,
    /// What AI extracted
    pub ai_extraction: AIExtraction,
    /// What was actually correct (from operator/algorithm)
    pub correct_extraction: CorrectExtraction,
    /// Type of feedback
    pub feedback_type: FeedbackType,
    /// Who provided the feedback
    pub feedback_source: String,
    /// When feedback was provided
    pub created_at: DateTime<Utc>,
}

/// What the AI extracted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIExtraction {
    pub medication: Option<String>,
    pub item_type: Option<String>,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub confidence: f64,
}

/// The correct extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectExtraction {
    pub medication: String,
    pub item_type: String,
    pub quantity: f64,
    pub price: f64,
}

/// Type of feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackType {
    /// AI missed the item entirely
    Missed,
    /// AI got the medication name wrong
    WrongMedication,
    /// AI got the item type wrong (offer vs request)
    WrongType,
    /// AI got quantity/price wrong
    WrongDetails,
    /// AI extracted noise as medication
    FalsePositive,
    /// AI extraction was correct
    Correct,
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics for the feedback loop
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedbackLoopStats {
    pub total_examples: usize,
    pub examples_by_category: HashMap<String, usize>,
    pub total_corrections: usize,
    pub total_feedback_received: u64,
    pub correct_extractions: u64,
    pub missed_extractions: u64,
    pub wrong_extractions: u64,
    pub false_positives: u64,
    pub accuracy_rate: f64,
    pub examples_used_in_prompts: u64,
}

// =============================================================================
// LLM Feedback Loop
// =============================================================================

/// LLM Feedback Loop for continuous improvement
pub struct LLMFeedbackLoop {
    config: RwLock<FeedbackLoopConfig>,
    /// Few-shot examples by category
    examples: RwLock<HashMap<ExampleCategory, Vec<FewShotExample>>>,
    /// Medication corrections (original -> corrected)
    corrections: RwLock<HashMap<String, MedicationCorrection>>,
    /// Statistics
    total_feedback: AtomicU64,
    correct_count: AtomicU64,
    missed_count: AtomicU64,
    wrong_count: AtomicU64,
    false_positive_count: AtomicU64,
    examples_used: AtomicU64,
}

impl Default for LLMFeedbackLoop {
    fn default() -> Self {
        Self::new(FeedbackLoopConfig::default())
    }
}

impl LLMFeedbackLoop {
    /// Create a new feedback loop
    pub fn new(config: FeedbackLoopConfig) -> Self {
        Self {
            config: RwLock::new(config),
            examples: RwLock::new(HashMap::new()),
            corrections: RwLock::new(HashMap::new()),
            total_feedback: AtomicU64::new(0),
            correct_count: AtomicU64::new(0),
            missed_count: AtomicU64::new(0),
            wrong_count: AtomicU64::new(0),
            false_positive_count: AtomicU64::new(0),
            examples_used: AtomicU64::new(0),
        }
    }

    /// Record extraction feedback
    pub fn record_feedback(&self, feedback: ExtractionFeedback) {
        let config = self.config.read().unwrap();
        if !config.enabled {
            return;
        }
        drop(config);

        self.total_feedback.fetch_add(1, Ordering::Relaxed);

        match feedback.feedback_type {
            FeedbackType::Correct => {
                self.correct_count.fetch_add(1, Ordering::Relaxed);
                // Potentially add as positive example
                self.maybe_add_positive_example(&feedback);
            }
            FeedbackType::Missed => {
                self.missed_count.fetch_add(1, Ordering::Relaxed);
                // Add as correction example
                self.add_correction_example(&feedback);
            }
            FeedbackType::WrongMedication
            | FeedbackType::WrongType
            | FeedbackType::WrongDetails => {
                self.wrong_count.fetch_add(1, Ordering::Relaxed);
                // Record medication correction
                self.record_medication_correction(&feedback);
                // Add as correction example
                self.add_correction_example(&feedback);
            }
            FeedbackType::FalsePositive => {
                self.false_positive_count.fetch_add(1, Ordering::Relaxed);
                // Add as edge case example
                self.add_edge_case_example(&feedback);
            }
        }
    }

    /// Maybe add a positive example if confidence is high enough
    fn maybe_add_positive_example(&self, feedback: &ExtractionFeedback) {
        let config = self.config.read().unwrap();
        if feedback.ai_extraction.confidence < config.min_confidence_for_example {
            return;
        }
        drop(config);

        let category = self.categorize_message(
            &feedback.message_content,
            &feedback.correct_extraction.item_type,
        );
        let expected_output = serde_json::json!({
            "items": [{
                "type": feedback.correct_extraction.item_type,
                "medication": feedback.correct_extraction.medication,
                "medication_raw": feedback.correct_extraction.medication,
                "quantity": feedback.correct_extraction.quantity,
                "price": feedback.correct_extraction.price,
                "ai_confidence": feedback.ai_extraction.confidence
            }]
        });

        let example = FewShotExample {
            id: uuid::Uuid::new_v4().to_string(),
            input: feedback.message_content.clone(),
            expected_output: expected_output.to_string(),
            category,
            source: ExampleSource::AutoConfirmed,
            confidence: feedback.ai_extraction.confidence,
            confirmation_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };

        self.add_example(example);
    }

    /// Add a correction example
    fn add_correction_example(&self, feedback: &ExtractionFeedback) {
        let expected_output = serde_json::json!({
            "items": [{
                "type": feedback.correct_extraction.item_type,
                "medication": feedback.correct_extraction.medication,
                "medication_raw": feedback.correct_extraction.medication,
                "quantity": feedback.correct_extraction.quantity,
                "price": feedback.correct_extraction.price
            }]
        });

        let example = FewShotExample {
            id: uuid::Uuid::new_v4().to_string(),
            input: feedback.message_content.clone(),
            expected_output: expected_output.to_string(),
            category: ExampleCategory::Correction,
            source: ExampleSource::OperatorCorrection,
            confidence: 1.0, // Operator-confirmed
            confirmation_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };

        self.add_example(example);
    }

    /// Add an edge case example (false positive)
    fn add_edge_case_example(&self, feedback: &ExtractionFeedback) {
        let expected_output = serde_json::json!({
            "items": []
        });

        let example = FewShotExample {
            id: uuid::Uuid::new_v4().to_string(),
            input: feedback.message_content.clone(),
            expected_output: expected_output.to_string(),
            category: ExampleCategory::EdgeCase,
            source: ExampleSource::OperatorCorrection,
            confidence: 1.0,
            confirmation_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };

        self.add_example(example);
    }

    /// Record a medication correction
    fn record_medication_correction(&self, feedback: &ExtractionFeedback) {
        if let Some(original) = &feedback.ai_extraction.medication {
            let key = original.to_lowercase();
            let mut corrections = self.corrections.write().unwrap();

            if let Some(existing) = corrections.get_mut(&key) {
                existing.correction_count += 1;
                existing.last_used = Utc::now();
            } else {
                let config = self.config.read().unwrap();
                if corrections.len() < config.max_corrections {
                    corrections.insert(
                        key,
                        MedicationCorrection {
                            original: original.clone(),
                            corrected: feedback.correct_extraction.medication.clone(),
                            language: Language::detect(original),
                            correction_count: 1,
                            created_at: Utc::now(),
                            last_used: Utc::now(),
                        },
                    );
                }
            }
        }
    }

    /// Add an example to the store
    fn add_example(&self, example: FewShotExample) {
        let config = self.config.read().unwrap();
        let max = config.max_examples;
        drop(config);

        let mut examples = self.examples.write().unwrap();
        let category_examples = examples.entry(example.category).or_default();

        // Check if similar example exists
        let existing = category_examples
            .iter_mut()
            .find(|e| Self::is_similar_example(e, &example));

        if let Some(existing) = existing {
            existing.confirmation_count += 1;
            existing.last_used = Utc::now();
        } else if category_examples.len() < max {
            category_examples.push(example);
        } else {
            // Replace oldest/least used example
            if let Some(idx) = Self::find_replaceable_example(category_examples) {
                category_examples[idx] = example;
            }
        }
    }

    /// Check if two examples are similar
    fn is_similar_example(a: &FewShotExample, b: &FewShotExample) -> bool {
        // Simple similarity: same category and similar input
        if a.category != b.category {
            return false;
        }

        let len_a = a.input.len();
        let len_b = b.input.len();
        let diff = (len_a as i64 - len_b as i64).unsigned_abs() as usize;

        // Similar length and same prefix (char-safe)
        diff < 50 && {
            let prefix_len = 20.min(a.input.chars().count()).min(b.input.chars().count());
            let a_prefix: String = a.input.chars().take(prefix_len).collect();
            let b_prefix: String = b.input.chars().take(prefix_len).collect();
            a_prefix == b_prefix
        }
    }

    /// Find an example that can be replaced
    fn find_replaceable_example(examples: &[FewShotExample]) -> Option<usize> {
        examples
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| (e.confirmation_count, e.usage_count))
            .map(|(idx, _)| idx)
    }

    /// Categorize a message
    fn categorize_message(&self, content: &str, item_type: &str) -> ExampleCategory {
        let lang = Language::detect(content);
        let is_offer = item_type.eq_ignore_ascii_case("offer");

        match (lang, is_offer) {
            (Language::Arabic, true) => ExampleCategory::ArabicOffer,
            (Language::Arabic, false) => ExampleCategory::ArabicRequest,
            (Language::English, true) => ExampleCategory::EnglishOffer,
            (Language::English, false) => ExampleCategory::EnglishRequest,
            _ => {
                if is_offer {
                    ExampleCategory::EnglishOffer
                } else {
                    ExampleCategory::EnglishRequest
                }
            }
        }
    }

    /// Get few-shot examples for prompt enhancement
    pub fn get_examples_for_prompt(&self, content: &str) -> Vec<FewShotExample> {
        let config = self.config.read().unwrap();
        if !config.enabled || !config.auto_enhance_prompts {
            return Vec::new();
        }
        let count = config.examples_per_prompt;
        drop(config);

        let lang = Language::detect(content);
        let examples = self.examples.read().unwrap();

        let mut selected = Vec::new();

        // Get relevant category examples
        let categories = match lang {
            Language::Arabic => vec![ExampleCategory::ArabicOffer, ExampleCategory::ArabicRequest],
            Language::English => vec![
                ExampleCategory::EnglishOffer,
                ExampleCategory::EnglishRequest,
            ],
            _ => vec![ExampleCategory::ArabicOffer, ExampleCategory::EnglishOffer],
        };

        for category in categories {
            if let Some(cat_examples) = examples.get(&category) {
                // Get top examples by confirmation count
                let mut sorted: Vec<_> = cat_examples.iter().collect();
                sorted.sort_by_key(|e| std::cmp::Reverse(e.confirmation_count));
                selected.extend(sorted.into_iter().take(count / 2).cloned());
            }
        }

        // Always include some correction examples
        if let Some(corrections) = examples.get(&ExampleCategory::Correction) {
            let mut sorted: Vec<_> = corrections.iter().collect();
            sorted.sort_by_key(|e| std::cmp::Reverse(e.confirmation_count));
            selected.extend(sorted.into_iter().take(1).cloned());
        }

        // Always include edge case examples
        if let Some(edge_cases) = examples.get(&ExampleCategory::EdgeCase) {
            let mut sorted: Vec<_> = edge_cases.iter().collect();
            sorted.sort_by_key(|e| std::cmp::Reverse(e.confirmation_count));
            selected.extend(sorted.into_iter().take(1).cloned());
        }

        // Update usage stats
        self.examples_used
            .fetch_add(selected.len() as u64, Ordering::Relaxed);

        // Mark examples as used
        drop(examples);
        let mut examples = self.examples.write().unwrap();
        for example in &selected {
            for cat_examples in examples.values_mut() {
                if let Some(e) = cat_examples.iter_mut().find(|e| e.id == example.id) {
                    e.usage_count += 1;
                    e.last_used = Utc::now();
                }
            }
        }

        selected.truncate(count);
        selected
    }

    /// Build enhanced prompt with few-shot examples
    pub fn build_enhanced_prompt(&self, base_prompt: &str, content: &str) -> String {
        let examples = self.get_examples_for_prompt(content);

        if examples.is_empty() {
            return base_prompt.to_string();
        }

        let mut enhanced = base_prompt.to_string();
        enhanced.push_str("\n\n# LEARNED EXAMPLES (from confirmed matches)\n");

        for (idx, example) in examples.iter().enumerate() {
            enhanced.push_str(&format!(
                "\n## Example {} ({})\nInput: {}\nOutput: {}\n",
                idx + 1,
                example.category.as_str(),
                example.input,
                example.expected_output
            ));
        }

        enhanced
    }

    /// Get medication corrections as mappings
    pub fn get_medication_mappings(&self) -> Vec<String> {
        let corrections = self.corrections.read().unwrap();
        corrections
            .values()
            .map(|c| format!("{} -> {}", c.original, c.corrected))
            .collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> FeedbackLoopStats {
        let examples = self.examples.read().unwrap();
        let corrections = self.corrections.read().unwrap();

        let total_examples: usize = examples.values().map(|v| v.len()).sum();
        let examples_by_category: HashMap<String, usize> = examples
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.len()))
            .collect();

        let total_feedback = self.total_feedback.load(Ordering::Relaxed);
        let correct = self.correct_count.load(Ordering::Relaxed);

        let accuracy_rate = if total_feedback > 0 {
            correct as f64 / total_feedback as f64
        } else {
            0.0
        };

        FeedbackLoopStats {
            total_examples,
            examples_by_category,
            total_corrections: corrections.len(),
            total_feedback_received: total_feedback,
            correct_extractions: correct,
            missed_extractions: self.missed_count.load(Ordering::Relaxed),
            wrong_extractions: self.wrong_count.load(Ordering::Relaxed),
            false_positives: self.false_positive_count.load(Ordering::Relaxed),
            accuracy_rate,
            examples_used_in_prompts: self.examples_used.load(Ordering::Relaxed),
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> FeedbackLoopConfig {
        self.config.read().unwrap().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: FeedbackLoopConfig) {
        *self.config.write().unwrap() = config;
    }

    /// Enable or disable feedback loop
    pub fn enable(&self, enabled: bool) {
        self.config.write().unwrap().enabled = enabled;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    /// Clear all learned data
    pub fn clear(&self) {
        self.examples.write().unwrap().clear();
        self.corrections.write().unwrap().clear();
        self.total_feedback.store(0, Ordering::Relaxed);
        self.correct_count.store(0, Ordering::Relaxed);
        self.missed_count.store(0, Ordering::Relaxed);
        self.wrong_count.store(0, Ordering::Relaxed);
        self.false_positive_count.store(0, Ordering::Relaxed);
        self.examples_used.store(0, Ordering::Relaxed);
    }

    /// Export all examples for persistence
    pub fn export_examples(&self) -> Vec<FewShotExample> {
        let examples = self.examples.read().unwrap();
        examples.values().flatten().cloned().collect()
    }

    /// Import examples from external source
    pub fn import_examples(&self, examples: Vec<FewShotExample>) {
        let mut store = self.examples.write().unwrap();
        for example in examples {
            store.entry(example.category).or_default().push(example);
        }
    }

    /// Export corrections for persistence
    pub fn export_corrections(&self) -> Vec<MedicationCorrection> {
        self.corrections.read().unwrap().values().cloned().collect()
    }

    /// Import corrections from external source
    pub fn import_corrections(&self, corrections: Vec<MedicationCorrection>) {
        let mut store = self.corrections.write().unwrap();
        for correction in corrections {
            store.insert(correction.original.to_lowercase(), correction);
        }
    }

    /// Get example count
    pub fn example_count(&self) -> usize {
        self.examples
            .read()
            .unwrap()
            .values()
            .map(|v| v.len())
            .sum()
    }

    /// Get correction count
    pub fn correction_count(&self) -> usize {
        self.corrections.read().unwrap().len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Language Detection Tests
    // =========================================================================

    #[test]
    fn test_language_detect_arabic() {
        let text = "متوفر اوجمنتين 1 جم";
        assert_eq!(Language::detect(text), Language::Arabic);
    }

    #[test]
    fn test_language_detect_english() {
        let text = "Looking for Augmentin 1g";
        assert_eq!(Language::detect(text), Language::English);
    }

    #[test]
    fn test_language_detect_mixed() {
        let text = "متوفر Augmentin 1g";
        assert_eq!(Language::detect(text), Language::Mixed);
    }

    #[test]
    fn test_language_detect_unknown() {
        let text = "12345";
        assert_eq!(Language::detect(text), Language::Unknown);
    }

    // =========================================================================
    // FeedbackLoop Basic Tests
    // =========================================================================

    #[test]
    fn test_feedback_loop_default() {
        let loop_ = LLMFeedbackLoop::default();
        assert!(loop_.is_enabled());
        assert_eq!(loop_.example_count(), 0);
        assert_eq!(loop_.correction_count(), 0);
    }

    #[test]
    fn test_feedback_loop_disabled() {
        let config = FeedbackLoopConfig {
            enabled: false,
            ..Default::default()
        };
        let loop_ = LLMFeedbackLoop::new(config);

        let feedback = create_test_feedback(FeedbackType::Correct);
        loop_.record_feedback(feedback);

        // Should not record when disabled
        let stats = loop_.get_stats();
        assert_eq!(stats.total_feedback_received, 0);
    }

    #[test]
    fn test_record_correct_feedback() {
        let loop_ = LLMFeedbackLoop::default();

        let feedback = create_test_feedback(FeedbackType::Correct);
        loop_.record_feedback(feedback);

        let stats = loop_.get_stats();
        assert_eq!(stats.total_feedback_received, 1);
        assert_eq!(stats.correct_extractions, 1);
    }

    #[test]
    fn test_record_missed_feedback() {
        let loop_ = LLMFeedbackLoop::default();

        let feedback = create_test_feedback(FeedbackType::Missed);
        loop_.record_feedback(feedback);

        let stats = loop_.get_stats();
        assert_eq!(stats.total_feedback_received, 1);
        assert_eq!(stats.missed_extractions, 1);
        // Should add correction example
        assert!(loop_.example_count() > 0);
    }

    #[test]
    fn test_record_wrong_medication_feedback() {
        let loop_ = LLMFeedbackLoop::default();

        let feedback = create_test_feedback(FeedbackType::WrongMedication);
        loop_.record_feedback(feedback);

        let stats = loop_.get_stats();
        assert_eq!(stats.wrong_extractions, 1);
        // Should record medication correction
        assert!(loop_.correction_count() > 0);
    }

    #[test]
    fn test_record_false_positive_feedback() {
        let loop_ = LLMFeedbackLoop::default();

        let feedback = create_test_feedback(FeedbackType::FalsePositive);
        loop_.record_feedback(feedback);

        let stats = loop_.get_stats();
        assert_eq!(stats.false_positives, 1);
        // Should add edge case example
        assert!(loop_.example_count() > 0);
    }

    // =========================================================================
    // Example Management Tests
    // =========================================================================

    #[test]
    fn test_get_examples_for_prompt_arabic() {
        let loop_ = LLMFeedbackLoop::default();

        // Add some Arabic examples
        let example = FewShotExample {
            id: "1".to_string(),
            input: "متوفر اوجمنتين".to_string(),
            expected_output: r#"{"items":[]}"#.to_string(),
            category: ExampleCategory::ArabicOffer,
            source: ExampleSource::Manual,
            confidence: 0.95,
            confirmation_count: 5,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };
        loop_.import_examples(vec![example]);

        let examples = loop_.get_examples_for_prompt("محتاج دواء");
        assert!(!examples.is_empty());
    }

    #[test]
    fn test_get_examples_for_prompt_english() {
        let loop_ = LLMFeedbackLoop::default();

        let example = FewShotExample {
            id: "1".to_string(),
            input: "Looking for Augmentin".to_string(),
            expected_output: r#"{"items":[]}"#.to_string(),
            category: ExampleCategory::EnglishRequest,
            source: ExampleSource::Manual,
            confidence: 0.95,
            confirmation_count: 5,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };
        loop_.import_examples(vec![example]);

        let examples = loop_.get_examples_for_prompt("Need medication");
        assert!(!examples.is_empty());
    }

    #[test]
    fn test_build_enhanced_prompt() {
        let loop_ = LLMFeedbackLoop::default();

        let example = FewShotExample {
            id: "1".to_string(),
            input: "Test input".to_string(),
            expected_output: r#"{"items":[]}"#.to_string(),
            category: ExampleCategory::EnglishOffer,
            source: ExampleSource::Manual,
            confidence: 0.95,
            confirmation_count: 5,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };
        loop_.import_examples(vec![example]);

        let enhanced = loop_.build_enhanced_prompt("Base prompt", "Test content");
        assert!(enhanced.contains("Base prompt"));
        assert!(enhanced.contains("LEARNED EXAMPLES"));
    }

    // =========================================================================
    // Medication Correction Tests
    // =========================================================================

    #[test]
    fn test_medication_mappings() {
        let loop_ = LLMFeedbackLoop::default();

        let feedback = ExtractionFeedback {
            message_id: "msg-1".to_string(),
            message_content: "متوفر بروفين".to_string(),
            ai_extraction: AIExtraction {
                medication: Some("Brofen".to_string()),
                item_type: Some("OFFER".to_string()),
                quantity: Some(1.0),
                price: Some(100.0),
                confidence: 0.8,
            },
            correct_extraction: CorrectExtraction {
                medication: "Brufen".to_string(),
                item_type: "OFFER".to_string(),
                quantity: 1.0,
                price: 100.0,
            },
            feedback_type: FeedbackType::WrongMedication,
            feedback_source: "operator".to_string(),
            created_at: Utc::now(),
        };

        loop_.record_feedback(feedback);

        let mappings = loop_.get_medication_mappings();
        assert!(!mappings.is_empty());
        assert!(mappings[0].contains("Brofen"));
        assert!(mappings[0].contains("Brufen"));
    }

    // =========================================================================
    // Export/Import Tests
    // =========================================================================

    #[test]
    fn test_export_import_examples() {
        let loop1 = LLMFeedbackLoop::default();

        let example = FewShotExample {
            id: "1".to_string(),
            input: "Test".to_string(),
            expected_output: "{}".to_string(),
            category: ExampleCategory::ArabicOffer,
            source: ExampleSource::Manual,
            confidence: 0.9,
            confirmation_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
            usage_count: 0,
        };
        loop1.import_examples(vec![example]);

        let exported = loop1.export_examples();
        assert_eq!(exported.len(), 1);

        let loop2 = LLMFeedbackLoop::default();
        loop2.import_examples(exported);
        assert_eq!(loop2.example_count(), 1);
    }

    #[test]
    fn test_export_import_corrections() {
        let loop1 = LLMFeedbackLoop::default();

        let correction = MedicationCorrection {
            original: "Brofen".to_string(),
            corrected: "Brufen".to_string(),
            language: Language::English,
            correction_count: 1,
            created_at: Utc::now(),
            last_used: Utc::now(),
        };
        loop1.import_corrections(vec![correction]);

        let exported = loop1.export_corrections();
        assert_eq!(exported.len(), 1);

        let loop2 = LLMFeedbackLoop::default();
        loop2.import_corrections(exported);
        assert_eq!(loop2.correction_count(), 1);
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_accuracy_rate() {
        let loop_ = LLMFeedbackLoop::default();

        // Record 8 correct, 2 wrong
        for _ in 0..8 {
            loop_.record_feedback(create_test_feedback(FeedbackType::Correct));
        }
        for _ in 0..2 {
            loop_.record_feedback(create_test_feedback(FeedbackType::WrongMedication));
        }

        let stats = loop_.get_stats();
        assert!((stats.accuracy_rate - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_clear() {
        let loop_ = LLMFeedbackLoop::default();

        loop_.record_feedback(create_test_feedback(FeedbackType::Correct));
        loop_.record_feedback(create_test_feedback(FeedbackType::WrongMedication));

        assert!(loop_.get_stats().total_feedback_received > 0);

        loop_.clear();

        let stats = loop_.get_stats();
        assert_eq!(stats.total_feedback_received, 0);
        assert_eq!(stats.total_examples, 0);
        assert_eq!(stats.total_corrections, 0);
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn create_test_feedback(feedback_type: FeedbackType) -> ExtractionFeedback {
        ExtractionFeedback {
            message_id: uuid::Uuid::new_v4().to_string(),
            message_content: "متوفر اوجمنتين 1 جم".to_string(),
            ai_extraction: AIExtraction {
                medication: Some("Augmentin 1g".to_string()),
                item_type: Some("OFFER".to_string()),
                quantity: Some(1.0),
                price: Some(300.0),
                confidence: 0.9,
            },
            correct_extraction: CorrectExtraction {
                medication: "Augmentin 1g".to_string(),
                item_type: "OFFER".to_string(),
                quantity: 1.0,
                price: 300.0,
            },
            feedback_type,
            feedback_source: "test".to_string(),
            created_at: Utc::now(),
        }
    }

    // =========================================================================
    // Category Tests
    // =========================================================================

    #[test]
    fn test_example_category_as_str() {
        assert_eq!(ExampleCategory::ArabicOffer.as_str(), "arabic_offer");
        assert_eq!(ExampleCategory::EnglishRequest.as_str(), "english_request");
        assert_eq!(ExampleCategory::Correction.as_str(), "correction");
        assert_eq!(ExampleCategory::EdgeCase.as_str(), "edge_case");
    }
}
