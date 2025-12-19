//! PostgreSQL implementation for GroupRepository
//!
//! Ported from legacy/storage/gorm/group_repo.go

use async_trait::async_trait;
use sqlx::PgPool;

use crate::Result;
use crate::domain::Group;
use crate::repository::GroupRepository;

/// PostgreSQL implementation of GroupRepository
pub struct PostgresGroupRepo {
    pool: PgPool,
}

impl PostgresGroupRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GroupRepository for PostgresGroupRepo {
    /// Get all groups
    async fn get_all(&self) -> Result<Vec<Group>> {
        let groups = sqlx::query_as::<_, Group>(
            r#"
            SELECT jid, name, description, monitored, added_at, last_message, message_count
            FROM groups
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Get group by JID
    async fn get_by_jid(&self, jid: &str) -> Result<Option<Group>> {
        let group = sqlx::query_as::<_, Group>(
            r#"
            SELECT jid, name, description, monitored, added_at, last_message, message_count
            FROM groups
            WHERE jid = $1
            "#,
        )
        .bind(jid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(group)
    }

    /// Check if a group is monitored
    async fn is_monitored(&self, jid: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT monitored FROM groups WHERE jid = $1
            "#,
        )
        .bind(jid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.unwrap_or(false))
    }

    /// Get all monitored groups
    async fn get_monitored(&self) -> Result<Vec<Group>> {
        let groups = sqlx::query_as::<_, Group>(
            r#"
            SELECT jid, name, description, monitored, added_at, last_message, message_count
            FROM groups
            WHERE monitored = true
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(groups)
    }

    /// Save or update a group
    async fn save(&self, group: &Group) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO groups (jid, name, description, monitored, added_at, last_message, message_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (jid) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                monitored = EXCLUDED.monitored,
                last_message = EXCLUDED.last_message,
                message_count = EXCLUDED.message_count
            "#,
        )
        .bind(&group.jid)
        .bind(&group.name)
        .bind(&group.description)
        .bind(group.monitored)
        .bind(group.added_at)
        .bind(group.last_message)
        .bind(group.message_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update monitoring status
    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE groups SET monitored = $2 WHERE jid = $1
            "#,
        )
        .bind(jid)
        .bind(monitored)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a group
    async fn delete(&self, jid: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM groups WHERE jid = $1
            "#,
        )
        .bind(jid)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update last message timestamp
    async fn update_last_message(&self, jid: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE groups SET last_message = NOW() WHERE jid = $1
            "#,
        )
        .bind(jid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Increment message count
    async fn increment_message_count(&self, jid: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE groups SET message_count = message_count + 1 WHERE jid = $1
            "#,
        )
        .bind(jid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
