//! Convex MedicationMappingRepository implementation

use std::sync::Arc;

use async_trait::async_trait;

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{MedicationMappingModel, MedicationMappingRepository};

/// Convex-backed medication mapping repository
pub struct ConvexMedicationMappingRepo {
    client: Arc<ConvexClient>,
}

impl ConvexMedicationMappingRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MedicationMappingRepository for ConvexMedicationMappingRepo {
    async fn save(&self, mapping: &MedicationMappingModel) -> Result<MedicationMappingModel> {
        // MedicationMappingModel: id (String), arabic_name, english_name, synonyms, embedding, created_at, updated_at
        let id: String = self
            .client
            .mutation(
                "medicationMappings:upsert",
                convex_args! {
                    "arabicName" => &mapping.arabic_name,
                    "englishName" => &mapping.english_name,
                    "synonyms" => mapping.synonyms.as_ref()
                },
            )
            .await?;

        let mut saved = mapping.clone();
        saved.id = id;
        Ok(saved)
    }

    async fn find_relevant(&self, query: &str, limit: i64) -> Result<Vec<MedicationMappingModel>> {
        self.client
            .query(
                "medicationMappings:search",
                convex_args! {
                    "query" => query,
                    "limit" => limit
                },
            )
            .await
    }

    async fn find_similar(
        &self,
        _embedding: &[f32],
        _limit: i64,
    ) -> Result<Vec<MedicationMappingModel>> {
        // TODO: Vector search not yet implemented in Convex
        Ok(vec![])
    }

    async fn get_all(&self, limit: i64, _offset: i64) -> Result<Vec<MedicationMappingModel>> {
        self.client
            .query("medicationMappings:list", convex_args! { "limit" => limit })
            .await
    }

    async fn count(&self) -> Result<i64> {
        let all: Vec<MedicationMappingModel> = self.get_all(10000, 0).await?;
        Ok(all.len() as i64)
    }

    async fn get_needing_embeddings(&self, _limit: i64) -> Result<Vec<MedicationMappingModel>> {
        // Return empty - Convex doesn't track embeddings yet
        Ok(vec![])
    }

    async fn count_needing_embeddings(&self) -> Result<i64> {
        Ok(0)
    }
}
