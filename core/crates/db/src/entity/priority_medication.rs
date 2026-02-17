//! PriorityMedication entity - Priority medications for fast-track processing
//!
//! Medications marked as priority will be:
//! - Parsed with higher priority in the queue
//! - Matched faster in the matching engine
//! - Potentially trigger immediate notifications

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Priority level for medications
#[derive(
    Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Default, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorityLevel {
    /// Low priority (priority score: 1)
    #[sea_orm(num_value = 1)]
    Low = 1,

    /// Normal priority (priority score: 3) - Default
    #[default]
    #[sea_orm(num_value = 3)]
    Normal = 3,

    /// High priority (priority score: 5)
    #[sea_orm(num_value = 5)]
    High = 5,

    /// Urgent priority (priority score: 8)
    #[sea_orm(num_value = 8)]
    Urgent = 8,

    /// Critical priority (priority score: 10) - Highest
    #[sea_orm(num_value = 10)]
    Critical = 10,
}

impl PriorityLevel {
    /// Get numeric priority score
    pub fn score(&self) -> i32 {
        match self {
            PriorityLevel::Low => 1,
            PriorityLevel::Normal => 3,
            PriorityLevel::High => 5,
            PriorityLevel::Urgent => 8,
            PriorityLevel::Critical => 10,
        }
    }

    /// Check if this is a high priority level (>= High)
    pub fn is_high_priority(&self) -> bool {
        self.score() >= 5
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "priority_medications")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Medication name (normalized for matching)
    #[sea_orm(unique)]
    pub medication_name: String,

    /// Optional Arabic name
    pub medication_name_ar: Option<String>,

    /// Priority level (1-10)
    pub priority_level: PriorityLevel,

    /// Reason for priority (e.g., "High demand", "Critical medication", "Seasonal")
    pub reason: Option<String>,

    /// Whether this priority is currently active
    pub active: bool,

    /// When this priority becomes active
    pub active_from: DateTimeUtc,

    /// When this priority expires (None = no expiration)
    pub active_until: Option<DateTimeUtc>,

    /// User who created this priority
    pub created_by: Option<Uuid>,

    /// Metadata
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new priority medication
    pub fn new(medication_name: impl Into<String>, priority_level: PriorityLevel) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            medication_name: medication_name.into(),
            medication_name_ar: None,
            priority_level,
            reason: None,
            active: true,
            active_from: now,
            active_until: None,
            created_by: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set Arabic name
    pub fn with_arabic_name(mut self, name: impl Into<String>) -> Self {
        self.medication_name_ar = Some(name.into());
        self
    }

    /// Set reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set expiration date
    pub fn with_expiration(mut self, until: DateTimeUtc) -> Self {
        self.active_until = Some(until);
        self
    }

    /// Set created by user
    pub fn with_created_by(mut self, user_id: Uuid) -> Self {
        self.created_by = Some(user_id);
        self
    }

    /// Check if this priority is currently active
    pub fn is_currently_active(&self) -> bool {
        if !self.active {
            return false;
        }

        let now = chrono::Utc::now();

        // Check if we're past the start date
        if now < self.active_from {
            return false;
        }

        // Check if we're before the expiration date (if set)
        if let Some(until) = self.active_until
            && now > until
        {
            return false;
        }

        true
    }

    /// Get priority score
    pub fn priority_score(&self) -> i32 {
        if self.is_currently_active() {
            self.priority_level.score()
        } else {
            0 // Inactive priorities have no score
        }
    }

    /// Check if medication name matches (case-insensitive, normalized)
    pub fn matches_medication(&self, medication: &str) -> bool {
        let normalized_input = normalize_medication_name(medication);
        let normalized_self = normalize_medication_name(&self.medication_name);

        normalized_self == normalized_input
            || self
                .medication_name_ar
                .as_ref()
                .map(|ar| normalize_medication_name(ar) == normalized_input)
                .unwrap_or(false)
    }
}

/// Normalize medication name for matching
/// - Lowercase
/// - Trim whitespace
/// - Remove extra spaces
fn normalize_medication_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_level_score() {
        assert_eq!(PriorityLevel::Low.score(), 1);
        assert_eq!(PriorityLevel::Normal.score(), 3);
        assert_eq!(PriorityLevel::High.score(), 5);
        assert_eq!(PriorityLevel::Urgent.score(), 8);
        assert_eq!(PriorityLevel::Critical.score(), 10);
    }

    #[test]
    fn test_is_high_priority() {
        assert!(!PriorityLevel::Low.is_high_priority());
        assert!(!PriorityLevel::Normal.is_high_priority());
        assert!(PriorityLevel::High.is_high_priority());
        assert!(PriorityLevel::Urgent.is_high_priority());
        assert!(PriorityLevel::Critical.is_high_priority());
    }

    #[test]
    fn test_is_currently_active() {
        let now = chrono::Utc::now();

        // Active priority with no expiration
        let priority = Model::new("Insulin", PriorityLevel::Critical);
        assert!(priority.is_currently_active());

        // Inactive priority
        let mut priority = Model::new("Aspirin", PriorityLevel::High);
        priority.active = false;
        assert!(!priority.is_currently_active());

        // Expired priority
        let mut priority = Model::new("Paracetamol", PriorityLevel::High);
        priority.active_until = Some(now - chrono::Duration::days(1));
        assert!(!priority.is_currently_active());

        // Future priority
        let mut priority = Model::new("Vaccine", PriorityLevel::Urgent);
        priority.active_from = now + chrono::Duration::days(1);
        assert!(!priority.is_currently_active());
    }

    #[test]
    fn test_matches_medication() {
        let priority = Model::new("Insulin", PriorityLevel::Critical).with_arabic_name("انسولين");

        assert!(priority.matches_medication("Insulin"));
        assert!(priority.matches_medication("insulin"));
        assert!(priority.matches_medication("  INSULIN  "));
        assert!(priority.matches_medication("انسولين"));
        assert!(!priority.matches_medication("Aspirin"));
    }

    #[test]
    fn test_normalize_medication_name() {
        assert_eq!(normalize_medication_name("Insulin"), "insulin");
        assert_eq!(normalize_medication_name("  INSULIN  "), "insulin");
        assert_eq!(normalize_medication_name("Insulin  100mg"), "insulin 100mg");
    }
}
