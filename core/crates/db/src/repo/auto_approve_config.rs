//! Auto-approve configuration repository implementation
//!
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::auto_approve_config::{self, Entity as AutoApproveConfig};
use crate::traits::AutoApproveConfigRepository;
use crate::{Error, Result};

/// SeaORM-based auto-approve configuration repository
pub struct SeaOrmAutoApproveConfigRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmAutoApproveConfigRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AutoApproveConfigRepository for SeaOrmAutoApproveConfigRepo {
    /// Get the current configuration (there should only be one row)
    async fn get(&self) -> Result<Option<auto_approve_config::Model>> {
        AutoApproveConfig::find()
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    /// Update the configuration
    /// Requirements: 5.1, 5.4
    async fn update(
        &self,
        config: &auto_approve_config::Model,
    ) -> Result<auto_approve_config::Model> {
        let mut active: auto_approve_config::ActiveModel = config.clone().into();
        active.updated_at = Set(chrono::Utc::now());
        active.update(&*self.db).await.map_err(Error::from)
    }

    /// Create or update the configuration (upsert)
    async fn save(
        &self,
        config: &auto_approve_config::Model,
    ) -> Result<auto_approve_config::Model> {
        // Check if config exists
        let existing = self.get().await?;

        if existing.is_some() {
            self.update(config).await
        } else {
            let active: auto_approve_config::ActiveModel = config.clone().into();
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    /// Get the configuration by ID
    async fn get_by_id(&self, id: Uuid) -> Result<Option<auto_approve_config::Model>> {
        AutoApproveConfig::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    /// Check if auto-approval is enabled
    /// Requirements: 5.2
    async fn is_enabled(&self) -> Result<bool> {
        let config = self.get().await?;
        Ok(config.map(|c| c.enabled).unwrap_or(false))
    }

    /// Enable or disable auto-approval
    /// Requirements: 5.2
    async fn set_enabled(&self, enabled: bool, user_id: Option<Uuid>) -> Result<()> {
        let config = self.get().await?;
        if let Some(mut config) = config {
            config.enabled = enabled;
            config.updated_by = user_id;
            self.update(&config).await?;
        }
        Ok(())
    }

    /// Update the confidence threshold
    /// Requirements: 5.1
    async fn set_confidence_threshold(&self, threshold: f64, user_id: Option<Uuid>) -> Result<()> {
        let config = self.get().await?;
        if let Some(mut config) = config {
            config.confidence_threshold = threshold;
            config.updated_by = user_id;
            self.update(&config).await?;
        }
        Ok(())
    }

    /// Update category-specific thresholds
    /// Requirements: 5.3
    async fn set_category_thresholds(
        &self,
        thresholds: &std::collections::HashMap<String, f64>,
        user_id: Option<Uuid>,
    ) -> Result<()> {
        let config = self.get().await?;
        if let Some(mut config) = config {
            config.set_category_thresholds(thresholds);
            config.updated_by = user_id;
            self.update(&config).await?;
        }
        Ok(())
    }

    /// Update the schedule
    /// Requirements: 5.5
    async fn set_schedule(&self, schedule: Option<String>, user_id: Option<Uuid>) -> Result<()> {
        let config = self.get().await?;
        if let Some(mut config) = config {
            config.schedule = schedule;
            config.updated_by = user_id;
            self.update(&config).await?;
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::TestDb;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_get_config() {
        let db = TestDb::new().await;
        let repo = SeaOrmAutoApproveConfigRepo::new(db.db.clone());

        // The migration should have inserted a default config
        let config = repo.get().await.expect("Get config");
        assert!(config.is_some(), "Should have default config");

        let config = config.unwrap();
        assert!(!config.enabled, "Default should be disabled");
        assert!((config.confidence_threshold - 0.85).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_update_config() {
        let db = TestDb::new().await;
        let repo = SeaOrmAutoApproveConfigRepo::new(db.db.clone());

        let mut config = repo
            .get()
            .await
            .expect("Get config")
            .expect("Config exists");
        config.enabled = true;
        config.confidence_threshold = 0.90;

        let updated = repo.update(&config).await.expect("Update config");
        assert!(updated.enabled);
        assert!((updated.confidence_threshold - 0.90).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let db = TestDb::new().await;
        let repo = SeaOrmAutoApproveConfigRepo::new(db.db.clone());

        repo.set_enabled(true, None).await.expect("Set enabled");
        assert!(repo.is_enabled().await.expect("Is enabled"));

        repo.set_enabled(false, None).await.expect("Set disabled");
        assert!(!repo.is_enabled().await.expect("Is disabled"));
    }

    #[tokio::test]
    async fn test_set_category_thresholds() {
        let db = TestDb::new().await;
        let repo = SeaOrmAutoApproveConfigRepo::new(db.db.clone());

        let mut thresholds = HashMap::new();
        thresholds.insert("controlled".to_string(), 0.95);
        thresholds.insert("generic".to_string(), 0.80);

        repo.set_category_thresholds(&thresholds, None)
            .await
            .expect("Set thresholds");

        let config = repo
            .get()
            .await
            .expect("Get config")
            .expect("Config exists");
        let retrieved = config.get_category_thresholds();

        assert!((retrieved.get("controlled").unwrap() - 0.95).abs() < 0.001);
        assert!((retrieved.get("generic").unwrap() - 0.80).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_set_schedule() {
        let db = TestDb::new().await;
        let repo = SeaOrmAutoApproveConfigRepo::new(db.db.clone());

        repo.set_schedule(Some("0 9-17 * * 1-5".to_string()), None)
            .await
            .expect("Set schedule");

        let config = repo
            .get()
            .await
            .expect("Get config")
            .expect("Config exists");
        assert_eq!(config.schedule, Some("0 9-17 * * 1-5".to_string()));

        repo.set_schedule(None, None).await.expect("Clear schedule");
        let config = repo
            .get()
            .await
            .expect("Get config")
            .expect("Config exists");
        assert!(config.schedule.is_none());
    }
}
