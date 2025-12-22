//! Janitor worker for background cleanup tasks
//!
//! Handles scheduled cleanup of old records, expired items, and database maintenance.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, interval};
use tracing::{error, info, warn};

use crate::Result;
use crate::repository::{
    AuditLogRepository, MatchQueueRepository, OfferRepository, RawMessageRepository,
    RequestRepository,
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
            raw_message_retention_days: 30,
            offer_retention_days: 90,
            request_retention_days: 90,
            match_retention_days: 365,
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
                .unwrap_or(30),
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
                .unwrap_or(365),
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
    pub audit_logs_deleted: u64,
    pub last_run: Option<Instant>,
    pub run_count: u64,
}

/// Janitor worker for scheduled cleanup
pub struct Janitor {
    config: JanitorConfig,
    raw_message_repo: Arc<dyn RawMessageRepository>,
    offer_repo: Arc<dyn OfferRepository>,
    request_repo: Arc<dyn RequestRepository>,
    match_repo: Arc<dyn MatchQueueRepository>,
    audit_log_repo: Arc<dyn AuditLogRepository>,
    stats: tokio::sync::RwLock<CleanupStats>,
}

impl Janitor {
    /// Create a new janitor worker
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: JanitorConfig,
        raw_message_repo: Arc<dyn RawMessageRepository>,
        offer_repo: Arc<dyn OfferRepository>,
        request_repo: Arc<dyn RequestRepository>,
        match_repo: Arc<dyn MatchQueueRepository>,
        audit_log_repo: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            config,
            raw_message_repo,
            offer_repo,
            request_repo,
            match_repo,
            audit_log_repo,
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
        info!("🧹 Starting cleanup cycle");

        // Calculate cutoff dates
        let now = chrono::Utc::now();
        let raw_msg_cutoff =
            now - chrono::Duration::days(self.config.raw_message_retention_days as i64);
        let offer_cutoff = now - chrono::Duration::days(self.config.offer_retention_days as i64);
        let request_cutoff =
            now - chrono::Duration::days(self.config.request_retention_days as i64);
        let match_cutoff = now - chrono::Duration::days(self.config.match_retention_days as i64);
        let audit_cutoff =
            now - chrono::Duration::days(self.config.audit_log_retention_days as i64);

        let mut cycle_stats = CleanupStats::default();

        // Clean raw messages
        match self.raw_message_repo.delete_before(&raw_msg_cutoff).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Deleted old raw messages");
                }
                cycle_stats.raw_messages_deleted = count;
            }
            Err(e) => warn!(error = %e, "Failed to clean raw messages"),
        }

        // Clean offers
        match self.offer_repo.delete_before(&offer_cutoff).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Deleted old offers");
                }
                cycle_stats.offers_deleted = count;
            }
            Err(e) => warn!(error = %e, "Failed to clean offers"),
        }

        // Clean requests
        match self.request_repo.delete_before(&request_cutoff).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Deleted old requests");
                }
                cycle_stats.requests_deleted = count;
            }
            Err(e) => warn!(error = %e, "Failed to clean requests"),
        }

        // Clean matches
        match self.match_repo.delete_before(&match_cutoff).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Deleted old matches");
                }
                cycle_stats.matches_deleted = count;
            }
            Err(e) => warn!(error = %e, "Failed to clean matches"),
        }

        // Clean audit logs
        match self.audit_log_repo.delete_before(&audit_cutoff).await {
            Ok(count) => {
                if count > 0 {
                    info!(count, "Deleted old audit logs");
                }
                cycle_stats.audit_logs_deleted = count;
            }
            Err(e) => warn!(error = %e, "Failed to clean audit logs"),
        }

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
            offers = cycle_stats.offers_deleted,
            requests = cycle_stats.requests_deleted,
            matches = cycle_stats.matches_deleted,
            "🧹 Cleanup cycle complete"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = JanitorConfig::default();
        assert_eq!(config.interval, Duration::from_secs(3600));
        assert_eq!(config.raw_message_retention_days, 30);
        assert_eq!(config.offer_retention_days, 90);
        assert!(config.enabled);
    }

    #[test]
    fn test_cleanup_stats_default() {
        let stats = CleanupStats::default();
        assert_eq!(stats.run_count, 0);
        assert!(stats.last_run.is_none());
    }
}
