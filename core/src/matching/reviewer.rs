//! AI Reviewer for Match Auditing
//!
//! Uses LLM-based reasoning to audit medication name matches.
//! Focuses strictly on name comparison without providing medical information.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::{Offer, Request};
use ai_client::Client as AIClient;

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
    pub suggested_action: Option<String>,
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
- Ignore dosage numbers when comparing names

OUTPUT:
- approved: Names clearly match (same medication name)
- flagged: Names are similar but uncertain (possible typo or variant)
- rejected: Names are clearly different (different medication names)

Keep explanations brief and focused ONLY on name comparison."#;

        let user_prompt = format!(
            "Compare these medication NAMES only:\n\n\
            OFFER NAME: {}\n\
            OFFER RAW (Arabic): {}\n\n\
            REQUEST NAME: {}\n\
            REQUEST RAW (Arabic): {}\n\n\
            Scoring engine score: {:.1}%\n\n\
            Are these the SAME medication name? Compare names only, do not provide medical information.",
            offer.medication,
            offer.medication_raw,
            request.medication,
            request.medication_raw,
            score * 100.0
        );

        self.client
            .generate_object_with_system::<ReviewResult>(system_prompt, &user_prompt)
            .await
    }
}
