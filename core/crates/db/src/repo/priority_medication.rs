//! PriorityMedication repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::priority_medication::{self, Entity as PriorityMedication};
use crate::traits::PriorityMedicationRepository;
use crate::{Error, Result};

/// SeaORM-based priority medication repository
pub struct SeaOrmPriorityMedicationRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmPriorityMedicationRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PriorityMedicationRepository for SeaOrmPriorityMedicationRepo {
    async fn save(&self, model: &priority_medication::Model) -> Result<priority_medication::Model> {
        let existing = PriorityMedication::find_by_id(model.id)
            .one(&*self.db)
            .await?;

        let mut active: priority_medication::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.updated_at = Set(chrono::Utc::now());
            active.update(&*self.db).await.map_err(Error::from)
        } else {
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<priority_medication::Model>> {
        PriorityMedication::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_medication_name(
        &self,
        name: &str,
    ) -> Result<Option<priority_medication::Model>> {
        PriorityMedication::find()
            .filter(priority_medication::Column::MedicationName.eq(name))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = PriorityMedication::delete_by_id(id).exec(&*self.db).await?;
        Ok(result.rows_affected > 0)
    }

    async fn get_all_active(&self) -> Result<Vec<priority_medication::Model>> {
        let now = chrono::Utc::now();
        PriorityMedication::find()
            .filter(priority_medication::Column::Active.eq(true))
            .filter(priority_medication::Column::ActiveFrom.lte(now))
            .filter(
                Condition::any()
                    .add(priority_medication::Column::ActiveUntil.is_null())
                    .add(priority_medication::Column::ActiveUntil.gt(now)),
            )
            .order_by_desc(priority_medication::Column::PriorityLevel)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<priority_medication::Model>> {
        PriorityMedication::find()
            .order_by_desc(priority_medication::Column::PriorityLevel)
            .order_by_asc(priority_medication::Column::MedicationName)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_active(&self) -> Result<i64> {
        let now = chrono::Utc::now();
        PriorityMedication::find()
            .filter(priority_medication::Column::Active.eq(true))
            .filter(priority_medication::Column::ActiveFrom.lte(now))
            .filter(
                Condition::any()
                    .add(priority_medication::Column::ActiveUntil.is_null())
                    .add(priority_medication::Column::ActiveUntil.gt(now)),
            )
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn get_priority_for_medication(&self, medication: &str) -> Result<Option<i32>> {
        let now = chrono::Utc::now();
        let normalized = normalize_medication_name(medication);

        let result = PriorityMedication::find()
            .filter(priority_medication::Column::Active.eq(true))
            .filter(priority_medication::Column::ActiveFrom.lte(now))
            .filter(
                Condition::any()
                    .add(priority_medication::Column::ActiveUntil.is_null())
                    .add(priority_medication::Column::ActiveUntil.gt(now)),
            )
            .all(&*self.db)
            .await?;

        // Find matching medication (case-insensitive)
        for priority in result {
            if priority.matches_medication(&normalized) {
                return Ok(Some(priority.priority_score()));
            }
        }

        Ok(None)
    }

    async fn check_is_priority(&self, medication: &str) -> Result<bool> {
        self.get_priority_for_medication(medication)
            .await
            .map(|opt| opt.is_some())
    }

    async fn get_priorities_for_medications(
        &self,
        medications: &[String],
    ) -> Result<std::collections::HashMap<String, i32>> {
        let now = chrono::Utc::now();
        let priorities = PriorityMedication::find()
            .filter(priority_medication::Column::Active.eq(true))
            .filter(priority_medication::Column::ActiveFrom.lte(now))
            .filter(
                Condition::any()
                    .add(priority_medication::Column::ActiveUntil.is_null())
                    .add(priority_medication::Column::ActiveUntil.gt(now)),
            )
            .all(&*self.db)
            .await?;

        let mut result = std::collections::HashMap::new();

        for medication in medications {
            let normalized = normalize_medication_name(medication);
            for priority in &priorities {
                if priority.matches_medication(&normalized) {
                    result.insert(medication.clone(), priority.priority_score());
                    break;
                }
            }
        }

        Ok(result)
    }
}

/// Normalize medication name for matching
fn normalize_medication_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
