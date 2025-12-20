use async_trait::async_trait;
use pgvector::Vector;
use sqlx::{PgPool, Row};

use crate::Result;
use crate::domain::MedicationMapping;
use crate::repository::MedicationMappingRepository;

pub struct PostgresMedicationMappingRepo {
    pool: PgPool,
}

impl PostgresMedicationMappingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MedicationMappingRepository for PostgresMedicationMappingRepo {
    async fn save(&self, mapping: &MedicationMapping) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO medication_mappings (
                id, arabic_name, english_name, synonyms, embedding, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE SET
                arabic_name = EXCLUDED.arabic_name,
                english_name = EXCLUDED.english_name,
                synonyms = EXCLUDED.synonyms,
                embedding = EXCLUDED.embedding,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&mapping.id)
        .bind(&mapping.arabic_name)
        .bind(&mapping.english_name)
        .bind(&mapping.synonyms)
        .bind(&mapping.embedding)
        .bind(mapping.created_at)
        .bind(mapping.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_relevant(&self, query: &str, limit: i64) -> Result<Vec<MedicationMapping>> {
        if query.is_empty() {
            return Ok(vec![]);
        }

        let mappings = sqlx::query_as::<_, MedicationMapping>(
            r#"
            SELECT id, arabic_name, english_name, synonyms, embedding, created_at, updated_at
            FROM medication_mappings
            WHERE arabic_name % $1 OR english_name % $1
            ORDER BY GREATEST(similarity(arabic_name, $1), similarity(english_name, $1)) DESC
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(mappings)
    }

    async fn find_similar(&self, embedding: &[f32], limit: i64) -> Result<Vec<MedicationMapping>> {
        if embedding.is_empty() {
            return Ok(vec![]);
        }

        let vec = Vector::from(embedding.to_vec());

        // Use pgvector's <=> operator for cosine distance (smaller = more similar)
        let mappings = sqlx::query_as::<_, MedicationMapping>(
            r#"
            SELECT id, arabic_name, english_name, synonyms, embedding, created_at, updated_at
            FROM medication_mappings
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1
            LIMIT $2
            "#,
        )
        .bind(vec)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(mappings)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<MedicationMapping>> {
        let mappings = sqlx::query_as::<_, MedicationMapping>(
            r#"
            SELECT id, arabic_name, english_name, synonyms, embedding, created_at, updated_at
            FROM medication_mappings
            ORDER BY arabic_name ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(mappings)
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM medication_mappings")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}
