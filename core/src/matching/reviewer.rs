//! AI Reviewer for Match Auditing
//!
//! Uses LLM-based reasoning to audit medication name matches.
//! Focuses strictly on name comparison without providing medical information.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::{Offer, Request};
use ai_client::{AIContext, Client as AIClient};

// =============================================================================
// Match Detail Types for Structured AI Analysis
// =============================================================================

/// Detailed comparison of a single field (brand name, Arabic name, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct MatchField {
    /// Value from the offer
    pub offer_value: String,
    /// Value from the request
    pub request_value: String,
    /// Whether the fields match
    pub matches: bool,
    /// Type of match: "exact", "transliteration", "fuzzy", "partial", "no_match"
    pub match_type: String,
    /// Similarity score (0.0 - 1.0)
    #[serde(default)]
    pub similarity: f32,
}

/// Dosage comparison details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct DosageComparison {
    /// Dosage from the offer (e.g., "10.8mg")
    pub offer_dosage: Option<String>,
    /// Dosage from the request (e.g., "3.6mg")
    pub request_dosage: Option<String>,
    /// Whether dosages match
    pub matches: bool,
    /// Whether dosage difference is being ignored per matching rules
    pub ignored: bool,
    /// Explanation note
    #[serde(default)]
    pub note: String,
}

/// Structured match details providing granular analysis breakdown
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct MatchDetails {
    /// Brand name comparison result
    pub brand_match: MatchField,
    /// Arabic/transliteration name comparison
    pub arabic_match: MatchField,
    /// Dosage comparison (may be ignored per rules)
    pub dosage: DosageComparison,
    /// Generic/active ingredient match (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_match: Option<MatchField>,
    /// Key differences found between offer and request
    #[serde(default)]
    pub differences: Vec<String>,
    /// Reasons supporting the final decision
    #[serde(default)]
    pub decision_reasons: Vec<String>,
}

// =============================================================================
// Review Status and Result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Names match - same medication
    Approved,
    /// Names are similar but have minor differences
    Flagged,
    /// Names are different - not the same medication
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewResult {
    pub status: ReviewStatus,
    pub confidence: f32,
    pub explanation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
    /// Detailed match analysis (populated by enhanced AI analysis)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_details: Option<MatchDetails>,
}

/// AI-powered medication name matcher
pub struct AIReviewer {
    client: Arc<AIClient>,
}

impl AIReviewer {
    pub fn new(client: Arc<AIClient>) -> Self {
        Self { client }
    }

    /// Audit a match between an offer and a request
    /// Focuses ONLY on medication name comparison
    /// Uses AIContext::Comparison for zero temperature (deterministic output)
    pub async fn audit_match(
        &self,
        offer: &Offer,
        request: &Request,
        score: f64,
        _reasoning: &str,
    ) -> Result<ReviewResult, ai_client::Error> {
        let system_prompt = r#"You are a medication NAME MATCHER. Your ONLY job is to compare medication names.

RULES:
1. Compare ONLY the medication names provided
2. Do NOT provide any medical information, drug details, or therapeutic uses
3. Do NOT explain what the medications are or what they do
4. Focus ONLY on whether the names refer to the same product

COMPARISON CRITERIA:
- Are the names the same brand/product? (exact or transliteration match)
- Are the Arabic names (Raw) equivalent transliterations?
- Ignore dosage numbers when comparing names (note them as "ignored" in dosage comparison)

OUTPUT FORMAT:
You MUST provide a JSON response with detailed match analysis:
- status: "approved" (names match), "flagged" (uncertain), or "rejected" (different)
- confidence: 0.0 to 1.0
- explanation: Brief summary of name comparison result
- match_details: Structured analysis with:
  - brand_match: Compare brand/product names with match_type ("exact", "transliteration", "fuzzy", "no_match")
  - arabic_match: Compare Arabic/raw names with match_type
  - dosage: Compare dosages, set ignored=true if dosage differs but names match
  - differences: List key differences found (if any)
  - decision_reasons: List reasons for your decision

Keep explanations focused ONLY on name comparison."#;

        let user_prompt = format!(
            "Compare these medication NAMES only:\n\n\
            OFFER:\n  Brand Name: {}\n  Arabic/Raw: {}\n\n\
            REQUEST:\n  Brand Name: {}\n  Arabic/Raw: {}\n\n\
            Scoring engine score: {:.1}%\n\n\
            Provide detailed match analysis in the specified JSON format.",
            offer.medication,
            offer.medication_raw,
            request.medication,
            request.medication_raw,
            score * 100.0
        );

        // Use AIContext::Comparison for zero temperature (deterministic, exact matching)
        self.client
            .generate_object_with_context::<ReviewResult>(
                system_prompt,
                &user_prompt,
                AIContext::Comparison,
            )
            .await
    }
}
