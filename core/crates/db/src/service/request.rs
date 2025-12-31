//! Request Service - Medication demand request management

use sea_orm::{prelude::Expr, *};
use uuid::Uuid;

use crate::entity::offer::Status;
use crate::entity::request::{self, Entity as Request};
use crate::{Error, Result};

/// Service for request operations
pub struct RequestService;

impl RequestService {
    /// Save a new request
    pub async fn save(
        db: &DatabaseConnection,
        model: request::ActiveModel,
    ) -> Result<request::Model> {
        model.insert(db).await.map_err(Error::from)
    }

    /// Get request by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<request::Model>> {
        Request::find_by_id(id).one(db).await.map_err(Error::from)
    }

    /// Get all active requests
    pub async fn get_active(db: &DatabaseConnection) -> Result<Vec<request::Model>> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .order_by_desc(request::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get active requests for a specific medication
    pub async fn get_active_by_medication(
        db: &DatabaseConnection,
        medication: &str,
    ) -> Result<Vec<request::Model>> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::Medication.eq(medication))
            .order_by_desc(request::Column::CreatedAt)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Count active requests
    pub async fn count_active(db: &DatabaseConnection) -> Result<u64> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .count(db)
            .await
            .map_err(Error::from)
    }

    /// Find potential duplicates by participant and medication
    pub async fn find_duplicates(
        db: &DatabaseConnection,
        participant_id: Uuid,
        medication: &str,
        hours: i64,
    ) -> Result<Vec<request::Model>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        Request::find()
            .filter(request::Column::ParticipantId.eq(participant_id))
            .filter(request::Column::Medication.eq(medication))
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::CreatedAt.gte(cutoff))
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Find semantic duplicates using vector similarity
    pub async fn find_semantic_duplicates(
        db: &DatabaseConnection,
        embedding: &[f32],
        threshold: f64,
        limit: u64,
    ) -> Result<Vec<request::Model>> {
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        Request::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM requests 
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

    /// Update request status
    pub async fn update_status(
        db: &DatabaseConnection,
        id: Uuid,
        status: Status,
    ) -> Result<request::Model> {
        let request = Request::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Request not found: {}", id)))?;

        let mut active: request::ActiveModel = request.into();
        active.status = Set(status);
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await.map_err(Error::from)
    }

    /// Mark request as matched
    pub async fn mark_matched(db: &DatabaseConnection, id: Uuid) -> Result<request::Model> {
        Self::update_status(db, id, Status::Matched).await
    }

    /// Mark request as expired
    pub async fn mark_expired(db: &DatabaseConnection, id: Uuid) -> Result<request::Model> {
        Self::update_status(db, id, Status::Expired).await
    }

    /// Mark request as duplicate
    pub async fn mark_duplicate(db: &DatabaseConnection, id: Uuid) -> Result<request::Model> {
        Self::update_status(db, id, Status::Duplicate).await
    }

    /// Expire old requests (batch operation)
    pub async fn expire_old(db: &DatabaseConnection, hours: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let result = Request::update_many()
            .col_expr(request::Column::Status, Expr::value(Status::Expired))
            .col_expr(request::Column::UpdatedAt, Expr::current_timestamp().into())
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::CreatedAt.lt(cutoff))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete a request
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Request::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_reuse() {
        // Request reuses Status from offer
        assert_eq!(Status::default(), Status::Active);
    }
}
