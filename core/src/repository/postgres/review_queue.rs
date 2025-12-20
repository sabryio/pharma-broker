//! PostgreSQL Review Queue Repository
//!
//! Implements ReviewQueueRepository for storing and managing AI parse results
//! that require human review.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::Result;
use crate::domain::{ReviewQueueItem, ReviewQueueStats, ReviewStatus};
use crate::repository::ReviewQueueRepository;

/// PostgreSQL implementation of ReviewQueueRepository
pub struct PostgresReviewQueueRepo {
    pool: PgPool,
}

impl PostgresReviewQueueRepo {
    /// Create a new repository with the given connection pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReviewQueueRepository for PostgresReviewQueueRepo {
    async fn save(&self, item: &ReviewQueueItem) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO review_queue 
               (id, raw_message_id, ai_result, confidence, reason, status, 
                reviewed_by, review_notes, created_at, reviewed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               ON CONFLICT (id) DO UPDATE SET
                 status = EXCLUDED.status,
                 reviewed_by = EXCLUDED.reviewed_by,
                 review_notes = EXCLUDED.review_notes,
                 reviewed_at = EXCLUDED.reviewed_at"#,
        )
        .bind(item.id)
        .bind(&item.raw_message_id)
        .bind(&item.ai_result)
        .bind(item.confidence)
        .bind(&item.reason)
        .bind(item.status.to_string())
        .bind(&item.reviewed_by)
        .bind(&item.review_notes)
        .bind(item.created_at)
        .bind(item.reviewed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<ReviewQueueItem>> {
        let uuid = Uuid::parse_str(id).map_err(|e| crate::Error::validation(e.to_string()))?;

        let row = sqlx::query(
            r#"SELECT id, raw_message_id, ai_result, confidence, reason, status,
                      reviewed_by, review_notes, created_at, reviewed_at
               FROM review_queue WHERE id = $1"#,
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_item(row)?)),
            None => Ok(None),
        }
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<ReviewQueueItem>> {
        let rows = sqlx::query(
            r#"SELECT id, raw_message_id, ai_result, confidence, reason, status,
                      reviewed_by, review_notes, created_at, reviewed_at
               FROM review_queue 
               WHERE status = 'pending'
               ORDER BY created_at ASC
               LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_item).collect()
    }

    async fn get_by_status(
        &self,
        status: ReviewStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReviewQueueItem>> {
        let rows = sqlx::query(
            r#"SELECT id, raw_message_id, ai_result, confidence, reason, status,
                      reviewed_by, review_notes, created_at, reviewed_at
               FROM review_queue 
               WHERE status = $1
               ORDER BY created_at DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(status.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_item).collect()
    }

    async fn update_status(
        &self,
        id: &str,
        status: ReviewStatus,
        reviewed_by: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let uuid = Uuid::parse_str(id).map_err(|e| crate::Error::validation(e.to_string()))?;

        sqlx::query(
            r#"UPDATE review_queue 
               SET status = $2, reviewed_by = $3, review_notes = $4, reviewed_at = NOW()
               WHERE id = $1"#,
        )
        .bind(uuid)
        .bind(status.to_string())
        .bind(reviewed_by)
        .bind(notes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_stats(&self) -> Result<ReviewQueueStats> {
        let row = sqlx::query(
            r#"SELECT 
                 COUNT(*) as total,
                 COUNT(*) FILTER (WHERE status = 'pending') as pending,
                 COUNT(*) FILTER (WHERE status = 'approved') as approved,
                 COUNT(*) FILTER (WHERE status = 'rejected') as rejected,
                 COUNT(*) FILTER (WHERE status = 'skipped') as skipped,
                 COALESCE(AVG(confidence) FILTER (WHERE status = 'pending'), 0) as avg_pending_confidence
               FROM review_queue"#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ReviewQueueStats {
            total: row.try_get::<i64, _>("total").unwrap_or(0),
            pending: row.try_get::<i64, _>("pending").unwrap_or(0),
            approved: row.try_get::<i64, _>("approved").unwrap_or(0),
            rejected: row.try_get::<i64, _>("rejected").unwrap_or(0),
            skipped: row.try_get::<i64, _>("skipped").unwrap_or(0),
            avg_pending_confidence: row
                .try_get::<f64, _>("avg_pending_confidence")
                .unwrap_or(0.0),
        })
    }

    async fn count_pending(&self) -> Result<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM review_queue WHERE status = 'pending'")
                .fetch_one(&self.pool)
                .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn exists_for_message(&self, raw_message_id: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM review_queue WHERE raw_message_id = $1) as exists",
        )
        .bind(raw_message_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<bool, _>("exists"))
    }
}

/// Convert a database row to ReviewQueueItem
fn row_to_item(row: sqlx::postgres::PgRow) -> Result<ReviewQueueItem> {
    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
        "pending" => ReviewStatus::Pending,
        "approved" => ReviewStatus::Approved,
        "rejected" => ReviewStatus::Rejected,
        "skipped" => ReviewStatus::Skipped,
        _ => ReviewStatus::Pending,
    };

    Ok(ReviewQueueItem {
        id: row.try_get("id")?,
        raw_message_id: row.try_get("raw_message_id")?,
        ai_result: row.try_get("ai_result")?,
        confidence: row.try_get("confidence")?,
        reason: row.try_get("reason")?,
        status,
        reviewed_by: row.try_get("reviewed_by")?,
        review_notes: row.try_get("review_notes")?,
        created_at: row.try_get("created_at")?,
        reviewed_at: row.try_get("reviewed_at")?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_to_string() {
        assert_eq!(ReviewStatus::Pending.to_string(), "pending");
        assert_eq!(ReviewStatus::Approved.to_string(), "approved");
        assert_eq!(ReviewStatus::Rejected.to_string(), "rejected");
        assert_eq!(ReviewStatus::Skipped.to_string(), "skipped");
    }

    #[test]
    fn test_review_queue_stats_default() {
        let stats = ReviewQueueStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.approved, 0);
        assert_eq!(stats.rejected, 0);
        assert_eq!(stats.avg_pending_confidence, 0.0);
    }

    #[test]
    fn test_review_queue_item_creation() {
        let item = ReviewQueueItem::for_low_confidence(
            "msg-123".to_string(),
            serde_json::json!({"items": []}),
            0.35,
        );

        assert_eq!(item.raw_message_id, "msg-123");
        assert_eq!(item.confidence, 0.35);
        assert!(item.reason.contains("0.35"));
        assert_eq!(item.status, ReviewStatus::Pending);
    }
}
