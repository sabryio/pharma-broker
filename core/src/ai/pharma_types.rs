//! Pharma-specific AI types with JSON Schema support
//!
//! These types derive `schemars::JsonSchema` for structured output via the AI client.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A parsed medication item from AI
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParsedItem {
    /// Item type: "OFFER" or "REQUEST"
    #[serde(rename = "type")]
    pub item_type: ItemType,

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

    /// Urgency flag
    #[serde(default)]
    pub urgent: bool,

    /// Additional notes
    #[serde(default)]
    pub notes: Option<String>,
}

/// Item type enumeration
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ItemType {
    #[default]
    Offer,
    Request,
}

impl fmt::Display for ItemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ItemType {
    /// Get string representation for backward compatibility
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemType::Offer => "OFFER",
            ItemType::Request => "REQUEST",
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
        let offer = ItemType::Offer;
        let json = serde_json::to_string(&offer).unwrap();
        assert_eq!(json, "\"OFFER\"");

        let request = ItemType::Request;
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "\"REQUEST\"");
    }
}
