//! Convex AuditLogRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::params::AuditByEntityParams;
use crate::traits::{AuditLogModel, AuditLogRepository};

/// Convex-backed audit log repository
pub struct ConvexAuditLogRepo {
    client: Arc<ConvexClient>,
}

impl ConvexAuditLogRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AuditLogRepository for ConvexAuditLogRepo {
    async fn save(&self, log: &AuditLogModel) -> Result<AuditLogModel> {
        // AuditLogModel: id (Uuid), action, entity_type, entity_id, actor, details, ip_address, user_agent, created_at
        let _id: String = self
            .client
            .mutation(
                "auditLogs:log",
                convex_args! {
                    "entityType" => &log.entity_type,
                    "entityId" => &log.entity_id,
                    "action" => &log.action,
                    "actor" => &log.actor,
                    "details" => log.details.as_ref(),
                    "ipAddress" => log.ip_address.as_ref(),
                    "userAgent" => log.user_agent.as_ref()
                },
            )
            .await?;

        // Return the log as-is since Convex generates a different ID format
        Ok(log.clone())
    }

    async fn get_by_entity(&self, params: AuditByEntityParams<'_>) -> Result<Vec<AuditLogModel>> {
        self.client
            .query(
                "auditLogs:getByEntity",
                convex_args! {
                    "entityType" => params.entity_type,
                    "entityId" => params.entity_id,
                    "limit" => params.limit
                },
            )
            .await
    }

    async fn get_by_actor(&self, actor: &str, limit: i64) -> Result<Vec<AuditLogModel>> {
        self.client
            .query(
                "auditLogs:getByActor",
                convex_args! {
                    "actor" => actor,
                    "limit" => limit
                },
            )
            .await
    }

    async fn get_by_action(&self, action: &str, limit: i64) -> Result<Vec<AuditLogModel>> {
        self.client
            .query(
                "auditLogs:getByAction",
                convex_args! {
                    "action" => action,
                    "limit" => limit
                },
            )
            .await
    }

    async fn get_recent(&self, limit: i64, _offset: i64) -> Result<Vec<AuditLogModel>> {
        self.client
            .query("auditLogs:getRecent", convex_args! { "limit" => limit })
            .await
    }

    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<AuditLogModel>> {
        self.client
            .query(
                "auditLogs:getByDateRange",
                convex_args! {
                    "start" => start.timestamp_millis(),
                    "end" => end.timestamp_millis(),
                    "limit" => limit
                },
            )
            .await
    }

    async fn count(&self) -> Result<i64> {
        self.client.query("auditLogs:count", convex_args!()).await
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        self.client
            .mutation(
                "auditLogs:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
