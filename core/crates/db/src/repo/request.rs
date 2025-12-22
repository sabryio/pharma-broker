//! Request repository implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;

use crate::entity::offer::Status;
use crate::entity::request::{self, Entity as Request};
use crate::traits::RequestRepository;
use crate::{Error, Result};

/// SeaORM-based request repository
pub struct SeaOrmRequestRepo {
    db: DatabaseConnection,
}

impl SeaOrmRequestRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RequestRepository for SeaOrmRequestRepo {
    async fn get_by_id(&self, id: &str) -> Result<Option<request::Model>> {
        Request::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<request::Model>> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .order_by_desc(request::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn search(&self, query: &str, limit: i64, _offset: i64) -> Result<Vec<request::Model>> {
        let pattern = format!("%{}%", query);
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::Medication.like(&pattern))
            .order_by_desc(request::Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count_active(&self) -> Result<i64> {
        Request::find()
            .filter(request::Column::Status.eq(Status::Active))
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }

    async fn find_recent_duplicate(
        &self,
        params: crate::params::FindDuplicateParams<'_>,
    ) -> Result<Option<request::Model>> {
        let cutoff = Utc::now() - params.within;
        Request::find()
            .filter(request::Column::SourcePhone.eq(params.sender_phone))
            .filter(request::Column::Medication.eq(params.medication))
            .filter(request::Column::Status.eq(Status::Active))
            .filter(request::Column::CreatedAt.gte(cutoff))
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &request::Model) -> Result<request::Model> {
        let active: request::ActiveModel = model.clone().into();
        active.insert(&self.db).await.map_err(Error::from)
    }

    async fn update_status(&self, id: &str, status: Status) -> Result<request::Model> {
        let request = Request::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Request not found: {}", id)))?;

        let mut active: request::ActiveModel = request.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now());
        active.update(&self.db).await.map_err(Error::from)
    }

    async fn find_semantic_duplicates(
        &self,
        params: crate::params::SemanticDuplicateParams<'_>,
    ) -> Result<Vec<request::Model>> {
        let cutoff = Utc::now() - params.within;
        let embedding_str = format!(
            "[{}]",
            params
                .embedding
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
                AND created_at >= $3
                AND 1 - (content_embedding <=> $1::vector) > $2
                ORDER BY content_embedding <=> $1::vector
                "#,
                [
                    embedding_str.into(),
                    params.similarity_threshold.into(),
                    cutoff.into(),
                ],
            ))
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = Request::delete_many()
            .filter(request::Column::CreatedAt.lt(*cutoff))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }
}
