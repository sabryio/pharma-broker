//! Convex GroupRepository implementation

use std::sync::Arc;

use async_trait::async_trait;

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{GroupModel, GroupRepository};

/// Convex-backed group repository
pub struct ConvexGroupRepo {
    client: Arc<ConvexClient>,
}

impl ConvexGroupRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl GroupRepository for ConvexGroupRepo {
    async fn get_all(&self) -> Result<Vec<GroupModel>> {
        self.client.query("groups:list", convex_args!()).await
    }

    async fn get_by_jid(&self, jid: &str) -> Result<Option<GroupModel>> {
        self.client
            .query("groups:getByJid", convex_args! { "jid" => jid })
            .await
    }

    async fn is_monitored(&self, jid: &str) -> Result<bool> {
        let group: Option<GroupModel> = self
            .client
            .query("groups:getByJid", convex_args! { "jid" => jid })
            .await?;
        // GroupModel uses `monitored` field (not `is_monitored`)
        Ok(group.map(|g| g.monitored).unwrap_or(false))
    }

    async fn get_monitored(&self) -> Result<Vec<GroupModel>> {
        self.client
            .query("groups:listMonitored", convex_args!())
            .await
    }

    async fn save(&self, group: &GroupModel) -> Result<GroupModel> {
        self.client
            .mutation(
                "groups:upsert",
                convex_args! {
                    "jid" => &group.jid,
                    "name" => &group.name,
                    "monitored" => group.monitored
                },
            )
            .await
    }

    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()> {
        self.client
            .mutation_void(
                "groups:setMonitored",
                convex_args! { "jid" => jid, "monitored" => monitored },
            )
            .await
    }

    async fn delete(&self, jid: &str) -> Result<bool> {
        self.client
            .mutation_void("groups:remove", convex_args! { "jid" => jid })
            .await?;
        Ok(true)
    }

    async fn update_last_message(&self, jid: &str) -> Result<()> {
        self.client
            .mutation_void("groups:updateLastMessage", convex_args! { "jid" => jid })
            .await
    }

    async fn increment_message_count(&self, jid: &str) -> Result<()> {
        self.client
            .mutation_void(
                "groups:incrementMessageCount",
                convex_args! { "jid" => jid },
            )
            .await
    }
}
