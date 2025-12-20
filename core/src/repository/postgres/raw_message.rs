//! PostgreSQL implementation for RawMessageRepository
//!
//! Ported from legacy/storage/gorm/raw_message_repo.go

use async_trait::async_trait;
use sqlx::PgPool;

use crate::Result;
use crate::domain::RawMessage;
use crate::repository::RawMessageRepository;

/// PostgreSQL implementation of RawMessageRepository
pub struct PostgresRawMessageRepo {
    pool: PgPool,
}

impl PostgresRawMessageRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RawMessageRepository for PostgresRawMessageRepo {
    /// Save a new raw message to the database
    async fn save(&self, message: &RawMessage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO raw_messages (
                id, external_id, group_jid, group_name,
                sender_jid, sender_phone, sender_name, content,
                timestamp, processed_at, error,
                reply_to_id, reply_to_content, reply_to_sender
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&message.id)
        .bind(&message.external_id)
        .bind(&message.group_jid)
        .bind(&message.group_name)
        .bind(&message.sender_jid)
        .bind(&message.sender_phone)
        .bind(&message.sender_name)
        .bind(&message.content)
        .bind(message.timestamp)
        .bind(message.processed_at)
        .bind(&message.error)
        .bind(&message.reply_to_id)
        .bind(&message.reply_to_content)
        .bind(&message.reply_to_sender)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get unprocessed messages for AI processing
    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessage>> {
        let messages = sqlx::query_as::<_, RawMessage>(
            r#"
            SELECT 
                id, external_id, group_jid, group_name,
                sender_jid, sender_phone, sender_name, content,
                timestamp, processed_at, error,
                reply_to_id, reply_to_content, reply_to_sender
            FROM raw_messages
            WHERE processed_at IS NULL
            ORDER BY timestamp ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    /// Mark a message as processed (with optional error)
    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE raw_messages
            SET processed_at = NOW(), error = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_before(&self, cutoff: &chrono::DateTime<chrono::Utc>) -> Result<usize> {
        // Only delete processed messages
        let result = sqlx::query(
            "DELETE FROM raw_messages WHERE processed_at IS NOT NULL AND processed_at < $1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as usize)
    }
}
