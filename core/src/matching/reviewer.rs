//! AI Reviewer for Match Auditing
//!
//! Uses LLM-based reasoning to audit high-confidence matches and flag potential risks.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::domain::{Offer, Request};
use ai_client::Client as AIClient;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Match looks perfect
    Approved,
    /// Match is likely correct but has a minor discrepancy (e.g. dosage)
    Flagged,
    /// Match is likely incorrect or dangerous
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewResult {
    pub status: ReviewStatus,
    pub confidence: f32,
    pub explanation: String,
    pub suggested_action: Option<String>,
}

/// AI-powered expert reviewer for matches
pub struct AIReviewer {
    client: Arc<AIClient>,
}

impl AIReviewer {
    pub fn new(client: Arc<AIClient>) -> Self {
        Self { client }
    }

    /// Audit a match between an offer and a request
    pub async fn audit_match(
        &self,
        offer: &Offer,
        request: &Request,
        score: f64,
        reasoning: &str,
    ) -> Result<ReviewResult, ai_client::Error> {
        let system_prompt = "You are an expert pharmaceutical data auditor. \
            Your job is to review potential matches between drug offers and requests. \
            Look for discrepancies in medication names, dosages, or quantities. \
            Be strict about safety and medical accuracy.";

        let user_prompt = format!(
            "Review this match:\n\
            OFFER:\n- Product: {}\n- Raw: {}\n\n\
            REQUEST:\n- Product: {}\n- Raw: {}\n\n\
            SCORING ENGINE RESULT:\n- Score: {:.1}%\n- Reasoning: {}\n\n\
            Provide a status (approved, flagged, or rejected) and a brief expert explanation.",
            offer.medication,
            offer.medication_raw,
            request.medication,
            request.medication_raw,
            score * 100.0,
            reasoning
        );

        self.client
            .generate_object_with_system::<ReviewResult>(system_prompt, &user_prompt)
            .await
    }
}
