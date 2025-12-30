//! Convex OfferRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::params::{FindDuplicateParams, SemanticDuplicateParams};
use crate::traits::{ItemStatus, OfferModel, OfferRepository};

/// Convex-backed offer repository
pub struct ConvexOfferRepo {
    client: Arc<ConvexClient>,
}

impl ConvexOfferRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl OfferRepository for ConvexOfferRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<OfferModel>> {
        self.client
            .query("offers:get", convex_args! { "id" => id })
            .await
    }

    async fn get_active(&self, limit: i64, _offset: i64) -> Result<Vec<OfferModel>> {
        self.client
            .query(
                "offers:list",
                convex_args! {
                    "status" => "ACTIVE",
                    "limit" => limit
                },
            )
            .await
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<OfferModel>> {
        self.client
            .query(
                "offers:search",
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
            .query("offers:countActive", convex_args! {})
            .await
    }

    async fn find_recent_duplicate(
        &self,
        params: FindDuplicateParams<'_>,
    ) -> Result<Option<OfferModel>> {
        self.client
            .query(
                "offers:findRecentDuplicate",
                convex_args! {
                    "senderPhone" => params.sender_phone,
                    "medication" => params.medication,
                    "withinMs" => params.within.num_milliseconds()
                },
            )
            .await
    }

    async fn save(&self, offer: &OfferModel) -> Result<OfferModel> {
        let price_f64 = offer.price.as_ref().and_then(|p| {
            use rust_decimal::prelude::ToPrimitive;
            p.to_f64()
        });
        let quantity_f64 = offer.quantity.as_ref().and_then(|q| {
            use rust_decimal::prelude::ToPrimitive;
            q.to_f64()
        });
        let expiry_str = offer.expiry_date.map(|d| d.to_string());

        let id: String = self
            .client
            .mutation(
                "offers:create",
                convex_args! {
                    "rawMessageId" => &offer.raw_message_id,
                    "sourcePhone" => &offer.source_phone,
                    "sourceName" => offer.source_name.as_ref(),
                    "sourceGroup" => &offer.source_group,
                    "medication" => &offer.medication,
                    "medicationRaw" => &offer.medication_raw,
                    "quantity" => quantity_f64,
                    "unit" => offer.unit.as_ref(),
                    "price" => price_f64,
                    "currency" => offer.currency.as_ref(),
                    "expiryDate" => expiry_str,
                    "batchNumber" => offer.batch_number.as_ref(),
                    "notes" => offer.notes.as_ref(),
                    "status" => format!("{:?}", offer.status),
                    "urgencyLevel" => format!("{:?}", offer.urgency_level),
                    "expiryInfo" => offer.expiry_info.as_ref(),
                    "aiConfidence" => offer.ai_confidence,
                    "contentEmbedding" => offer.content_embedding.as_ref().map(|v| v.to_vec())
                },
            )
            .await?;

        let mut saved = offer.clone();
        saved.id = id;
        Ok(saved)
    }

    async fn update_status(&self, id: &str, status: ItemStatus) -> Result<OfferModel> {
        self.client
            .mutation(
                "offers:updateStatus",
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
    ) -> Result<Vec<OfferModel>> {
        // Convert f32 embedding to f64 for Convex
        let embedding_vec: Vec<f64> = params.embedding.iter().map(|x| *x as f64).collect();

        self.client
            .action(
                "offers:searchSimilar",
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
                "offers:deleteBefore",
                convex_args! { "cutoff" => cutoff.timestamp_millis() },
            )
            .await
    }
}
