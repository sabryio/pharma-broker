//! Domain types and enums
//!
//! Ported from legacy/domain/entity/entity.go:8-51

use serde::{Deserialize, Serialize};

/// Categorizes incoming WhatsApp messages
/// Ported from Go: MessageType (entity.go:8)
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Offer,
    Request,
    Both,
    #[default]
    Unknown,
}

/// Operator's decision on a match
/// Ported from Go: FeedbackDecision (entity.go:46)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackDecision {
    Confirmed,
    Rejected,
}

/// Confidence bands for matching
/// Ported from Go: matching/interface.go
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceBand {
    Auto,    // >= 0.90
    Suggest, // 0.70 - 0.89
    Review,  // 0.50 - 0.69
    None,    // < 0.50
}

impl ConfidenceBand {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s >= 0.90 => Self::Auto,
            s if s >= 0.70 => Self::Suggest,
            s if s >= 0.50 => Self::Review,
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_band_from_score() {
        assert_eq!(ConfidenceBand::from_score(1.0), ConfidenceBand::Auto);
        assert_eq!(ConfidenceBand::from_score(0.95), ConfidenceBand::Auto);
        assert_eq!(ConfidenceBand::from_score(0.90), ConfidenceBand::Auto);
        assert_eq!(ConfidenceBand::from_score(0.89), ConfidenceBand::Suggest);
        assert_eq!(ConfidenceBand::from_score(0.70), ConfidenceBand::Suggest);
        assert_eq!(ConfidenceBand::from_score(0.69), ConfidenceBand::Review);
        assert_eq!(ConfidenceBand::from_score(0.50), ConfidenceBand::Review);
        assert_eq!(ConfidenceBand::from_score(0.49), ConfidenceBand::None);
        assert_eq!(ConfidenceBand::from_score(0.0), ConfidenceBand::None);
    }
}
