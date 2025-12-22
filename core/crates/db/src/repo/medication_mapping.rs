//! MedicationMapping repository implementation

use async_trait::async_trait;
use sea_orm::*;

use crate::entity::medication_mapping::{self, Entity as MedicationMapping};
use crate::traits::MedicationMappingRepository;
use crate::{Error, Result};

/// SeaORM-based medication mapping repository
pub struct SeaOrmMedicationMappingRepo {
    db: DatabaseConnection,
}

impl SeaOrmMedicationMappingRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MedicationMappingRepository for SeaOrmMedicationMappingRepo {
    async fn save(&self, model: &medication_mapping::Model) -> Result<medication_mapping::Model> {
        let existing = MedicationMapping::find_by_id(&model.id)
            .one(&self.db)
            .await?;
        let active: medication_mapping::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.update(&self.db).await.map_err(Error::from)
        } else {
            active.insert(&self.db).await.map_err(Error::from)
        }
    }

    async fn find_relevant(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<medication_mapping::Model>> {
        let pattern = format!("%{}%", query);
        MedicationMapping::find()
            .filter(
                Condition::any()
                    .add(medication_mapping::Column::ArabicName.like(&pattern))
                    .add(medication_mapping::Column::EnglishName.like(&pattern)),
            )
            .order_by_asc(medication_mapping::Column::EnglishName)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<medication_mapping::Model>> {
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        MedicationMapping::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM medication_mappings 
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
                [embedding_str.into(), limit.into()],
            ))
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<medication_mapping::Model>> {
        MedicationMapping::find()
            .order_by_asc(medication_mapping::Column::EnglishName)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count(&self) -> Result<i64> {
        MedicationMapping::find()
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }
}
