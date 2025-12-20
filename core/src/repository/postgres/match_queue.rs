use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::Result;
use crate::domain::MatchQueueItem;
use crate::repository::MatchQueueRepository;

pub struct PostgresMatchQueueRepo {
    pool: PgPool,
}

impl PostgresMatchQueueRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MatchQueueRepository for PostgresMatchQueueRepo {
    async fn enqueue(&self, request_id: &str, priority: i32) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO match_queue_items (request_id, priority, status)
            VALUES ($1, $2, 'PENDING')
            "#,
        )
        .bind(Uuid::parse_str(request_id).unwrap_or_default())
        .bind(priority)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn fetch_batch(&self, limit: i64) -> Result<Vec<MatchQueueItem>> {
        // Optimized fetch:
        // 1. Selects pending items ordered by priority and time
        // 2. Locks selected rows (FOR UPDATE SKIP LOCKED) to prevent other workers from picking them
        // 3. Updates status to PROCESSING immediately to hold the lock logic
        // This acts as a robust distributed queue consumer pattern.
        let items = sqlx::query_as::<_, MatchQueueItem>(
            r#"
            UPDATE match_queue_items
            SET status = 'PROCESSING', updated_at = NOW()
            WHERE id IN (
                SELECT id
                FROM match_queue_items
                WHERE status = 'PENDING'
                  AND next_attempt_at <= NOW()
                ORDER BY priority DESC, created_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    async fn complete(&self, id: &Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE match_queue_items
            SET status = 'COMPLETED', updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn fail(&self, id: &Uuid, error: &str) -> Result<()> {
        // Exponential backoff logic could be added here in SQL or application layer
        // For now, simple retry count increment and error logging
        sqlx::query(
            r#"
            UPDATE match_queue_items
            SET status = CASE WHEN attempts >= 3 THEN 'FAILED' ELSE 'PENDING' END,
                attempts = attempts + 1,
                last_error = $2,
                next_attempt_at = NOW() + (POWER(2, attempts) * INTERVAL '1 minute'),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn count_pending(&self) -> Result<i64> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM match_queue_items WHERE status = 'PENDING'")
                .fetch_one(&self.pool)
                .await?;
        Ok(count.0)
    }
}
