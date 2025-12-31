//! MedicationMapping Service - Arabic to English medication name mappings

use sea_orm::*;
use uuid::Uuid;

use crate::entity::medication_mapping::{self, Entity as MedicationMapping};
use crate::{Error, Result};

/// Service for medication mapping operations
pub struct MedicationMappingService;

impl MedicationMappingService {
    /// Save a new mapping
    pub async fn save(
        db: &DatabaseConnection,
        model: medication_mapping::ActiveModel,
    ) -> Result<medication_mapping::Model> {
        let id = model.id.clone().unwrap();

        // Upsert logic
        let existing = MedicationMapping::find_by_id(id).one(db).await?;

        if existing.is_some() {
            model.update(db).await.map_err(Error::from)
        } else {
            model.insert(db).await.map_err(Error::from)
        }
    }

    /// Get mapping by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<medication_mapping::Model>> {
        MedicationMapping::find_by_id(id)
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get mapping by Arabic name
    pub async fn get_by_arabic_name(
        db: &DatabaseConnection,
        arabic_name: &str,
    ) -> Result<Option<medication_mapping::Model>> {
        MedicationMapping::find()
            .filter(medication_mapping::Column::ArabicName.eq(arabic_name))
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get mapping by English name
    pub async fn get_by_english_name(
        db: &DatabaseConnection,
        english_name: &str,
    ) -> Result<Option<medication_mapping::Model>> {
        MedicationMapping::find()
            .filter(medication_mapping::Column::EnglishName.eq(english_name))
            .one(db)
            .await
            .map_err(Error::from)
    }

    /// Get all mappings
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<medication_mapping::Model>> {
        MedicationMapping::find()
            .order_by_asc(medication_mapping::Column::EnglishName)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Find relevant mappings using vector similarity search
    pub async fn find_relevant(
        db: &DatabaseConnection,
        embedding: &[f32],
        threshold: f64,
        limit: u64,
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
                AND 1 - (embedding <=> $1::vector) > $2
                ORDER BY embedding <=> $1::vector
                LIMIT $3
                "#,
                [
                    embedding_str.into(),
                    threshold.into(),
                    (limit as i64).into(),
                ],
            ))
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Search mappings by text (Arabic or English)
    pub async fn search(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<medication_mapping::Model>> {
        let pattern = format!("%{}%", query);
        MedicationMapping::find()
            .filter(
                Condition::any()
                    .add(medication_mapping::Column::ArabicName.like(&pattern))
                    .add(medication_mapping::Column::EnglishName.like(&pattern)),
            )
            .order_by_asc(medication_mapping::Column::EnglishName)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count all mappings
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        MedicationMapping::find()
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Delete a mapping
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = MedicationMapping::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_service_exists() {
        // Basic compile test
    }
}
