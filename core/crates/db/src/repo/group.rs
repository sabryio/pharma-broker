//! Group repository implementation

use async_trait::async_trait;
use sea_orm::{prelude::Expr, *};

use crate::entity::group::{self, Entity as Group};
use crate::traits::GroupRepository;
use crate::{Error, Result};

/// SeaORM-based group repository
pub struct SeaOrmGroupRepo {
    db: DatabaseConnection,
}

impl SeaOrmGroupRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GroupRepository for SeaOrmGroupRepo {
    async fn get_all(&self) -> Result<Vec<group::Model>> {
        Group::find()
            .order_by_asc(group::Column::Name)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_jid(&self, jid: &str) -> Result<Option<group::Model>> {
        Group::find_by_id(jid)
            .one(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn is_monitored(&self, jid: &str) -> Result<bool> {
        let group = Group::find_by_id(jid).one(&self.db).await?;
        Ok(group.map(|g| g.monitored).unwrap_or(false))
    }

    async fn get_monitored(&self) -> Result<Vec<group::Model>> {
        Group::find()
            .filter(group::Column::Monitored.eq(true))
            .order_by_asc(group::Column::Name)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &group::Model) -> Result<group::Model> {
        let existing = Group::find_by_id(&model.jid).one(&self.db).await?;
        let active: group::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.update(&self.db).await.map_err(Error::from)
        } else {
            active.insert(&self.db).await.map_err(Error::from)
        }
    }

    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()> {
        let group = Group::find_by_id(jid)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Group not found: {}", jid)))?;

        let mut active: group::ActiveModel = group.into();
        active.monitored = Set(monitored);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn delete(&self, jid: &str) -> Result<bool> {
        let result = Group::delete_by_id(jid).exec(&self.db).await?;
        Ok(result.rows_affected > 0)
    }

    async fn update_last_message(&self, jid: &str) -> Result<()> {
        Group::update_many()
            .col_expr(group::Column::LastMessage, Expr::current_timestamp().into())
            .filter(group::Column::Jid.eq(jid))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn increment_message_count(&self, jid: &str) -> Result<()> {
        Group::update_many()
            .col_expr(
                group::Column::MessageCount,
                Expr::col(group::Column::MessageCount).add(1),
            )
            .filter(group::Column::Jid.eq(jid))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}
