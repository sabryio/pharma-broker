//! Group repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{prelude::Expr, *};
use uuid::Uuid;

use crate::entity::group::{self, Entity as Group};
use crate::traits::GroupRepository;
use crate::{Error, Result};

/// SeaORM-based group repository
pub struct SeaOrmGroupRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmGroupRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GroupRepository for SeaOrmGroupRepo {
    async fn get_all(&self) -> Result<Vec<group::Model>> {
        Group::find()
            .order_by_asc(group::Column::Name)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<group::Model>> {
        Group::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_jid(&self, jid: &str) -> Result<Option<group::Model>> {
        Group::find()
            .filter(group::Column::Jid.eq(jid))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn is_monitored(&self, jid: &str) -> Result<bool> {
        let group = self.get_by_jid(jid).await?;
        Ok(group.map(|g| g.monitored).unwrap_or(false))
    }

    async fn get_monitored(&self) -> Result<Vec<group::Model>> {
        Group::find()
            .filter(group::Column::Monitored.eq(true))
            .order_by_asc(group::Column::Name)
            .all(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &group::Model) -> Result<group::Model> {
        let existing = self.get_by_id(model.id).await?;
        let active: group::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.update(&*self.db).await.map_err(Error::from)
        } else {
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    async fn update_monitored(&self, jid: &str, monitored: bool) -> Result<()> {
        let group = self
            .get_by_jid(jid)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Group not found: {}", jid)))?;

        let mut active: group::ActiveModel = group.into();
        active.monitored = Set(monitored);
        active.update(&*self.db).await?;
        Ok(())
    }

    async fn delete(&self, jid: &str) -> Result<bool> {
        let result = Group::delete_many()
            .filter(group::Column::Jid.eq(jid))
            .exec(&*self.db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    async fn update_last_message(&self, jid: &str) -> Result<()> {
        Group::update_many()
            .col_expr(group::Column::LastMessage, Expr::current_timestamp().into())
            .filter(group::Column::Jid.eq(jid))
            .exec(&*self.db)
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
            .exec(&*self.db)
            .await?;
        Ok(())
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_group};
    use sea_orm::EntityTrait;

    async fn create_group(db: &TestDb, jid: &str, name: &str, monitored: bool) -> group::Model {
        let group_am = new_test_group(jid, name, monitored);
        group::Entity::insert(group_am)
            .exec(&*db.db)
            .await
            .expect("Insert group");

        group::Entity::find_by_id(jid)
            .one(&*db.db)
            .await
            .expect("Find group")
            .expect("Group should exist")
    }

    #[tokio::test]
    async fn test_get_by_jid_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        let group = create_group(&db, "group1@g.us", "Group 1", true).await;

        let found = repo
            .get_by_jid(&group.jid)
            .await
            .expect("GetByJid should succeed");
        assert!(found.is_some(), "Should find the group");
        assert_eq!(found.unwrap().jid, group.jid, "JID should match");
    }

    #[tokio::test]
    async fn test_get_by_jid_not_found() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        let found = repo
            .get_by_jid("non-existent@g.us")
            .await
            .expect("GetByJid should not error");
        assert!(found.is_none(), "Should return None for non-existent JID");
    }

    #[tokio::test]
    async fn test_get_all() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "a-group@g.us", "Alpha", true).await;
        create_group(&db, "b-group@g.us", "Beta", false).await;
        create_group(&db, "c-group@g.us", "Charlie", true).await;

        let all = repo.get_all().await.expect("GetAll");
        assert_eq!(all.len(), 3, "Should have 3 groups");
        // Ordered by name
        assert_eq!(all[0].name, "Alpha");
        assert_eq!(all[1].name, "Beta");
        assert_eq!(all[2].name, "Charlie");
    }

    #[tokio::test]
    async fn test_is_monitored() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "monitored@g.us", "Monitored", true).await;
        create_group(&db, "not-monitored@g.us", "Not Monitored", false).await;

        assert!(
            repo.is_monitored("monitored@g.us")
                .await
                .expect("is_monitored"),
            "Should be monitored"
        );
        assert!(
            !repo
                .is_monitored("not-monitored@g.us")
                .await
                .expect("is_monitored"),
            "Should not be monitored"
        );
        assert!(
            !repo
                .is_monitored("non-existent@g.us")
                .await
                .expect("is_monitored"),
            "Non-existent should be false"
        );
    }

    #[tokio::test]
    async fn test_get_monitored() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "mon1@g.us", "Mon1", true).await;
        create_group(&db, "mon2@g.us", "Mon2", true).await;
        create_group(&db, "unmon@g.us", "Unmon", false).await;

        let monitored = repo.get_monitored().await.expect("GetMonitored");
        assert_eq!(monitored.len(), 2, "Should have 2 monitored groups");
        assert!(
            monitored.iter().all(|g| g.monitored),
            "All should be monitored"
        );
    }

    #[tokio::test]
    async fn test_update_monitored() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "toggle@g.us", "Toggle", false).await;
        assert!(
            !repo.is_monitored("toggle@g.us").await.expect("check"),
            "Initially not monitored"
        );

        repo.update_monitored("toggle@g.us", true)
            .await
            .expect("UpdateMonitored");
        assert!(
            repo.is_monitored("toggle@g.us").await.expect("check"),
            "Should now be monitored"
        );
    }

    #[tokio::test]
    async fn test_delete() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "to-delete@g.us", "Delete Me", false).await;
        assert!(
            repo.get_by_jid("to-delete@g.us")
                .await
                .expect("get")
                .is_some(),
            "Should exist before delete"
        );

        let deleted = repo.delete("to-delete@g.us").await.expect("Delete");
        assert!(deleted, "Delete should return true");

        assert!(
            repo.get_by_jid("to-delete@g.us")
                .await
                .expect("get")
                .is_none(),
            "Should not exist after delete"
        );
    }

    #[tokio::test]
    async fn test_increment_message_count() {
        let db = TestDb::new().await;
        let repo = SeaOrmGroupRepo::new(db.db.clone());

        create_group(&db, "counter@g.us", "Counter", true).await;

        // Increment 3 times
        repo.increment_message_count("counter@g.us")
            .await
            .expect("inc1");
        repo.increment_message_count("counter@g.us")
            .await
            .expect("inc2");
        repo.increment_message_count("counter@g.us")
            .await
            .expect("inc3");

        let group = repo.get_by_jid("counter@g.us").await.expect("get").unwrap();
        assert_eq!(group.message_count, 3, "Message count should be 3");
    }
}
