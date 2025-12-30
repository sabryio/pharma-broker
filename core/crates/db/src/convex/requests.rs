//! Convex RequestRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::params::{FindDuplicateParams, SemanticDuplicateParams};
use crate::traits::{ItemStatus, RequestModel, RequestRepository};

/// Convex-backed request repository
pub struct ConvexRequestRepo {
    client: Arc<ConvexClient>,
}

impl ConvexRequestRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl RequestRepository for ConvexRequestRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<RequestModel>> {
        self.client
            .query("requests:get", convex_args! { "id" => id })
            .await
    }

    async fn get_active(&self, limit: i64, _offset: i64) -> Result<Vec<RequestModel>> {
        self.client
            .query(
                "requests:list",
                convex_args! {
                    "status" => "ACTIVE",
                    "limit" => limit
                },
            )
            .await
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<RequestModel>> {
        self.client
            .query(
                "requests:search",
                convex_args! {
                    "query" => query,
                    "limit" => limit,
                    "status" => "ACTIVE"
                },
            )
            .await
    }

    async fn count_active(&self) -> Result<i64> {
        self.client
            .query("requests:countActive", convex_args! {})
            .await
    }

    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<RequestModel>> {
        self.client
            .query(
                "requests:findRecentDuplicate",
                convex_args! {
                    "senderPhone" => params.sender_phone,
                    "medication" => params.medication,
                    "withinMs" => params.within.num_milliseconds()
                },
            )
            .await
    }

    async fn save(&self, request: &RequestModel) -> Result<RequestModel> {
        // ... (no changes needed)
        let max_price_f64 = request.max_price.as_ref().and_then(|p| {
            use rust_decimal::prelude::ToPrimitive;
            p.to_f64()
        });
        let quantity_f64 = request.quantity.as_ref().and_then(|q| {
            use rust_decimal::prelude::ToPrimitive;
            q.to_f64()
        });

        let id: String = self
            .client
            .mutation(
                "requests:create",
                convex_args! {
                    "rawMessageId" => &request.raw_message_id,
                    "sourcePhone" => &request.source_phone,
                    "sourceName" => request.source_name.as_ref(),
                    "sourceGroup" => &request.source_group,
                    "medication" => &request.medication,
                    "medicationRaw" => &request.medication_raw,
                    "quantity" => quantity_f64,
                    "unit" => request.unit.as_ref(),
                    "maxPrice" => max_price_f64,
                    "currency" => request.currency.as_ref(),
                    "urgencyLevel" => format!("{:?}", request.urgency_level),
                    "expiryRequirement" => request.expiry_requirement.as_ref(),
                    "aiConfidence" => request.ai_confidence,
                    "notes" => request.notes.as_ref(),
                    "status" => format!("{:?}", request.status),
                    "contentEmbedding" => request.content_embedding.as_ref().map(|v| v.to_vec())
                },
            )
            .await?;

        let mut saved = request.clone();
        saved.id = id;
        Ok(saved)
    }

    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<RequestModel> {
        self.client
            .mutation(
                "requests:updateStatus",
                convex_args! {
                    "id" => id,
                    "status" => format!("{:?}", status)
                },
            )
            .await
    }

    async fn find_semantic_duplicates(
        &self,
        params: SemanticDuplicateParams<'_>,
    ) -> Result<Vec<RequestModel>> {
        // Convert f32 embedding to f64 for Convex
        let embedding_vec: Vec<f64> = params.embedding.iter().map(|x| *x as f64).collect();

        self.client
            .action(
                "requests:searchSimilar",
                convex_args! {
                    "embedding" => embedding_vec,
                    "limit" => 10i64,
                    "statusFilter" => "ACTIVE",
                    "withinMs" => params.within.num_milliseconds(),
                    "similarityThreshold" => params.similarity_threshold
                },
            )
            .await
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        self.client
            .mutation(
                "requests:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
