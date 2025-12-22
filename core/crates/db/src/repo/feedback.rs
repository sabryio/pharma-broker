//! Feedback repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::feedback_record::{self, Entity as FeedbackRecord};
use crate::traits::{FeedbackRepository, FeedbackStats};
use crate::{Error, Result};

/// SeaORM-based feedback repository
pub struct SeaOrmFeedbackRepo {
    db: DatabaseConnection,
}

impl SeaOrmFeedbackRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FeedbackRepository for SeaOrmFeedbackRepo {
    async fn save(&self, model: &feedback_record::Model) -> Result<feedback_record::Model> {
        let active: feedback_record::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn get_by_match(&self, match_id: &str) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::MatchId.eq(match_id))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::CreatedAt.gte(start))
            .filter(feedback_record::Column::CreatedAt.lte(end))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<FeedbackStats> {
        let records = self.get_by_date_range(start, end).await?;

        let total_feedback = records.len() as i64;
        let confirmed: Vec<_> = records.iter().filter(|r| r.confirmed).collect();
        let rejected: Vec<_> = records.iter().filter(|r| !r.confirmed).collect();

        let confirmed_count = confirmed.len() as i64;
        let rejected_count = rejected.len() as i64;

        let confirmation_rate = if total_feedback > 0 {
            confirmed_count as f64 / total_feedback as f64
        } else {
            0.0
        };

        // Helper to calculate average of a field
        let avg =
            |records: &[&feedback_record::Model], f: fn(&feedback_record::Model) -> f64| -> f64 {
                if records.is_empty() {
                    0.0
                } else {
                    records.iter().map(|r| f(r)).sum::<f64>() / records.len() as f64
                }
            };

        let confirmed_avg_medication = avg(&confirmed, |r| r.medication_score);
        let rejected_avg_medication = avg(&rejected, |r| r.medication_score);
        let confirmed_avg_dosage = avg(&confirmed, |r| r.dosage_score);
        let rejected_avg_dosage = avg(&rejected, |r| r.dosage_score);
        let confirmed_avg_quantity = avg(&confirmed, |r| r.quantity_score);
        let rejected_avg_quantity = avg(&rejected, |r| r.quantity_score);
        let confirmed_avg_price = avg(&confirmed, |r| r.price_score);
        let rejected_avg_price = avg(&rejected, |r| r.price_score);
        let confirmed_avg_recency = avg(&confirmed, |r| r.recency_score);
        let rejected_avg_recency = avg(&rejected, |r| r.recency_score);
        let confirmed_avg_total = avg(&confirmed, |r| r.total_score);
        let rejected_avg_total = avg(&rejected, |r| r.total_score);

        Ok(FeedbackStats {
            total_feedback,
            confirmed_count,
            rejected_count,
            avg_confirmed_score: confirmed_avg_total,
            avg_rejected_score: rejected_avg_total,
            confirmation_rate,
            confirmed_avg_medication,
            rejected_avg_medication,
            medication_diff: confirmed_avg_medication - rejected_avg_medication,
            confirmed_avg_dosage,
            rejected_avg_dosage,
            dosage_diff: confirmed_avg_dosage - rejected_avg_dosage,
            confirmed_avg_quantity,
            rejected_avg_quantity,
            quantity_diff: confirmed_avg_quantity - rejected_avg_quantity,
            confirmed_avg_price,
            rejected_avg_price,
            price_diff: confirmed_avg_price - rejected_avg_price,
            confirmed_avg_recency,
            rejected_avg_recency,
            recency_diff: confirmed_avg_recency - rejected_avg_recency,
            confirmed_avg_total,
            rejected_avg_total,
        })
    }

    async fn count(&self) -> Result<i64> {
        FeedbackRecord::find()
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn get_by_match_id(&self, match_id: &str) -> Result<Option<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::MatchId.eq(match_id))
            .one(&self.db)
            .await
            .map_err(Error::from)
    }
}
