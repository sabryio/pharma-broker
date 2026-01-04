//! MedicationMaster repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::medication_master::{self, Entity as MedicationMaster};
use crate::traits::MedicationMasterRepository;
use crate::{Error, Result};

/// SeaORM-based medication master repository
pub struct SeaOrmMedicationMasterRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmMedicationMasterRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MedicationMasterRepository for SeaOrmMedicationMasterRepo {
    async fn save(&self, model: &medication_master::Model) -> Result<medication_master::Model> {
        let existing = MedicationMaster::find_by_id(model.id)
            .one(&*self.db)
            .await?;
        let mut active: medication_master::ActiveModel = model.clone().into();

        if existing.is_some() {
            // Force canonical_name to be set so update works
            active.canonical_name = Set(model.canonical_name.clone());
            active.updated_at = Set(chrono::Utc::now());
            active.update(&*self.db).await.map_err(Error::from)
        } else {
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<medication_master::Model>> {
        MedicationMaster::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<medication_master::Model>> {
        MedicationMaster::find()
            .filter(medication_master::Column::CanonicalName.eq(name))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn search(&self, name: &str, limit: i64) -> Result<Vec<medication_master::Model>> {
        let pattern = format!("%{}%", name);
        MedicationMaster::find()
            .filter(
                Condition::any()
                    .add(medication_master::Column::CanonicalName.like(&pattern))
                    .add(medication_master::Column::CanonicalNameAr.like(&pattern)),
            )
            .order_by_asc(medication_master::Column::CanonicalName)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn search_fuzzy(
        &self,
        name: &str,
        limit: i64,
        min_similarity: f32,
    ) -> Result<Vec<(medication_master::Model, f32)>> {
        // Use pg_trgm similarity function for fuzzy matching
        let raw_results = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT *, 
                    GREATEST(
                        similarity(canonical_name, $1),
                        COALESCE(similarity(canonical_name_ar, $1), 0)
                    ) as score
                FROM medication_master
                WHERE similarity(canonical_name, $1) > $2
                   OR (canonical_name_ar IS NOT NULL AND similarity(canonical_name_ar, $1) > $2)
                ORDER BY score DESC
                LIMIT $3
                "#,
                [name.into(), min_similarity.into(), limit.into()],
            ))
            .await?;

        let mut results = Vec::new();
        for row in raw_results {
            let model = medication_master::Model::from_query_result(&row, "")?;
            let score: f64 = row.try_get("", "score")?;
            results.push((model, score as f32));
        }

        Ok(results)
    }

    async fn search_semantic(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(medication_master::Model, f32)>> {
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let raw_results = self
            .db
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
            SELECT *, (1 - (embedding <=> $1::vector)) as score
            FROM medication_master
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
                [embedding_str.into(), limit.into()],
            ))
            .await?;

        let mut results = Vec::new();
        for row in raw_results {
            let model = medication_master::Model::from_query_result(&row, "")?;
            // PostgreSQL returns FLOAT8 (f64) for the calculated score
            let score: f64 = row.try_get("", "score")?;
            results.push((model, score as f32));
        }

        Ok(results)
    }

    async fn count(&self) -> Result<i64> {
        MedicationMaster::find()
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<medication_master::Model>> {
        MedicationMaster::find()
            .order_by_asc(medication_master::Column::CanonicalName)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn find_relevant(
        &self,
        content: &str,
        limit: i64,
    ) -> Result<Vec<medication_master::Model>> {
        // Extract potential medication names from content using simple word matching
        // This is a basic implementation - could be enhanced with NLP
        let words: Vec<&str> = content.split_whitespace().collect();

        if words.is_empty() {
            return Ok(Vec::new());
        }

        // Build a condition that matches any word in the content
        let mut condition = Condition::any();
        for word in words.iter().take(20) {
            // Limit to first 20 words
            if word.len() >= 3 {
                // Only match words with 3+ chars
                let pattern = format!("%{}%", word);
                condition = condition
                    .add(medication_master::Column::CanonicalName.like(&pattern))
                    .add(medication_master::Column::CanonicalNameAr.like(&pattern));
            }
        }

        MedicationMaster::find()
            .filter(condition)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }
}
