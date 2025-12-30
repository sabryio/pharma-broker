//! Convex RawMessageRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{RawMessageModel, RawMessageRepository};

/// Convex-backed raw message repository
pub struct ConvexRawMessageRepo {
    client: Arc<ConvexClient>,
}

impl ConvexRawMessageRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl RawMessageRepository for ConvexRawMessageRepo {
    async fn save(&self, message: &RawMessageModel) -> Result<RawMessageModel> {
        // RawMessageModel: id, external_id, group_jid, group_name, sender_jid, sender_phone, sender_name,
        // content, timestamp, processed_at, error, reply_to_id, reply_to_content, reply_to_sender, created_at
        let id: String = self
            .client
            .mutation(
                "rawMessages:save",
                convex_args! {
                    "externalId" => message.external_id.as_ref(),
                    "groupJid" => &message.group_jid,
                    "groupName" => &message.group_name,
                    "senderJid" => &message.sender_jid,
                    "senderPhone" => message.sender_phone.as_ref(),
                    "senderName" => message.sender_name.as_ref(),
                    "content" => &message.content,
                    "timestamp" => message.timestamp.timestamp_millis(),
                    "replyToId" => message.reply_to_id.as_ref(),
                    "replyToContent" => message.reply_to_content.as_ref(),
                    "replyToSender" => message.reply_to_sender.as_ref()
                },
            )
            .await?;

        let mut saved = message.clone();
        saved.id = id;
        Ok(saved)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<RawMessageModel>> {
        self.client
            .query(
                "rawMessages:getByExternalId",
                convex_args! { "externalId" => id },
            )
            .await
    }

    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<RawMessageModel>> {
        self.client
            .query(
                "rawMessages:getUnprocessed",
                convex_args! { "limit" => limit },
            )
            .await
    }

    async fn mark_processed(&self, id: &str, error: Option<&str>) -> Result<RawMessageModel> {
        self.client
            .mutation(
                "rawMessages:markProcessed",
                convex_args! {
                    "id" => id,
                    "error" => error
                },
            )
            .await
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        self.client
            .mutation(
                "rawMessages:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
