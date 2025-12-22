//! Parameter structs for feedback-related operations
//!
//! These structs consolidate multiple function arguments into descriptive,
//! self-documenting parameter objects.

use serde::{Deserialize, Serialize};

// =============================================================================
// FeedbackScores
// =============================================================================

/// Score breakdown for feedback records
///
/// Groups all individual scores together for cleaner API signatures.
/// Can be constructed from a MatchScore or manually.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedbackScores {
    /// Medication name similarity score (0.0 - 1.0)
    pub medication: f64,
    /// Dosage match score (0.0 - 1.0)
    pub dosage: f64,
    /// Quantity fulfillment score (0.0 - 1.0)
    pub quantity: f64,
    /// Price compatibility score (0.0 - 1.0)
    pub price: f64,
    /// Recency/freshness score (0.0 - 1.0)
    pub recency: f64,
    /// Total weighted score (0.0 - 1.0)
    pub total: f64,
}

impl FeedbackScores {
    /// Create a new FeedbackScores with all values
    pub fn new(
        medication: f64,
        dosage: f64,
        quantity: f64,
        price: f64,
        recency: f64,
        total: f64,
    ) -> Self {
        Self {
            medication,
            dosage,
            quantity,
            price,
            recency,
            total,
        }
    }

    /// Create from total score with estimated component scores
    ///
    /// Useful when only the total score is known and component scores
    /// need to be estimated for backward compatibility.
    pub fn from_total_estimated(total: f64) -> Self {
        Self {
            medication: total * 0.9,
            dosage: total * 0.8,
            quantity: total * 0.85,
            price: total * 0.95,
            recency: 0.7,
            total,
        }
    }

    /// Validate that all scores are in valid range
    pub fn is_valid(&self) -> bool {
        let in_range = |v: f64| (0.0..=1.0).contains(&v);
        in_range(self.medication)
            && in_range(self.dosage)
            && in_range(self.quantity)
            && in_range(self.price)
            && in_range(self.recency)
            && in_range(self.total)
    }

    /// Clamp all scores to valid range
    pub fn clamped(self) -> Self {
        Self {
            medication: self.medication.clamp(0.0, 1.0),
            dosage: self.dosage.clamp(0.0, 1.0),
            quantity: self.quantity.clamp(0.0, 1.0),
            price: self.price.clamp(0.0, 1.0),
            recency: self.recency.clamp(0.0, 1.0),
            total: self.total.clamp(0.0, 1.0),
        }
    }
}

// =============================================================================
// CreateFeedbackParams
// =============================================================================

/// Parameters for creating a feedback record
///
/// Consolidates all parameters needed to create a FeedbackRecord.
#[derive(Debug, Clone)]
pub struct CreateFeedbackParams {
    /// The match ID this feedback is for
    pub match_id: String,
    /// The user who provided the feedback
    pub user_id: String,
    /// Whether the match was confirmed (true) or rejected (false)
    pub confirmed: bool,
    /// Score breakdown
    pub scores: FeedbackScores,
}

impl CreateFeedbackParams {
    /// Create new feedback parameters
    pub fn new(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        confirmed: bool,
        scores: FeedbackScores,
    ) -> Self {
        Self {
            match_id: match_id.into(),
            user_id: user_id.into(),
            confirmed,
            scores,
        }
    }

    /// Create confirmation feedback with estimated scores
    pub fn confirmed(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        total_score: f64,
    ) -> Self {
        Self {
            match_id: match_id.into(),
            user_id: user_id.into(),
            confirmed: true,
            scores: FeedbackScores::from_total_estimated(total_score),
        }
    }

    /// Create rejection feedback with estimated scores
    pub fn rejected(
        match_id: impl Into<String>,
        user_id: impl Into<String>,
        total_score: f64,
    ) -> Self {
        Self {
            match_id: match_id.into(),
            user_id: user_id.into(),
            confirmed: false,
            scores: FeedbackScores::from_total_estimated(total_score),
        }
    }
}

// =============================================================================
// RecordFeedbackParams
// =============================================================================

/// Parameters for recording feedback with medication information
///
/// Used by the matching engine to record feedback with historical learning data.
#[derive(Debug, Clone)]
pub struct RecordFeedbackParams<'a> {
    /// User who provided the feedback
    pub user_id: &'a str,
    /// Whether the match was confirmed
    pub confirmed: bool,
    /// Total match score
    pub total_score: f64,
    /// Offer medication name (for historical learning)
    pub offer_medication: Option<&'a str>,
    /// Request medication name (for historical learning)
    pub request_medication: Option<&'a str>,
}

impl<'a> RecordFeedbackParams<'a> {
    /// Create basic feedback params without medication info
    pub fn basic(user_id: &'a str, confirmed: bool, total_score: f64) -> Self {
        Self {
            user_id,
            confirmed,
            total_score,
            offer_medication: None,
            request_medication: None,
        }
    }

    /// Create feedback params with medication info for historical learning
    pub fn with_medications(
        user_id: &'a str,
        confirmed: bool,
        total_score: f64,
        offer_medication: &'a str,
        request_medication: &'a str,
    ) -> Self {
        Self {
            user_id,
            confirmed,
            total_score,
            offer_medication: Some(offer_medication),
            request_medication: Some(request_medication),
        }
    }

    /// Check if this has medication info for historical learning
    pub fn has_medication_info(&self) -> bool {
        self.offer_medication.is_some() && self.request_medication.is_some()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_scores_new() {
        let scores = FeedbackScores::new(0.9, 0.8, 0.85, 0.95, 0.7, 0.88);
        assert!((scores.medication - 0.9).abs() < 0.001);
        assert!((scores.total - 0.88).abs() < 0.001);
    }

    #[test]
    fn test_feedback_scores_from_total() {
        let scores = FeedbackScores::from_total_estimated(0.85);
        assert!((scores.total - 0.85).abs() < 0.001);
        assert!(scores.medication > 0.0);
    }

    #[test]
    fn test_feedback_scores_validation() {
        let valid = FeedbackScores::new(0.9, 0.8, 0.85, 0.95, 0.7, 0.88);
        assert!(valid.is_valid());

        let invalid = FeedbackScores::new(1.5, 0.8, 0.85, 0.95, 0.7, 0.88);
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_feedback_scores_clamped() {
        let scores = FeedbackScores::new(1.5, -0.1, 0.85, 0.95, 0.7, 0.88);
        let clamped = scores.clamped();
        assert!((clamped.medication - 1.0).abs() < 0.001);
        assert!((clamped.dosage - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_create_feedback_params() {
        let params = CreateFeedbackParams::confirmed("match-123", "user-456", 0.85);
        assert_eq!(params.match_id, "match-123");
        assert_eq!(params.user_id, "user-456");
        assert!(params.confirmed);
    }

    #[test]
    fn test_record_feedback_params() {
        let params = RecordFeedbackParams::with_medications(
            "user-1",
            true,
            0.9,
            "Aspirin 100mg",
            "Aspirin 100mg",
        );
        assert!(params.has_medication_info());

        let basic = RecordFeedbackParams::basic("user-1", false, 0.5);
        assert!(!basic.has_medication_info());
    }
}
