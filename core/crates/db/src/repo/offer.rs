//! Offer repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sea_orm::*;

use crate::entity::offer::{self, Entity as Offer, Status};
use crate::traits::OfferRepository;
use crate::{Error, Result};

/// SeaORM-based offer repository
pub struct SeaOrmOfferRepo {
    db: DatabaseConnection,
}

impl SeaOrmOfferRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl OfferRepository for SeaOrmOfferRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<offer::Model>> {
        Offer::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<offer::Model>> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .order_by_desc(offer::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<offer::Model>> {
        let pattern = format!("%{}%", query);
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::Medication.like(&pattern))
            .order_by_desc(offer::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_active(&self) -> Result<i64> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<offer::Model>> {
        let cutoff = Utc::now() - within;
        Offer::find()
            .filter(offer::Column::SourcePhone.eq(sender_phone))
            .filter(offer::Column::Medication.eq(medication))
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::CreatedAt.gte(cutoff))
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &offer::Model) -> Result<offer::Model> {
        let active: offer::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn update_status(&self, id: &str, status: Status) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now());
        active.update(&self.db).await.map_err(Error::from)
    }

    async fn find_semantic_duplicates(
        &self,
        embedding: &[f32],
        threshold: f64,
        within: Duration,
    ) -> Result<Vec<offer::Model>> {
        let cutoff = Utc::now() - within;
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        Offer::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM offers 
                WHERE status = 'ACTIVE' 
                AND content_embedding IS NOT NULL
                AND created_at >= $3
                AND 1 - (content_embedding <=> $1::vector) > $2
                ORDER BY content_embedding <=> $1::vector
                "#,
                [embedding_str.into(), threshold.into(), cutoff.into()],
            ))
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = Offer::delete_many()
            .filter(offer::Column::CreatedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
