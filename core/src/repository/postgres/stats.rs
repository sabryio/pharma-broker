use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::Result;
use crate::domain::Stats;
use crate::repository::StatsRepository;

pub struct PostgresStatsRepo {
    pool: PgPool,
}

impl PostgresStatsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StatsRepository for PostgresStatsRepo {
    async fn get_stats(&self) -> Result<Stats> {
        let active_offers = sqlx::query("SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE'")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>(0);

        let active_requests = sqlx::query("SELECT COUNT(*) FROM requests WHERE status = 'ACTIVE'")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>(0);

        let pending_matches = sqlx::query("SELECT COUNT(*) FROM matches WHERE status = 'PENDING'")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>(0);

        let confirmed_today = sqlx::query(
            "SELECT COUNT(*) FROM matches WHERE status = 'CONFIRMED' AND confirmed_at >= CURRENT_DATE"
        )
        .fetch_one(&self.pool)
        .await?
        .get::<i64, _>(0);

        let processed_today =
            sqlx::query("SELECT COUNT(*) FROM raw_messages WHERE processed_at >= CURRENT_DATE")
                .fetch_one(&self.pool)
                .await?
                .get::<i64, _>(0);

        let monitored_groups = sqlx::query("SELECT COUNT(*) FROM groups WHERE monitored = true")
            .fetch_one(&self.pool)
            .await?
            .get::<i64, _>(0) as i32;

        Ok(Stats {
            active_offers,
            active_requests,
            pending_matches,
            confirmed_today,
            processed_today,
            avg_match_score: 0.0, // Placeholder
            monitored_groups,
            connected_clients: 0, // Set by AppState
        })
    }
}
