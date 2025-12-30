//! Convex FeedbackRepository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::client::ConvexClient;
use crate::Result;
use crate::convex_args;
use crate::traits::{FeedbackModel, FeedbackRepository, FeedbackStats};

/// Convex-backed feedback repository
pub struct ConvexFeedbackRepo {
    client: Arc<ConvexClient>,
}

impl ConvexFeedbackRepo {
    pub fn new(client: Arc<ConvexClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl FeedbackRepository for ConvexFeedbackRepo {
    async fn save(&self, record: &FeedbackModel) -> Result<FeedbackModel> {
        // FeedbackModel: id (Uuid), match_id, user_id, confirmed, medication_score, dosage_score, quantity_score, price_score, recency_score, total_score, created_at
        let _id: String = self
            .client
            .mutation(
                "feedback:save",
                convex_args! {
                    "matchId" => &record.match_id,
                    "userId" => &record.user_id,
                    "confirmed" => record.confirmed,
                    "medicationScore" => record.medication_score,
                    "dosageScore" => record.dosage_score,
                    "quantityScore" => record.quantity_score,
                    "priceScore" => record.price_score,
                    "recencyScore" => record.recency_score,
                    "totalScore" => record.total_score
                },
            )
            .await?;

        Ok(record.clone())
    }

    async fn get_by_match(&self, match_id: &str) -> Result<Vec<FeedbackModel>> {
        self.client
            .query(
                "feedback:getByMatch",
                convex_args! { "matchId" => match_id },
            )
            .await
    }

    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FeedbackModel>> {
        self.client
            .query(
                "feedback:getByDateRange",
                convex_args! {
                    "start" => start.timestamp_millis(),
                    "end" => end.timestamp_millis()
                },
            )
            .await
    }

    async fn get_stats(&self, _start: DateTime<Utc>, _end: DateTime<Utc>) -> Result<FeedbackStats> {
        // Return default stats - would need custom Convex function
        Ok(FeedbackStats::default())
    }

    async fn count(&self) -> Result<i64> {
        self.client.query("feedback:count", convex_args!()).await
    }

    async fn get_by_match_id(&self, match_id: &str) -> Result<Option<FeedbackModel>> {
        let records: Vec<FeedbackModel> = self.get_by_match(match_id).await?;
        Ok(records.into_iter().next())
    }
}
