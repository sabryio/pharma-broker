//! Common types shared across entities
//!
//! This module contains types that are used by multiple entities to avoid duplication.
//! All shared enums and types should be defined here and re-exported from entity modules.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

// =============================================================================
// UrgencyLevel - Canonical Definition
// =============================================================================

/// Urgency level for medication requests/offers
///
/// This is the single source of truth for urgency levels across the application.
/// Used by both AI parsing (with JsonSchema) and database entities (with SeaORM).
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UrgencyLevel {
    /// Normal priority - no urgency indicated
    #[default]
    #[sea_orm(string_value = "NORMAL")]
    Normal,
    /// Moderate urgency - needed soon
    #[sea_orm(string_value = "SOON")]
    Soon,
    /// High urgency - needed urgently
    #[sea_orm(string_value = "URGENT")]
    Urgent,
    /// Critical urgency - immediate need
    #[sea_orm(string_value = "CRITICAL")]
    Critical,
}

impl UrgencyLevel {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Soon => "SOON",
            Self::Urgent => "URGENT",
            Self::Critical => "CRITICAL",
        }
    }

    /// Convert from boolean urgent flag (backward compatibility)
    pub fn from_bool(urgent: bool) -> Self {
        if urgent { Self::Urgent } else { Self::Normal }
    }

    /// Check if this is any level of urgency
    pub fn is_urgent(&self) -> bool {
        !matches!(self, Self::Normal)
    }

    /// Get priority score (0.0 = normal, 1.0 = critical)
    pub fn priority_score(&self) -> f64 {
        match self {
            Self::Normal => 0.0,
            Self::Soon => 0.3,
            Self::Urgent => 0.7,
            Self::Critical => 1.0,
        }
    }

    /// Parse from string (case-insensitive)
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NORMAL" => Some(Self::Normal),
            "SOON" => Some(Self::Soon),
            "URGENT" => Some(Self::Urgent),
            "CRITICAL" => Some(Self::Critical),
            _ => None,
        }
    }
}

impl fmt::Display for UrgencyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// ItemStatus - Canonical Definition
// =============================================================================

/// Status for offers and requests
///
/// Shared between offers and requests tables.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemStatus {
    /// Active and available for matching
    #[default]
    #[sea_orm(string_value = "ACTIVE")]
    Active,
    /// Successfully matched
    #[sea_orm(string_value = "MATCHED")]
    Matched,
    /// Expired due to time
    #[sea_orm(string_value = "EXPIRED")]
    Expired,
    /// Cancelled by user
    #[sea_orm(string_value = "CANCELLED")]
    Cancelled,
    /// Marked as duplicate
    #[sea_orm(string_value = "DUPLICATE")]
    Duplicate,
}

impl ItemStatus {
    /// Check if the item is still active
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if the item has been resolved (matched, expired, etc.)
    pub fn is_resolved(&self) -> bool {
        !self.is_active()
    }
}

impl fmt::Display for ItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "ACTIVE"),
            Self::Matched => write!(f, "MATCHED"),
            Self::Expired => write!(f, "EXPIRED"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::Duplicate => write!(f, "DUPLICATE"),
        }
    }
}

// =============================================================================
// MatchStatus - Canonical Definition
// =============================================================================

/// Status for matches
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchStatus {
    /// Pending review
    #[default]
    #[sea_orm(string_value = "PENDING")]
    Pending,
    /// Confirmed by operator
    #[sea_orm(string_value = "CONFIRMED")]
    Confirmed,
    /// Rejected by operator
    #[sea_orm(string_value = "REJECTED")]
    Rejected,
    /// Expired without action
    #[sea_orm(string_value = "EXPIRED")]
    Expired,
}

impl MatchStatus {
    /// Check if the match is still pending
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Check if the match was successful
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

impl fmt::Display for MatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Confirmed => write!(f, "CONFIRMED"),
            Self::Rejected => write!(f, "REJECTED"),
            Self::Expired => write!(f, "EXPIRED"),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_urgency_level_from_str_loose() {
        assert_eq!(
            UrgencyLevel::from_str_loose("normal"),
            Some(UrgencyLevel::Normal)
        );
        assert_eq!(
            UrgencyLevel::from_str_loose("URGENT"),
            Some(UrgencyLevel::Urgent)
        );
        assert_eq!(UrgencyLevel::from_str_loose("invalid"), None);
    }

    #[test]
    fn test_item_status_is_active() {
        assert!(ItemStatus::Active.is_active());
        assert!(!ItemStatus::Matched.is_active());
        assert!(!ItemStatus::Expired.is_active());
    }

    #[test]
    fn test_match_status_is_pending() {
        assert!(MatchStatus::Pending.is_pending());
        assert!(!MatchStatus::Confirmed.is_pending());
    }
}
