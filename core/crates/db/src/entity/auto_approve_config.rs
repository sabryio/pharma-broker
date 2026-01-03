//! AutoApproveConfig entity - Configuration for AI auto-approval system
//!
//! Stores configuration settings for the AI supervised auto-approval feature.
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auto_approve_config")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    /// Whether auto-approval is enabled globally
    pub enabled: bool,
    /// Minimum AI confidence for auto-approval (0.70-0.99)
    pub confidence_threshold: f64,
    /// Maximum batch size per processing cycle
    pub batch_size: i32,
    /// Processing interval in seconds
    pub processing_interval_secs: i32,
    /// Undo window in minutes
    pub undo_window_mins: i32,
    /// Override rate threshold to pause (0.0-1.0)
    pub override_rate_pause_threshold: f64,
    /// Consecutive overrides to disable
    pub consecutive_override_limit: i32,
    /// Cooldown period after override (minutes)
    pub override_cooldown_mins: i32,
    /// Category-specific threshold overrides (JSON)
    #[sea_orm(column_type = "JsonBinary")]
    pub category_thresholds: serde_json::Value,
    /// Schedule for auto-approval (cron expression)
    pub schedule: Option<String>,
    /// Last update timestamp
    pub updated_at: DateTimeUtc,
    /// User who last updated the config
    pub updated_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new config with default values
    pub fn default_config() -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: false,
            confidence_threshold: 0.85,
            batch_size: 50,
            processing_interval_secs: 30,
            undo_window_mins: 30,
            override_rate_pause_threshold: 0.10,
            consecutive_override_limit: 5,
            override_cooldown_mins: 60,
            category_thresholds: serde_json::json!({}),
            schedule: None,
            updated_at: chrono::Utc::now(),
            updated_by: None,
        }
    }

    /// Get category thresholds as a HashMap
    pub fn get_category_thresholds(&self) -> HashMap<String, f64> {
        serde_json::from_value(self.category_thresholds.clone()).unwrap_or_default()
    }

    /// Set category thresholds from a HashMap
    pub fn set_category_thresholds(&mut self, thresholds: &HashMap<String, f64>) {
        self.category_thresholds =
            serde_json::to_value(thresholds).unwrap_or(serde_json::json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Model::default_config();
        assert!(!config.enabled);
        assert!((config.confidence_threshold - 0.85).abs() < 0.001);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.processing_interval_secs, 30);
        assert_eq!(config.undo_window_mins, 30);
        assert!((config.override_rate_pause_threshold - 0.10).abs() < 0.001);
        assert_eq!(config.consecutive_override_limit, 5);
        assert_eq!(config.override_cooldown_mins, 60);
        assert!(config.schedule.is_none());
    }

    #[test]
    fn test_category_thresholds() {
        let mut config = Model::default_config();
        let mut thresholds = HashMap::new();
        thresholds.insert("controlled".to_string(), 0.95);
        thresholds.insert("generic".to_string(), 0.80);

        config.set_category_thresholds(&thresholds);
        let retrieved = config.get_category_thresholds();

        assert!((retrieved.get("controlled").unwrap() - 0.95).abs() < 0.001);
        assert!((retrieved.get("generic").unwrap() - 0.80).abs() < 0.001);
    }
}
