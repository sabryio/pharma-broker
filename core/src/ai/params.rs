//! Parameter structs for AI parser feedback methods
//!
//! Provides structured parameter types to avoid too_many_arguments warnings.
//! Callers should use struct literal syntax to construct these types.

/// Common extraction data shared across feedback methods
#[derive(Debug, Clone, Default)]
pub struct ExtractionData {
    pub message_id: String,
    pub message_content: String,
    pub medication: String,
    pub item_type: String,
    pub quantity: f64,
    pub price: f64,
    pub ai_confidence: f64,
}

/// Data for recording medication corrections
#[derive(Debug, Clone, Default)]
pub struct MedicationCorrectionData {
    pub message_id: String,
    pub message_content: String,
    pub ai_medication: String,
    pub correct_medication: String,
    pub item_type: String,
    pub quantity: f64,
    pub price: f64,
    pub ai_confidence: f64,
}

/// Data for recording missed extractions
#[derive(Debug, Clone, Default)]
pub struct MissedExtractionData {
    pub message_id: String,
    pub message_content: String,
    pub medication: String,
    pub item_type: String,
    pub quantity: f64,
    pub price: f64,
}
