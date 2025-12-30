//! Convex WeightHistoryRepository implementation

use std::sync::Arc;

use async_trait::async_trait;

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{WeightHistoryModel, WeightHistoryRepository};

/// Convex-backed weight history repository
pub struct ConvexWeightHistoryRepo {
    client: Arc<ConvexClient>,
}

impl ConvexWeightHistoryRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl WeightHistoryRepository for ConvexWeightHistoryRepo {
    async fn save(&self, history: &WeightHistoryModel) -> Result<WeightHistoryModel> {
        // WeightHistoryModel: id (Uuid), medication_weight, dosage_weight, quantity_weight, price_weight, recency_weight, source, sample_count, created_at
        let _id: String = self
            .client
            .mutation(
                "weightHistory:save",
                convex_args! {
                    "medicationWeight" => history.medication_weight,
                    "dosageWeight" => history.dosage_weight,
                    "quantityWeight" => history.quantity_weight,
                    "priceWeight" => history.price_weight,
                    "recencyWeight" => history.recency_weight,
                    "source" => &history.source,
                    "sampleCount" => history.sample_count
                },
            )
            .await?;

        Ok(history.clone())
    }

    async fn get_current(&self) -> Result<Option<WeightHistoryModel>> {
        self.client
            .query("weightHistory:getCurrent", convex_args!())
            .await
    }

    async fn get_history(&self, limit: i64) -> Result<Vec<WeightHistoryModel>> {
        self.client
            .query(
                "weightHistory:getHistory",
                convex_args! { "limit" => limit },
            )
            .await
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<WeightHistoryModel>> {
        self.client
            .query("weightHistory:getById", convex_args! { "id" => id })
            .await
    }

    async fn count(&self) -> Result<i64> {
        self.client
            .query("weightHistory:count", convex_args!())
            .await
    }
}
