//! PostgreSQL Match Repository (runtime queries)

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::domain::{Match, MatchStatus};
use crate::repository::MatchRepository;
use crate::{Error, Result};

pub struct PostgresMatchRepo {
    pool: PgPool,
}

impl PostgresMatchRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MatchRepository for PostgresMatchRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<Match>> {
        let m = sqlx::query_as::<_, Match>(
            r#"SELECT id, offer_id, request_id, score, reasoning, matched_by,
                      status, created_at, confirmed_at, notes
               FROM matches WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(m)
    }

    async fn get_pending(&self, limit: i64, offset: i64) -> Result<Vec<Match>> {
        let matches = sqlx::query_as::<_, Match>(
            r#"SELECT id, offer_id, request_id, score, reasoning, matched_by,
                      status, created_at, confirmed_at, notes
               FROM matches WHERE status = 'PENDING'
               ORDER BY score DESC, created_at DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(matches)
    }

    async fn count_pending(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM matches WHERE status = 'PENDING'")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn exists(&self, offer_id: &str, request_id: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM matches WHERE offer_id = $1 AND request_id = $2) as ex",
        )
        .bind(offer_id)
        .bind(request_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<bool, _>("ex"))
    }

    async fn save(&self, m: &Match) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO matches (id, offer_id, request_id, score, reasoning, matched_by,
                status, created_at, confirmed_at, notes)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
               ON CONFLICT (id) DO UPDATE SET score = EXCLUDED.score, reasoning = EXCLUDED.reasoning,
                 status = EXCLUDED.status, confirmed_at = EXCLUDED.confirmed_at, notes = EXCLUDED.notes"#
        )
        .bind(&m.id).bind(&m.offer_id).bind(&m.request_id).bind(m.score)
        .bind(&m.reasoning).bind(&m.matched_by).bind(m.status.to_string())
        .bind(m.created_at).bind(m.confirmed_at).bind(&m.notes)
        .execute(&self.pool).await?;
        Ok(())
    }

    async fn update_status(
        &self,
        id: &str,
        status: MatchStatus,
        matched_by: &str,
        notes: &str,
    ) -> Result<()> {
        let confirmed_at = if status == MatchStatus::Confirmed {
            Some(chrono::Utc::now())
        } else {
            None
        };
        let result = sqlx::query("UPDATE matches SET status = $1, matched_by = $2, notes = $3, confirmed_at = $4 WHERE id = $5")
            .bind(status.to_string()).bind(matched_by).bind(notes).bind(confirmed_at).bind(id)
            .execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(Error::not_found(format!("Match {}", id)));
        }
        Ok(())
    }

    async fn delete_before(&self, cutoff: &chrono::DateTime<chrono::Utc>) -> Result<usize> {
        let result = sqlx::query("DELETE FROM matches WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as usize)
    }
}
