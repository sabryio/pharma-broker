//! Database Maintenance Service - Auto-partitioning and Data Pruning

use crate::traits::{
    AuditLogRepository, MatchQueueRepository, MatchRepository, OfferRepository,
    RawMessageRepository, RequestRepository,
};
use crate::{Error, Result};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::sync::Arc;

pub struct MaintenanceService;

impl MaintenanceService {
    /// Auto-create partitions for the current and next month for audit_logs
    pub async fn auto_partition_audit_logs(db: &DatabaseConnection) -> Result<()> {
        let now = Utc::now();
        Self::create_monthly_partition(db, now).await?;

        // Calculate next month
        let next_month_date = if now.month() == 12 {
            Utc.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0).single()
        } else {
            Utc.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0)
                .single()
        };

        if let Some(date) = next_month_date {
            Self::create_monthly_partition(db, date).await?;
        }

        Ok(())
    }

    async fn create_monthly_partition(db: &DatabaseConnection, date: DateTime<Utc>) -> Result<()> {
        let year = date.year();
        let month = date.month();
        let partition_name = format!("audit_logs_{:04}_{:02}", year, month);

        let start_date = format!("{:04}-{:02}-01", year, month);
        let (end_year, end_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let end_date = format!("{:04}-{:02}-01", end_year, end_month);

        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} PARTITION OF audit_logs FOR VALUES FROM ('{}') TO ('{}')",
            partition_name, start_date, end_date
        );

        db.execute(Statement::from_string(db.get_database_backend(), sql))
            .await
            .map(|_| ())
            .map_err(Error::from)
    }

    /// Prune data older than the specified duration from all supported repositories
    pub async fn prune_all(
        cutoff: DateTime<Utc>,
        repos: MaintenanceRepositories,
    ) -> Result<PruneReport> {
        Ok(PruneReport {
            audit_logs: repos.audit_log.delete_before(&cutoff).await?,
            raw_messages: repos.raw_message.delete_before(&cutoff).await?,
            offers: repos.offer.delete_before(&cutoff).await?,
            requests: repos.request.delete_before(&cutoff).await?,
            matches: repos.match_repo.delete_before(&cutoff).await?,
            match_queue: repos.match_queue.delete_before(&cutoff).await?,
        })
    }
}

pub struct MaintenanceRepositories {
    pub audit_log: Arc<dyn AuditLogRepository>,
    pub raw_message: Arc<dyn RawMessageRepository>,
    pub offer: Arc<dyn OfferRepository>,
    pub request: Arc<dyn RequestRepository>,
    pub match_repo: Arc<dyn MatchRepository>,
    pub match_queue: Arc<dyn MatchQueueRepository>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct PruneReport {
    pub audit_logs: u64,
    pub raw_messages: u64,
    pub offers: u64,
    pub requests: u64,
    pub matches: u64,
    pub match_queue: u64,
}
