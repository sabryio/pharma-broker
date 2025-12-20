//! PostgreSQL Weight History Repository

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::Result;
use crate::domain::WeightHistory;
use crate::repository::WeightHistoryRepository;

pub struct PostgresWeightHistoryRepo {
    pool: PgPool,
}

impl PostgresWeightHistoryRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WeightHistoryRepository for PostgresWeightHistoryRepo {
    async fn save(&self, history: &WeightHistory) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO weight_history 
               (id, medication_weight, dosage_weight, quantity_weight, price_weight,
                recency_weight, source, sample_count, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(history.id)
        .bind(history.medication_weight)
        .bind(history.dosage_weight)
        .bind(history.quantity_weight)
        .bind(history.price_weight)
        .bind(history.recency_weight)
        .bind(&history.source)
        .bind(history.sample_count)
        .bind(history.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_current(&self) -> Result<Option<WeightHistory>> {
        let history = sqlx::query_as::<_, WeightHistory>(
            r#"SELECT id, medication_weight, dosage_weight, quantity_weight, price_weight,
                      recency_weight, source, sample_count, created_at
               FROM weight_history
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(history)
    }

    async fn get_history(&self, limit: i64) -> Result<Vec<WeightHistory>> {
        let history = sqlx::query_as::<_, WeightHistory>(
            r#"SELECT id, medication_weight, dosage_weight, quantity_weight, price_weight,
                      recency_weight, source, sample_count, created_at
               FROM weight_history
               ORDER BY created_at DESC
               LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(history)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<WeightHistory>> {
        let uuid = Uuid::parse_str(id).map_err(|e| crate::Error::validation(e.to_string()))?;
        let history = sqlx::query_as::<_, WeightHistory>(
            r#"SELECT id, medication_weight, dosage_weight, quantity_weight, price_weight,
                      recency_weight, source, sample_count, created_at
               FROM weight_history WHERE id = $1"#,
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await?;
        Ok(history)
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM weight_history")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_history_new() {
        let history = WeightHistory::new(0.35, 0.20, 0.15, 0.15, 0.15, "manual".to_string(), 100);
        assert_eq!(history.medication_weight, 0.35);
        assert_eq!(history.source, "manual");
        assert_eq!(history.sample_count, 100);
    }
}
