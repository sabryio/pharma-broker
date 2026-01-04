//! Janitor worker for background cleanup tasks
//!
//! Handles scheduled cleanup of old records, expired items, and database maintenance.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, interval};
use tracing::{error, info};

use crate::Result;
use crate::repository::{
    AuditLogRepository, DatabaseConnection, MaintenanceService, MatchQueueRepository,
    MatchRepository, OfferRepository, RawMessageRepository, RequestRepository,
};

/// Janitor configuration
#[derive(Debug, Clone)]
pub struct JanitorConfig {
    /// How often to run cleanup (default: 1 hour)
    pub interval: Duration,
    /// Days to retain raw messages (default: 30)
    pub raw_message_retention_days: u32,
    /// Days to retain processed offers (default: 90)
    pub offer_retention_days: u32,
    /// Days to retain processed requests (default: 90)
    pub request_retention_days: u32,
    /// Days to retain confirmed matches (default: 365)
    pub match_retention_days: u32,
    /// Days to retain audit logs (default: 365)
    pub audit_log_retention_days: u32,
    /// Enable cleanup (safety switch)
    pub enabled: bool,
}

impl Default for JanitorConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600), // 1 hour
            raw_message_retention_days: 14,      // Aggressive for raw messages
            offer_retention_days: 90,
            request_retention_days: 90,
            match_retention_days: 180,
            audit_log_retention_days: 365,
            enabled: true,
        }
    }
}

impl JanitorConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            interval: Duration::from_secs(
                std::env::var("JANITOR_INTERVAL_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(3600),
            ),
            raw_message_retention_days: std::env::var("JANITOR_RAW_MSG_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(14),
            offer_retention_days: std::env::var("JANITOR_OFFER_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            request_retention_days: std::env::var("JANITOR_REQUEST_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            match_retention_days: std::env::var("JANITOR_MATCH_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(180),
            audit_log_retention_days: std::env::var("JANITOR_AUDIT_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(365),
            enabled: std::env::var("JANITOR_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
        }
    }
}

/// Cleanup statistics
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    pub raw_messages_deleted: u64,
    pub offers_deleted: u64,
    pub requests_deleted: u64,
    pub matches_deleted: u64,
    pub match_queue_deleted: u64,
    pub audit_logs_deleted: u64,
    pub last_run: Option<Instant>,
    pub run_count: u64,
}

/// Janitor worker for scheduled cleanup
pub struct Janitor {
    config: JanitorConfig,
    db: Arc<DatabaseConnection>,
    repos: JanitorRepositories,
    stats: tokio::sync::RwLock<CleanupStats>,
}

pub struct JanitorRepositories {
    pub raw_message: Arc<dyn RawMessageRepository>,
    pub offer: Arc<dyn OfferRepository>,
    pub request: Arc<dyn RequestRepository>,
    pub match_repo: Arc<dyn MatchRepository>,
    pub match_queue: Arc<dyn MatchQueueRepository>,
    pub audit_log: Arc<dyn AuditLogRepository>,
}

impl Janitor {
    /// Create a new janitor worker
    pub fn new(
        config: JanitorConfig,
        db: Arc<DatabaseConnection>,
        repos: JanitorRepositories,
    ) -> Self {
        Self {
            config,
            db,
            repos,
            stats: tokio::sync::RwLock::new(CleanupStats::default()),
        }
    }

    /// Get current cleanup statistics
    pub async fn stats(&self) -> CleanupStats {
        self.stats.read().await.clone()
    }

    /// Run the janitor loop
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        if !self.config.enabled {
            info!("🧹 Janitor disabled, not starting");
            return;
        }

        info!(
            interval_secs = self.config.interval.as_secs(),
            raw_msg_days = self.config.raw_message_retention_days,
            offer_days = self.config.offer_retention_days,
            "🧹 Janitor started"
        );

        let mut ticker = interval(self.config.interval);
        // Skip immediate first tick
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("🧹 Janitor stopped gracefully");
                        break;
                    }
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.run_cleanup().await {
                        error!(error = %e, "Janitor cleanup failed");
                    }
                }
            }
        }
    }

    /// Run a single cleanup cycle
    pub async fn run_cleanup(&self) -> Result<()> {
        let start = Instant::now();
        info!("🧹 Starting Database Maintenance & Cleanup cycle");

        // 1. Automated Partitioning
        info!("📦 Checking audit_log partitions...");
        if let Err(e) = MaintenanceService::auto_partition_audit_logs(&self.db).await {
            error!(error = %e, "Failed to auto-partition audit logs");
        }

        // 2. Data Pruning
        // Calculate cutoff dates (using most conservative cutoff for unified pruning if needed,
        // but individual cutoffs are better)
        let now = chrono::Utc::now();

        // We use individual cutoffs for better control
        let raw_msg_cutoff =
            now - chrono::Duration::days(self.config.raw_message_retention_days as i64);
        let audit_cutoff =
            now - chrono::Duration::days(self.config.audit_log_retention_days as i64);

        // Initializing stats directly
        let raw_messages_deleted = self
            .repos
            .raw_message
            .delete_before(&raw_msg_cutoff)
            .await
            .unwrap_or(0);
        let audit_logs_deleted = self
            .repos
            .audit_log
            .delete_before(&audit_cutoff)
            .await
            .unwrap_or(0);

        // Use default retention (90 days) for offers/requests
        let default_cutoff = now - chrono::Duration::days(90);
        let offers_deleted = self
            .repos
            .offer
            .delete_before(&default_cutoff)
            .await
            .unwrap_or(0);
        let requests_deleted = self
            .repos
            .request
            .delete_before(&default_cutoff)
            .await
            .unwrap_or(0);

        let cycle_stats = CleanupStats {
            raw_messages_deleted,
            audit_logs_deleted,
            offers_deleted,
            requests_deleted,
            ..Default::default()
        };

        // Update cumulative stats
        {
            let mut stats = self.stats.write().await;
            stats.raw_messages_deleted += cycle_stats.raw_messages_deleted;
            stats.offers_deleted += cycle_stats.offers_deleted;
            stats.requests_deleted += cycle_stats.requests_deleted;
            stats.matches_deleted += cycle_stats.matches_deleted;
            stats.audit_logs_deleted += cycle_stats.audit_logs_deleted;
            stats.last_run = Some(Instant::now());
            stats.run_count += 1;
        }

        let elapsed = start.elapsed();
        info!(
            elapsed_ms = elapsed.as_millis(),
            raw_msgs = cycle_stats.raw_messages_deleted,
            audit_logs = cycle_stats.audit_logs_deleted,
            "🧹 Maintenance cycle complete"
        );

        Ok(())
    }
}
