//! PostgreSQL Feedback Record Repository

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::Result;
use crate::domain::{FeedbackAverage, FeedbackRecord, FeedbackStats};
use crate::repository::FeedbackRecordRepository;

pub struct PostgresFeedbackRepo {
    pool: PgPool,
}

impl PostgresFeedbackRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FeedbackRecordRepository for PostgresFeedbackRepo {
    async fn save(&self, record: &FeedbackRecord) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO feedback_records 
               (id, match_id, user_id, confirmed, medication_score, dosage_score,
                quantity_score, price_score, recency_score, total_score, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(record.id)
        .bind(record.match_id)
        .bind(&record.user_id)
        .bind(record.confirmed)
        .bind(record.medication_score)
        .bind(record.dosage_score)
        .bind(record.quantity_score)
        .bind(record.price_score)
        .bind(record.recency_score)
        .bind(record.total_score)
        .bind(record.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_by_date_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<FeedbackRecord>> {
        let records = sqlx::query_as::<_, FeedbackRecord>(
            r#"SELECT id, match_id, user_id, confirmed, medication_score, dosage_score,
                      quantity_score, price_score, recency_score, total_score, created_at
               FROM feedback_records
               WHERE created_at >= $1 AND created_at <= $2
               ORDER BY created_at DESC"#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }

    async fn get_stats(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<FeedbackStats> {
        // Get counts
        let count_row = sqlx::query(
            r#"SELECT 
                 COUNT(*) as total,
                 COUNT(*) FILTER (WHERE confirmed = true) as confirmed,
                 COUNT(*) FILTER (WHERE confirmed = false) as rejected
               FROM feedback_records
               WHERE created_at >= $1 AND created_at <= $2"#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let sample_count: i64 = count_row.try_get("total").unwrap_or(0);
        let confirmed_count: i64 = count_row.try_get("confirmed").unwrap_or(0);
        let rejected_count: i64 = count_row.try_get("rejected").unwrap_or(0);

        // Get averages for confirmed matches
        let confirmed_avg = if confirmed_count > 0 {
            let row = sqlx::query(
                r#"SELECT 
                     AVG(medication_score) as medication,
                     AVG(dosage_score) as dosage,
                     AVG(quantity_score) as quantity,
                     AVG(price_score) as price,
                     AVG(recency_score) as recency,
                     AVG(total_score) as total
                   FROM feedback_records
                   WHERE created_at >= $1 AND created_at <= $2 AND confirmed = true"#,
            )
            .bind(start)
            .bind(end)
            .fetch_one(&self.pool)
            .await?;

            FeedbackAverage {
                medication: row.try_get::<f64, _>("medication").unwrap_or(0.0),
                dosage: row.try_get::<f64, _>("dosage").unwrap_or(0.0),
                quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
                price: row.try_get::<f64, _>("price").unwrap_or(0.0),
                recency: row.try_get::<f64, _>("recency").unwrap_or(0.0),
                total: row.try_get::<f64, _>("total").unwrap_or(0.0),
            }
        } else {
            FeedbackAverage::default()
        };

        // Get averages for rejected matches
        let rejected_avg = if rejected_count > 0 {
            let row = sqlx::query(
                r#"SELECT 
                     AVG(medication_score) as medication,
                     AVG(dosage_score) as dosage,
                     AVG(quantity_score) as quantity,
                     AVG(price_score) as price,
                     AVG(recency_score) as recency,
                     AVG(total_score) as total
                   FROM feedback_records
                   WHERE created_at >= $1 AND created_at <= $2 AND confirmed = false"#,
            )
            .bind(start)
            .bind(end)
            .fetch_one(&self.pool)
            .await?;

            FeedbackAverage {
                medication: row.try_get::<f64, _>("medication").unwrap_or(0.0),
                dosage: row.try_get::<f64, _>("dosage").unwrap_or(0.0),
                quantity: row.try_get::<f64, _>("quantity").unwrap_or(0.0),
                price: row.try_get::<f64, _>("price").unwrap_or(0.0),
                recency: row.try_get::<f64, _>("recency").unwrap_or(0.0),
                total: row.try_get::<f64, _>("total").unwrap_or(0.0),
            }
        } else {
            FeedbackAverage::default()
        };

        Ok(FeedbackStats {
            sample_count,
            confirmed_count,
            rejected_count,
            confirmed_avg,
            rejected_avg,
        })
    }

    async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM feedback_records")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn get_by_match_id(&self, match_id: &str) -> Result<Option<FeedbackRecord>> {
        let uuid =
            Uuid::parse_str(match_id).map_err(|e| crate::Error::validation(e.to_string()))?;
        let record = sqlx::query_as::<_, FeedbackRecord>(
            r#"SELECT id, match_id, user_id, confirmed, medication_score, dosage_score,
                      quantity_score, price_score, recency_score, total_score, created_at
               FROM feedback_records WHERE match_id = $1"#,
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_stats_default() {
        let stats = FeedbackStats::default();
        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.confirmed_count, 0);
        assert_eq!(stats.rejected_count, 0);
    }

    #[test]
    fn test_feedback_record_new() {
        let record = FeedbackRecord::new(
            Uuid::new_v4(),
            "user123".to_string(),
            true,
            0.9,
            0.8,
            0.7,
            0.6,
            0.5,
            0.8,
        );
        assert!(record.confirmed);
        assert_eq!(record.medication_score, 0.9);
        assert_eq!(record.user_id, "user123");
    }
}
