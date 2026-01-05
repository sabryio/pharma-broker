//! RawMessage repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::{Expr, Func};
use sea_orm::*;
use uuid::Uuid;

use crate::entity::raw_message::{self, Entity as RawMessage};
use crate::params::{ProcessingStatus, RawMessageQueryParams, RawMessageSortField, SortOrder};
use crate::traits::RawMessageRepository;
use crate::{Error, Result};

/// SeaORM-based raw message repository
pub struct SeaOrmRawMessageRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmRawMessageRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Apply common filters to a raw message query based on params
    fn apply_filters(
        query: Select<RawMessage>,
        params: &RawMessageQueryParams,
    ) -> Select<RawMessage> {
        let mut query = query;

        // Apply status filter
        match params.get_status() {
            ProcessingStatus::All => {}
            ProcessingStatus::Processed => {
                query = query
                    .filter(raw_message::Column::ProcessedAt.is_not_null())
                    .filter(raw_message::Column::Error.is_null());
            }
            ProcessingStatus::Unprocessed => {
                query = query.filter(raw_message::Column::ProcessedAt.is_null());
            }
            ProcessingStatus::Error => {
                query = query.filter(raw_message::Column::Error.is_not_null());
            }
        }

        // Apply search filter (case-insensitive content search)
        if let Some(search) = params.search.as_ref().filter(|s| !s.is_empty()) {
            let search_pattern = format!("%{}%", search.to_lowercase());
            query = query.filter(
                Expr::expr(Func::lower(Expr::col(raw_message::Column::Content)))
                    .like(&search_pattern),
            );
        }

        // Apply date range filters
        if let Some(start_date) = params.start_date {
            query = query.filter(raw_message::Column::Timestamp.gte(start_date));
        }
        if let Some(end_date) = params.end_date {
            query = query.filter(raw_message::Column::Timestamp.lte(end_date));
        }

        // Apply group filter
        if let Some(group_id) = params.group_id {
            query = query.filter(raw_message::Column::GroupId.eq(group_id));
        }

        // Apply participant filter
        if let Some(participant_id) = params.participant_id {
            query = query.filter(raw_message::Column::ParticipantId.eq(participant_id));
        }

        query
    }
}

#[async_trait]
impl RawMessageRepository for SeaOrmRawMessageRepo {
    async fn save(&self, model: &raw_message::Model) -> Result<raw_message::Model> {
        let active: raw_message::ActiveModel = model.clone().into();
        active.insert(&*self.db).await.map_err(Error::from)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<raw_message::Model>> {
        RawMessage::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_unprocessed(&self, limit: i64) -> Result<Vec<raw_message::Model>> {
        RawMessage::find()
            .filter(raw_message::Column::ProcessedAt.is_null())
            .order_by_asc(raw_message::Column::Timestamp)
            .limit(limit as u64)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn mark_processed(&self, id: Uuid, error: Option<&str>) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.processed_at = Set(Some(Utc::now()));
        active.error = Set(error.map(|e| e.to_string()));
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn reset_for_reprocessing(&self, id: Uuid) -> Result<raw_message::Model> {
        let msg = RawMessage::find_by_id(id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("RawMessage not found: {}", id)))?;

        let mut active: raw_message::ActiveModel = msg.into();
        active.processed_at = Set(None);
        active.error = Set(None);
        active.update(&*self.db).await.map_err(Error::from)
    }

    async fn delete_before(&self, cutoff: &DateTime<Utc>) -> Result<u64> {
        let result = RawMessage::delete_many()
            .filter(raw_message::Column::ProcessedAt.is_not_null())
            .filter(raw_message::Column::ProcessedAt.lt(*cutoff))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected)
    }

    async fn delete_by_id(&self, id: Uuid) -> Result<bool> {
        let result = RawMessage::delete_by_id(id).exec(&*self.db).await?;
        Ok(result.rows_affected > 0)
    }

    async fn get_all(&self, params: &RawMessageQueryParams) -> Result<Vec<raw_message::Model>> {
        let query = RawMessage::find();
        let mut query = Self::apply_filters(query, params);

        // Apply sorting
        let sort_order = match params.get_sort_order() {
            SortOrder::Asc => Order::Asc,
            SortOrder::Desc => Order::Desc,
        };

        query = match params.get_sort_field() {
            RawMessageSortField::Timestamp => {
                query.order_by(raw_message::Column::Timestamp, sort_order)
            }
            RawMessageSortField::ProcessedAt => {
                query.order_by(raw_message::Column::ProcessedAt, sort_order)
            }
            RawMessageSortField::CreatedAt => {
                query.order_by(raw_message::Column::CreatedAt, sort_order)
            }
        };

        // Apply pagination
        query = query
            .limit(params.get_limit() as u64)
            .offset(params.get_offset() as u64);

        query.all(&*self.db).await.map_err(Error::from)
    }

    async fn count_all(&self, params: &RawMessageQueryParams) -> Result<i64> {
        let query = RawMessage::find();
        let query = Self::apply_filters(query, params);

        let count = query.count(&*self.db).await.map_err(Error::from)?;
        Ok(count as i64)
    }
}
