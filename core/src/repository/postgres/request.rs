//! PostgreSQL Request Repository (runtime queries)

use async_trait::async_trait;
use chrono::Duration;
use sqlx::{PgPool, Row};

use crate::domain::{ItemStatus, Request};
use crate::repository::RequestRepository;
use crate::{Error, Result};

pub struct PostgresRequestRepo {
    pool: PgPool,
}

impl PostgresRequestRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RequestRepository for PostgresRequestRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<Request>> {
        let request = sqlx::query_as::<_, Request>(
            r#"SELECT id, raw_message_id, source_phone, source_name, source_group,
                      medication, medication_raw, quantity, unit,
                      max_price, currency, urgency_level, expiry_requirement,
                      ai_confidence, notes,
                      status, content_embedding, created_at, updated_at
               FROM requests WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(request)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Request>> {
        let requests = sqlx::query_as::<_, Request>(
            r#"SELECT id, raw_message_id, source_phone, source_name, source_group,
                      medication, medication_raw, quantity, unit,
                      max_price, currency, urgency_level, expiry_requirement,
                      ai_confidence, notes,
                      status, content_embedding, created_at, updated_at
               FROM requests WHERE status = 'ACTIVE'
               ORDER BY urgency_level DESC, created_at DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(requests)
    }

    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Request>> {
        let pattern = format!("%{}%", query);
        let requests = sqlx::query_as::<_, Request>(
            r#"SELECT id, raw_message_id, source_phone, source_name, source_group,
                      medication, medication_raw, quantity, unit,
                      max_price, currency, urgency_level, expiry_requirement,
                      ai_confidence, notes,
                      status, content_embedding, created_at, updated_at
               FROM requests WHERE status = 'ACTIVE'
                 AND (medication ILIKE $1 OR medication_raw ILIKE $1)
               ORDER BY urgency_level DESC, created_at DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(requests)
    }

    async fn count_active(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM requests WHERE status = 'ACTIVE'")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<Request>> {
        let cutoff = chrono::Utc::now() - within;

        let request = sqlx::query_as::<_, Request>(
            r#"SELECT id, raw_message_id, source_phone, source_name, source_group,
                      medication, medication_raw, quantity, unit,
                      max_price, currency, urgency_level, expiry_requirement,
                      ai_confidence, notes,
                      status, content_embedding, created_at, updated_at
               FROM requests
               WHERE source_phone = $1 AND medication = $2 AND created_at > $3
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(sender_phone)
        .bind(medication)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await?;
        Ok(request)
    }

    async fn find_semantic_duplicates(
        &self,
        embedding: &[f32],
        similarity_threshold: f64,
        within: Duration,
    ) -> Result<Vec<Request>> {
        let cutoff = chrono::Utc::now() - within;
        let max_distance = 1.0 - similarity_threshold;

        let requests = sqlx::query_as::<_, Request>(
            r#"SELECT id, raw_message_id, source_phone, source_name, source_group,
                      medication, medication_raw, quantity, unit,
                      max_price, currency, urgency_level, expiry_requirement,
                      ai_confidence, notes,
                      status, content_embedding, created_at, updated_at
               FROM requests
               WHERE created_at > $1
                 AND content_embedding <=> $2 < $3
               ORDER BY content_embedding <=> $2 ASC
               LIMIT 5"#,
        )
        .bind(cutoff)
        .bind(embedding)
        .bind(max_distance)
        .fetch_all(&self.pool)
        .await?;

        Ok(requests)
    }

    async fn save(&self, request: &Request) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO requests (id, raw_message_id, source_phone, source_name, source_group,
                medication, medication_raw, quantity, unit, max_price, currency,
                urgency_level, expiry_requirement, ai_confidence,
                notes, status, content_embedding, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
               ON CONFLICT (id) DO UPDATE SET medication = EXCLUDED.medication,
                 quantity = EXCLUDED.quantity, max_price = EXCLUDED.max_price,
                 urgency_level = EXCLUDED.urgency_level,
                 expiry_requirement = EXCLUDED.expiry_requirement, ai_confidence = EXCLUDED.ai_confidence,
                 status = EXCLUDED.status, content_embedding = EXCLUDED.content_embedding,
                 updated_at = EXCLUDED.updated_at"#,
        )
        .bind(&request.id)
        .bind(&request.raw_message_id)
        .bind(&request.source_phone)
        .bind(&request.source_name)
        .bind(&request.source_group)
        .bind(&request.medication)
        .bind(&request.medication_raw)
        .bind(request.quantity)
        .bind(&request.unit)
        .bind(request.max_price)
        .bind(&request.currency)
        .bind(&request.urgency_level)
        .bind(&request.expiry_requirement)
        .bind(request.ai_confidence)
        .bind(&request.notes)
        .bind(request.status.to_string())
        .bind(&request.content_embedding)
        .bind(request.created_at)
        .bind(request.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()> {
        let result =
            sqlx::query("UPDATE requests SET status = $1, updated_at = NOW() WHERE id = $2")
                .bind(status.to_string())
                .bind(id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(Error::not_found(format!("Request {}", id)));
        }
        Ok(())
    }

    async fn delete_before(&self, cutoff: &chrono::DateTime<chrono::Utc>) -> Result<usize> {
        let result = sqlx::query("DELETE FROM requests WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as usize)
    }
}
