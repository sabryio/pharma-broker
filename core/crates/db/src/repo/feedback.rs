//! Feedback repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

use crate::entity::feedback_record::{self, Entity as FeedbackRecord};
use crate::traits::{FeedbackRepository, FeedbackStats};
use crate::{Error, Result};

/// SeaORM-based feedback repository
pub struct SeaOrmFeedbackRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmFeedbackRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FeedbackRepository for SeaOrmFeedbackRepo {
    async fn save(&self, model: &feedback_record::Model) -> Result<feedback_record::Model> {
        let active: feedback_record::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn get_by_match(&self, match_id: Uuid) -> Result<Vec<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::MatchId.eq(match_id))
            .order_by_desc(feedback_record::Column::CreatedAt)
            .all(&*self.db)
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
            .all(&*self.db)
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
            .count(&*self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn get_by_match_id(&self, match_id: Uuid) -> Result<Option<feedback_record::Model>> {
        FeedbackRecord::find()
            .filter(feedback_record::Column::MatchId.eq(match_id))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{
        TestDb, new_test_feedback, new_test_group, new_test_match, new_test_offer,
        new_test_raw_message, new_test_request,
    };
    use chrono::Duration;
    use sea_orm::EntityTrait;

    /// Helper to create feedback with all dependencies
    async fn create_feedback_with_deps(db: &TestDb, confirmed: bool) -> feedback_record::Model {
        use crate::entity::{group, match_, offer, raw_message, request};

        // Create group
        let group_am = new_test_group("test-group@g.us", "Test Group", true);
        group::Entity::insert(group_am).exec(&*db.db).await.ok();

        // Create raw messages
        let msg1 = new_test_raw_message();
        let msg1_id = msg1.id.clone().unwrap();
        raw_message::Entity::insert(msg1)
            .exec(&*db.db)
            .await
            .expect("Insert msg1");

        let msg2 = new_test_raw_message();
        let msg2_id = msg2.id.clone().unwrap();
        raw_message::Entity::insert(msg2)
            .exec(&*db.db)
            .await
            .expect("Insert msg2");

        // Create offer and request
        let offer_am = new_test_offer(&msg1_id);
        let offer_id = offer_am.id.clone().unwrap();
        offer::Entity::insert(offer_am)
            .exec(&*db.db)
            .await
            .expect("Insert offer");

        let request_am = new_test_request(&msg2_id);
        let request_id = request_am.id.clone().unwrap();
        request::Entity::insert(request_am)
            .exec(&*db.db)
            .await
            .expect("Insert request");

        // Create match
        let match_am = new_test_match(&offer_id, &request_id);
        let match_id = match_am.id.clone().unwrap();
        match_::Entity::insert(match_am)
            .exec(&*db.db)
            .await
            .expect("Insert match");

        // Create feedback
        let feedback_am = new_test_feedback(&match_id, confirmed);
        let feedback_id = feedback_am.id.clone().unwrap();
        feedback_record::Entity::insert(feedback_am)
            .exec(&*db.db)
            .await
            .expect("Insert feedback");

        feedback_record::Entity::find_by_id(feedback_id)
            .one(&*db.db)
            .await
            .expect("Find feedback")
            .expect("Feedback should exist")
    }

    #[tokio::test]
    async fn test_get_by_match_id_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmFeedbackRepo::new(db.db.clone());

        let feedback = create_feedback_with_deps(&db, true).await;

        let found = repo
            .get_by_match_id(&feedback.match_id)
            .await
            .expect("GetByMatchId");
        assert!(found.is_some(), "Should find feedback");
        assert_eq!(found.unwrap().id, feedback.id);
    }

    #[tokio::test]
    async fn test_get_by_match_id_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmFeedbackRepo::new(db.db.clone());

        let found = repo
            .get_by_match_id("non-existent")
            .await
            .expect("GetByMatchId");
        assert!(found.is_none(), "Should return None");
    }

    #[tokio::test]
    async fn test_count() {
        let db = TestDb::new().await;
        let repo = SeaOrmFeedbackRepo::new(db.db.clone());

        assert_eq!(repo.count().await.expect("Count"), 0, "Initially 0");

        create_feedback_with_deps(&db, true).await;
        create_feedback_with_deps(&db, false).await;
        create_feedback_with_deps(&db, true).await;

        assert_eq!(repo.count().await.expect("Count"), 3, "Should count 3");
    }

    #[tokio::test]
    async fn test_get_stats() {
        let db = TestDb::new().await;
        let repo = SeaOrmFeedbackRepo::new(db.db.clone());

        // Create 2 confirmed and 1 rejected
        create_feedback_with_deps(&db, true).await;
        create_feedback_with_deps(&db, true).await;
        create_feedback_with_deps(&db, false).await;

        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        let stats = repo.get_stats(start, end).await.expect("GetStats");

        assert_eq!(stats.total_feedback, 3);
        assert_eq!(stats.confirmed_count, 2);
        assert_eq!(stats.rejected_count, 1);
        assert!((stats.confirmation_rate - 0.666).abs() < 0.01);
    }
}
