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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UrgencyLevel {
    /// Normal priority - no urgency indicated
    #[default]
    Normal,
    /// Moderate urgency - needed soon
    Soon,
    /// High urgency - needed urgently
    Urgent,
    /// Critical urgency - immediate need
    Critical,
}

impl UrgencyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            UrgencyLevel::Normal => "NORMAL",
            UrgencyLevel::Soon => "SOON",
            UrgencyLevel::Urgent => "URGENT",
            UrgencyLevel::Critical => "CRITICAL",
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

/// A parsed medication item from AI
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

/// Intent enumeration
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Intent {
    #[default]
    Offer,
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
            Intent::Offer => "OFFER",
            Intent::Request => "REQUEST",
        }
    }
}

/// AI parse result - the structured output schema
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ParseResult {
    /// List of parsed medication items
    pub items: Vec<ParsedItem>,
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
    fn test_item_type_serialization() {
        let offer = Intent::Offer;
        let json = serde_json::to_string(&offer).unwrap();
        assert_eq!(json, "\"OFFER\"");

        let request = Intent::Request;
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "\"REQUEST\"");
    }

    #[test]
    fn test_urgency_level_serialization() {
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Normal).unwrap(),
            "\"NORMAL\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Soon).unwrap(),
            "\"SOON\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Urgent).unwrap(),
            "\"URGENT\""
        );
        assert_eq!(
            serde_json::to_string(&UrgencyLevel::Critical).unwrap(),
            "\"CRITICAL\""
        );
    }

    #[test]
    fn test_urgency_level_deserialization() {
        let normal: UrgencyLevel = serde_json::from_str("\"NORMAL\"").unwrap();
        assert_eq!(normal, UrgencyLevel::Normal);

        let urgent: UrgencyLevel = serde_json::from_str("\"URGENT\"").unwrap();
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
        assert_eq!(format!("{}", UrgencyLevel::Normal), "NORMAL");
        assert_eq!(format!("{}", UrgencyLevel::Critical), "CRITICAL");
    }

    #[test]
    fn test_parsed_item_with_urgency() {
        let json = r#"{
            "type": "REQUEST",
            "medication": "Ozempic 1mg",
            "medication_raw": "Ozempic 1mg",
            "ai_confidence": 0.95,
            "urgent": true,
            "urgency_level": "CRITICAL",
            "expiry": "2025-06"
        }"#;

        let item: ParsedItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.item_type, Intent::Request);
        assert!(item.urgent);
        assert_eq!(item.urgency_level, UrgencyLevel::Critical);
        assert_eq!(item.expiry, Some("2025-06".to_string()));
    }

    #[test]
    fn test_parsed_item_defaults() {
        let json = r#"{
            "type": "OFFER",
            "medication": "Aspirin",
            "medication_raw": "اسبرين"
        }"#;

        let item: ParsedItem = serde_json::from_str(json).unwrap();
        assert!(!item.urgent);
        assert_eq!(item.urgency_level, UrgencyLevel::Normal);
        assert_eq!(item.expiry, None);
        assert_eq!(item.quantity, 0.0);
        assert_eq!(item.price, 0.0);
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
