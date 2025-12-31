//! MedicationAlias repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::medication_alias::{self, Entity as MedicationAlias};
use crate::traits::{CurationStats, MedicationAliasRepository};
use crate::{Error, Result};

/// SeaORM-based medication alias repository
pub struct SeaOrmMedicationAliasRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmMedicationAliasRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MedicationAliasRepository for SeaOrmMedicationAliasRepo {
    async fn save(&self, model: &medication_alias::Model) -> Result<medication_alias::Model> {
        let existing = MedicationAlias::find_by_id(model.id).one(&*self.db).await?;
        let mut active: medication_alias::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.last_seen_at = Set(chrono::Utc::now());
            active.update(&*self.db).await.map_err(Error::from)
        } else {
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<medication_alias::Model>> {
        MedicationAlias::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<medication_alias::Model>> {
        MedicationAlias::find()
            .filter(medication_alias::Column::AliasName.eq(name))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<medication_alias::Model>> {
        MedicationAlias::find()
            .filter(
                medication_alias::Column::CurationStatus.eq(crate::traits::CurationStatus::Pending),
            )
            .order_by_desc(medication_alias::Column::OccurrenceCount)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<medication_alias::Model>> {
        MedicationAlias::find()
            .order_by_desc(medication_alias::Column::OccurrenceCount)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_pending(&self) -> Result<i64> {
        MedicationAlias::find()
            .filter(
                medication_alias::Column::CurationStatus.eq(crate::traits::CurationStatus::Pending),
            )
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn count_all(&self) -> Result<i64> {
        MedicationAlias::find()
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn delete(&self, id: Uuid) -> Result<bool> {
        let result = MedicationAlias::delete_by_id(id).exec(&*self.db).await?;
        Ok(result.rows_affected > 0)
    }

    async fn get_stats(&self) -> Result<CurationStats> {
        use crate::entity::medication_master;
        use crate::entity::offer;

        let total_offers: i64 = offer::Entity::find().count(&*self.db).await? as i64;
        let curated_offers: i64 = offer::Entity::find()
            .filter(offer::Column::MasterMedicationId.is_not_null())
            .count(&*self.db)
            .await? as i64;
        let master_medications: i64 =
            medication_master::Entity::find().count(&*self.db).await? as i64;
        let total_aliases: i64 = MedicationAlias::find().count(&*self.db).await? as i64;
        let pending_aliases: i64 = self.count_pending().await?;

        Ok(CurationStats {
            total_offers,
            curated_offers,
            master_medications,
            total_aliases,
            pending_aliases,
        })
    }
}
