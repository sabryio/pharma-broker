//! PostgreSQL Offer Repository
//!
//! Ported from legacy/storage/gorm/offer_repo.go
//! Uses runtime queries (no compile-time DATABASE_URL required)

use async_trait::async_trait;
use chrono::Duration;
use sqlx::{PgPool, Row};

use crate::domain::{ItemStatus, Offer};
use crate::repository::OfferRepository;
use crate::{Error, Result};

/// PostgreSQL implementation of OfferRepository
pub struct PostgresOfferRepo {
    pool: PgPool,
}

impl PostgresOfferRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OfferRepository for PostgresOfferRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<Offer>> {
        let offer = sqlx::query_as::<_, Offer>(
            r#"
            SELECT id, raw_message_id, source_phone, source_name, source_group,
                   group_name, medication, medication_raw, quantity, unit,
                   price, currency, expiry_date, batch_number, notes,
                   raw_message, status, created_at, updated_at
            FROM offers WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(offer)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Offer>> {
        let offers = sqlx::query_as::<_, Offer>(
            r#"
            SELECT id, raw_message_id, source_phone, source_name, source_group,
                   group_name, medication, medication_raw, quantity, unit,
                   price, currency, expiry_date, batch_number, notes,
                   raw_message, status, created_at, updated_at
            FROM offers WHERE status = 'ACTIVE'
            ORDER BY created_at DESC LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(offers)
    }

    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Offer>> {
        let pattern = format!("%{}%", query);
        let offers = sqlx::query_as::<_, Offer>(
            r#"
            SELECT id, raw_message_id, source_phone, source_name, source_group,
                   group_name, medication, medication_raw, quantity, unit,
                   price, currency, expiry_date, batch_number, notes,
                   raw_message, status, created_at, updated_at
            FROM offers WHERE status = 'ACTIVE'
              AND (medication ILIKE $1 OR medication_raw ILIKE $1)
            ORDER BY created_at DESC LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(offers)
    }

    async fn count_active(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM offers WHERE status = 'ACTIVE'")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<Offer>> {
        let cutoff = chrono::Utc::now() - within;

        let offer = sqlx::query_as::<_, Offer>(
            r#"
            SELECT id, raw_message_id, source_phone, source_name, source_group,
                   group_name, medication, medication_raw, quantity, unit,
                   price, currency, expiry_date, batch_number, notes,
                   raw_message, status, created_at, updated_at
            FROM offers
            WHERE source_phone = $1 AND medication = $2 AND created_at > $3
            ORDER BY created_at DESC LIMIT 1
            "#,
        )
        .bind(sender_phone)
        .bind(medication)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await?;

        Ok(offer)
    }

    async fn save(&self, offer: &Offer) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO offers (
                id, raw_message_id, source_phone, source_name, source_group,
                group_name, medication, medication_raw, quantity, unit,
                price, currency, expiry_date, batch_number, notes,
                raw_message, status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            ON CONFLICT (id) DO UPDATE SET
                medication = EXCLUDED.medication, quantity = EXCLUDED.quantity,
                price = EXCLUDED.price, status = EXCLUDED.status, updated_at = EXCLUDED.updated_at
            "#
        )
        .bind(&offer.id)
        .bind(&offer.raw_message_id)
        .bind(&offer.source_phone)
        .bind(&offer.source_name)
        .bind(&offer.source_group)
        .bind(&offer.group_name)
        .bind(&offer.medication)
        .bind(&offer.medication_raw)
        .bind(offer.quantity)
        .bind(&offer.unit)
        .bind(offer.price)
        .bind(&offer.currency)
        .bind(offer.expiry_date)
        .bind(&offer.batch_number)
        .bind(&offer.notes)
        .bind(&offer.raw_message)
        .bind(offer.status.to_string())
        .bind(offer.created_at)
        .bind(offer.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<()> {
        let result = sqlx::query("UPDATE offers SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(Error::not_found(format!("Offer {}", id)));
        }

        Ok(())
    }
}
