//! Pharma-specific AI types with JSON Schema support
//!
//! These types derive `schemars::JsonSchema` for structured output via the AI client.
//!
//! Note: `UrgencyLevel` is defined here with `JsonSchema` for AI parsing.
//! The database entity version in `pharma_db::entity::common` is used for persistence.
//! Conversion methods are provided for interoperability.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Urgency level for medication requests/offers (AI parsing version)
///
/// This version includes `JsonSchema` for AI structured output.
/// Use `to_db_urgency()` to convert to the database entity version.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UrgencyLevel {
    /// Normal priority - no urgency indicated
    #[default]
    #[serde(rename = "normal")]
    Normal,
    /// Moderate urgency - needed soon
    #[serde(rename = "soon")]
    Soon,
    /// High urgency - needed urgently
    #[serde(rename = "urgent")]
    Urgent,
    /// Critical urgency - immediate need
    #[serde(rename = "critical")]
    Critical,
}

impl UrgencyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            UrgencyLevel::Normal => "normal",
            UrgencyLevel::Soon => "soon",
            UrgencyLevel::Urgent => "urgent",
            UrgencyLevel::Critical => "critical",
        }
    }

    /// Convert from boolean urgent flag (backward compatibility)
    pub fn from_bool(urgent: bool) -> Self {
        if urgent {
            UrgencyLevel::Urgent
        } else {
            UrgencyLevel::Normal
        }
    }

    /// Check if this is any level of urgency
    pub fn is_urgent(&self) -> bool {
        !matches!(self, UrgencyLevel::Normal)
    }

    /// Get priority score (0.0 = normal, 1.0 = critical)
    pub fn priority_score(&self) -> f64 {
        match self {
            UrgencyLevel::Normal => 0.0,
            UrgencyLevel::Soon => 0.3,
            UrgencyLevel::Urgent => 0.7,
            UrgencyLevel::Critical => 1.0,
        }
    }

    /// Convert to database entity UrgencyLevel
    pub fn to_db_urgency(&self) -> pharma_db::entity::common::UrgencyLevel {
        match self {
            UrgencyLevel::Normal => pharma_db::entity::common::UrgencyLevel::Normal,
            UrgencyLevel::Soon => pharma_db::entity::common::UrgencyLevel::Soon,
            UrgencyLevel::Urgent => pharma_db::entity::common::UrgencyLevel::Urgent,
            UrgencyLevel::Critical => pharma_db::entity::common::UrgencyLevel::Critical,
        }
    }

    /// Convert from database entity UrgencyLevel
    pub fn from_db_urgency(db: pharma_db::entity::common::UrgencyLevel) -> Self {
        match db {
            pharma_db::entity::common::UrgencyLevel::Normal => UrgencyLevel::Normal,
            pharma_db::entity::common::UrgencyLevel::Soon => UrgencyLevel::Soon,
            pharma_db::entity::common::UrgencyLevel::Urgent => UrgencyLevel::Urgent,
            pharma_db::entity::common::UrgencyLevel::Critical => UrgencyLevel::Critical,
        }
    }
}

impl fmt::Display for UrgencyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<UrgencyLevel> for pharma_db::entity::common::UrgencyLevel {
    fn from(ai: UrgencyLevel) -> Self {
        ai.to_db_urgency()
    }
}

impl From<pharma_db::entity::common::UrgencyLevel> for UrgencyLevel {
    fn from(db: pharma_db::entity::common::UrgencyLevel) -> Self {
        UrgencyLevel::from_db_urgency(db)
    }
}

/// Intent enumeration
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    #[default]
    #[serde(rename = "offer")]
    Offer,
    #[serde(rename = "request")]
    Request,
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Intent {
    /// Get string representation for backward compatibility
    pub fn as_str(&self) -> &'static str {
        match self {
            Intent::Offer => "offer",
            Intent::Request => "request",
        }
    }
}

/// A medication entry from the new prompt structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Medication {
    /// Exact medication name from message (preserves original language)
    pub name: String,

    /// Dosage/strength (e.g., "36", "1mg", "150") - can be null
    #[serde(default)]
    pub concentration: Option<String>,

    /// Physical form (امبول, فايل, اقراص, etc.) - can be null
    #[serde(default)]
    pub form: Option<String>,

    /// Expiration date (MM/YY format or description) - can be null
    #[serde(default)]
    pub expiry: Option<String>,

    /// AI confidence score (0.0 to 1.0)
    pub confidence: f64,

    /// Extraction accuracy explanation
    pub reason: String,
}

/// AI parse result - the new structured output schema
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParseResult {
    /// Intent: "offer" or "request"
    pub intent: Intent,

    /// Urgency level: "critical", "urgent", "soon", or "normal"
    pub urgency: UrgencyLevel,

    /// Brief explanation including urgency assessment
    pub reason: String,

    /// List of extracted medications
    pub medications: Vec<Medication>,
}

// =============================================================================
// Legacy Support - ParsedItem for backward compatibility
// =============================================================================

/// A parsed medication item from AI (LEGACY - for backward compatibility)
///
/// This structure is maintained for compatibility with existing code.
/// New code should use ParseResult with Medication entries.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParsedItem {
    /// Item type: "OFFER" or "REQUEST"
    #[serde(rename = "type")]
    pub item_type: Intent,

    /// Canonical English medication name with dosage
    pub medication: String,

    /// Original text from the message
    pub medication_raw: String,

    /// AI confidence score (0.0 to 1.0)
    #[serde(default)]
    pub ai_confidence: f64,

    /// Quantity of items
    #[serde(default)]
    pub quantity: f64,

    /// Unit (boxes, strips, ampoules, etc.)
    #[serde(default)]
    pub unit: Option<String>,

    /// Price per unit
    #[serde(default)]
    pub price: f64,

    /// Maximum price (for requests)
    #[serde(default)]
    pub max_price: f64,

    /// Urgency flag (backward compatible)
    #[serde(default)]
    pub urgent: bool,

    /// Urgency level (more granular)
    #[serde(default)]
    pub urgency_level: UrgencyLevel,

    /// Expiry date if mentioned (YYYY-MM format or description)
    #[serde(default)]
    pub expiry: Option<String>,

    /// Additional notes
    #[serde(default)]
    pub notes: Option<String>,
}

impl ParsedItem {
    /// Convert from new Medication format to legacy ParsedItem
    pub fn from_medication(med: &Medication, intent: Intent, urgency: UrgencyLevel) -> Self {
        // Build medication name with concentration if present
        let medication = if let Some(conc) = &med.concentration {
            format!("{} {}", med.name, conc)
        } else {
            med.name.clone()
        };

        // Build notes from form if present
        let notes = med.form.clone();

        Self {
            item_type: intent,
            medication,
            medication_raw: med.name.clone(),
            ai_confidence: med.confidence,
            quantity: 0.0,
            unit: med.form.clone(),
            price: 0.0,
            max_price: 0.0,
            urgent: urgency.is_urgent(),
            urgency_level: urgency,
            expiry: med.expiry.clone(),
            notes,
        }
    }
}

impl ParseResult {
    /// Convert to legacy format (Vec<ParsedItem>) for backward compatibility
    pub fn to_legacy_items(&self) -> Vec<ParsedItem> {
        self.medications
            .iter()
            .map(|med| ParsedItem::from_medication(med, self.intent, self.urgency))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_schema() {
        let schema = ai_client::generate_schema::<ParseResult>();
        assert!(schema.is_object());
    }

    #[test]
    fn test_intent_serialization() {
        let offer = Intent::Offer;
        let json = serde_json::to_string(&offer).unwrap();
        assert_eq!(json, "\"offer\"");

        let request = Intent::Request;
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "\"request\"");
    }

    #[test]
    fn test_urgency_level_serialization() {
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Normal).unwrap(),
            "\"normal\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Soon).unwrap(),
            "\"soon\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Urgent).unwrap(),
            "\"urgent\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Critical).unwrap(),
            "\"critical\""
        );
    }

    #[test]
    fn test_urgency_level_deserialization() {
        let normal: UrgencyLevel = serde_json::from_str("\"normal\"").unwrap();
        assert_eq!(normal, UrgencyLevel::Normal);

        let urgent: UrgencyLevel = serde_json::from_str("\"urgent\"").unwrap();
        assert_eq!(urgent, UrgencyLevel::Urgent);
    }

    #[test]
    fn test_urgency_level_from_bool() {
        assert_eq!(UrgencyLevel::from_bool(false), UrgencyLevel::Normal);
        assert_eq!(UrgencyLevel::from_bool(true), UrgencyLevel::Urgent);
    }

    #[test]
    fn test_urgency_level_is_urgent() {
        assert!(!UrgencyLevel::Normal.is_urgent());
        assert!(UrgencyLevel::Soon.is_urgent());
        assert!(UrgencyLevel::Urgent.is_urgent());
        assert!(UrgencyLevel::Critical.is_urgent());
    }

    #[test]
    fn test_urgency_level_priority_score() {
        assert_eq!(UrgencyLevel::Normal.priority_score(), 0.0);
        assert_eq!(UrgencyLevel::Soon.priority_score(), 0.3);
        assert_eq!(UrgencyLevel::Urgent.priority_score(), 0.7);
        assert_eq!(UrgencyLevel::Critical.priority_score(), 1.0);
    }

    #[test]
    fn test_urgency_level_display() {
        assert_eq!(format!("{}", UrgencyLevel::Normal), "normal");
        assert_eq!(format!("{}", UrgencyLevel::Critical), "critical");
    }

    #[test]
    fn test_new_parse_result() {
        let json = r#"{
            "intent": "request",
            "urgency": "critical",
            "reason": "Emergency request for insulin",
            "medications": [
                {
                    "name": "Ozempic",
                    "concentration": "1mg",
                    "form": "امبول",
                    "expiry": "10/27",
                    "confidence": 0.95,
                    "reason": "Clear extraction from message"
                }
            ]
        }"#;

        let result: ParseResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.intent, Intent::Request);
        assert_eq!(result.urgency, UrgencyLevel::Critical);
        assert_eq!(result.medications.len(), 1);
        assert_eq!(result.medications[0].name, "Ozempic");
        assert_eq!(result.medications[0].concentration, Some("1mg".to_string()));
    }

    #[test]
    fn test_legacy_conversion() {
        let med = Medication {
            name: "كونسرتا".to_string(),
            concentration: Some("36".to_string()),
            form: Some("اقراص".to_string()),
            expiry: None,
            confidence: 0.9,
            reason: "Extracted correctly".to_string(),
        };

        let item = ParsedItem::from_medication(&med, Intent::Offer, UrgencyLevel::Normal);
        assert_eq!(item.medication, "كونسرتا 36");
        assert_eq!(item.medication_raw, "كونسرتا");
        assert_eq!(item.item_type, Intent::Offer);
        assert!(!item.urgent);
    }

    #[test]
    fn test_parse_result_to_legacy() {
        let result = ParseResult {
            intent: Intent::Request,
            urgency: UrgencyLevel::Urgent,
            reason: "Urgent request".to_string(),
            medications: vec![Medication {
                name: "Aspirin".to_string(),
                concentration: Some("100mg".to_string()),
                form: Some("tablets".to_string()),
                expiry: None,
                confidence: 0.95,
                reason: "Clear".to_string(),
            }],
        };

        let items = result.to_legacy_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].medication, "Aspirin 100mg");
        assert!(items[0].urgent);
        assert_eq!(items[0].urgency_level, UrgencyLevel::Urgent);
    }

    #[test]
    fn test_urgency_level_db_conversion() {
        use pharma_db::entity::common::UrgencyLevel as DbUrgency;

        // AI to DB
        assert!(matches!(
            UrgencyLevel::Normal.to_db_urgency(),
            DbUrgency::Normal
        ));
        assert!(matches!(
            UrgencyLevel::Critical.to_db_urgency(),
            DbUrgency::Critical
        ));

        // DB to AI
        assert_eq!(
            UrgencyLevel::from_db_urgency(DbUrgency::Urgent),
            UrgencyLevel::Urgent
        );

        // Via From trait
        let ai: UrgencyLevel = DbUrgency::Soon.into();
        assert_eq!(ai, UrgencyLevel::Soon);

        let db: DbUrgency = UrgencyLevel::Critical.into();
        assert!(matches!(db, DbUrgency::Critical));
    }
}
