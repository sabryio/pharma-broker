//! Offer Service - Medication supply offer management

use sea_orm::{prelude::Expr, *};
use uuid::Uuid;

use crate::entity::offer::{self, Entity as Offer, Status};
use crate::{Error, Result};

/// Service for offer operations
pub struct OfferService;

impl OfferService {
    /// Save a new offer
    pub async fn save(db: &DatabaseConnection, model: offer::ActiveModel) -> Result<offer::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get offer by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<offer::Model>> {
        Offer::find_by_id(id).one(db).await.map_err(Error::from)
    }

    /// Get all active offers
    pub async fn get_active(db: &DatabaseConnection) -> Result<Vec<offer::Model>> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .order_by_desc(offer::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get active offers for a specific medication
    pub async fn get_active_by_medication(
        db: &DatabaseConnection,
        medication: &str,
    ) -> Result<Vec<offer::Model>> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::Medication.eq(medication))
            .order_by_desc(offer::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count active offers
    pub async fn count_active(db: &DatabaseConnection) -> Result<u64> {
        Offer::find()
            .filter(offer::Column::Status.eq(Status::Active))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Find potential duplicates by source phone and medication
    pub async fn find_duplicates(
        db: &DatabaseConnection,
        source_phone: &str,
        medication: &str,
        hours: i64,
    ) -> Result<Vec<offer::Model>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        Offer::find()
            .filter(offer::Column::SourcePhone.eq(source_phone))
            .filter(offer::Column::Medication.eq(medication))
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::CreatedAt.gte(cutoff))
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Find semantic duplicates using vector similarity (requires raw SQL)
    pub async fn find_semantic_duplicates(
        db: &DatabaseConnection,
        embedding: &[f32],
        threshold: f64,
        limit: u64,
    ) -> Result<Vec<offer::Model>> {
        // Use raw SQL for vector similarity search
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
                AND 1 - (content_embedding <=> $1::vector) > $2
                ORDER BY content_embedding <=> $1::vector
                LIMIT $3
                "#,
                [
                    embedding_str.into(),
                    threshold.into(),
                    (limit as i64).into(),
                ],
            ))
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Update offer status
    pub async fn update_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: Status,
    ) -> Result<offer::Model> {
        let offer = Offer::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Offer not found: {}", id)))?;

        let mut active: offer::ActiveModel = offer.into();
        active.status = Set(status);
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await.map_err(Error::from)
    }

    /// Mark offer as matched
    pub async fn mark_matched(db: &DatabaseConnection, id: Uuid) -> Result<offer::Model> {
        Self::update_status(db, id, Status::Matched).await
    }

    /// Mark offer as expired
    pub async fn mark_expired(db: &DatabaseConnection, id: Uuid) -> Result<offer::Model> {
        Self::update_status(db, id, Status::Expired).await
    }

    /// Mark offer as duplicate
    pub async fn mark_duplicate(db: &DatabaseConnection, id: Uuid) -> Result<offer::Model> {
        Self::update_status(db, id, Status::Duplicate).await
    }

    /// Expire old offers (batch operation)
    pub async fn expire_old(db: &DatabaseConnection, hours: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let result = Offer::update_many()
            .col_expr(offer::Column::Status, Expr::value(Status::Expired))
            .col_expr(offer::Column::UpdatedAt, Expr::current_timestamp().into())
            .filter(offer::Column::Status.eq(Status::Active))
            .filter(offer::Column::CreatedAt.lt(cutoff))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete an offer
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Offer::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_default() {
        assert_eq!(Status::default(), Status::Active);
    }
}
