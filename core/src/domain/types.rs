//! Domain types and enums
//!
//! Ported from legacy/domain/entity/entity.go:8-51

use serde::{Deserialize, Serialize};

/// Categorizes incoming WhatsApp messages
/// Ported from Go: MessageType (entity.go:8)
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageType {
    Offer,
    Request,
    Both,
    #[default]
    Unknown,
}

/// Tracks lifecycle of offers/requests
/// Ported from Go: ItemStatus (entity.go:18)
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemStatus {
    #[default]
    Active,
    Matched,
    Expired,
    Archived,
    Duplicate,
}

impl std::fmt::Display for ItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "ACTIVE"),
            Self::Matched => write!(f, "MATCHED"),
            Self::Expired => write!(f, "EXPIRED"),
            Self::Archived => write!(f, "ARCHIVED"),
            Self::Duplicate => write!(f, "DUPLICATE"),
        }
    }
}

/// Tracks lifecycle of matches
/// Ported from Go: MatchStatus (entity.go:28)
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchStatus {
    #[default]
    Pending,
    Confirmed,
    Rejected,
}

impl std::fmt::Display for MatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Confirmed => write!(f, "CONFIRMED"),
            Self::Rejected => write!(f, "REJECTED"),
        }
    }
}

/// Review status for low-confidence parses
/// Ported from Go: ReviewStatus (entity.go:37)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewStatus {
    Pending,
    Approved,
    Rejected,
}

/// Operator's decision on a match
/// Ported from Go: FeedbackDecision (entity.go:46)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "SCREAMING_SNAKE_CASE")]
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
