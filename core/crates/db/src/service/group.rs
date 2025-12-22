//! Group Service - WhatsApp group management

use sea_orm::{prelude::Expr, *};

use crate::entity::group::{self, Entity as Group};
use crate::{Error, Result};

/// Service for group operations
pub struct GroupService;

impl GroupService {
    /// Get all groups
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<group::Model>> {
        Group::find()
            .order_by_asc(group::Column::Name)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Get group by JID
    pub async fn get_by_jid(db: &DatabaseConnection, jid: &str) -> Result<Option<group::Model>> {
        Group::find_by_id(jid).one(db).await.map_err(Error::from)
    }

    /// Get all monitored groups
    pub async fn get_monitored(db: &DatabaseConnection) -> Result<Vec<group::Model>> {
        Group::find()
            .filter(group::Column::Monitored.eq(true))
            .order_by_asc(group::Column::Name)
            .all(db)
            .await
            .map_err(Error::from)
    }

    /// Check if a group is monitored
    pub async fn is_monitored(db: &DatabaseConnection, jid: &str) -> Result<bool> {
        let group = Group::find_by_id(jid).one(db).await?;
        Ok(group.map(|g| g.monitored).unwrap_or(false))
    }

    /// Save or update a group (upsert)
    pub async fn save(db: &DatabaseConnection, model: group::ActiveModel) -> Result<group::Model> {
        let jid = model.jid.clone().unwrap();

        // Check if exists
        let existing = Group::find_by_id(&jid).one(db).await?;

        if existing.is_some() {
            model.update(db).await.map_err(Error::from)
        } else {
            model.insert(db).await.map_err(Error::from)
        }
    }

    /// Update monitoring status
    pub async fn update_monitored(
        db: &DatabaseConnection,
        jid: &str,
        monitored: bool,
    ) -> Result<()> {
        let group = Group::find_by_id(jid)
            .one(db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Group not found: {}", jid)))?;

        let mut active: group::ActiveModel = group.into();
        active.monitored = Set(monitored);
        active.update(db).await?;

        Ok(())
    }

    /// Update last message timestamp
    pub async fn update_last_message(db: &DatabaseConnection, jid: &str) -> Result<()> {
        Group::update_many()
            .col_expr(group::Column::LastMessage, Expr::current_timestamp().into())
            .filter(group::Column::Jid.eq(jid))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Increment message count
    pub async fn increment_message_count(db: &DatabaseConnection, jid: &str) -> Result<()> {
        Group::update_many()
            .col_expr(
                group::Column::MessageCount,
                Expr::col(group::Column::MessageCount).add(1),
            )
            .filter(group::Column::Jid.eq(jid))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Delete a group
    pub async fn delete(db: &DatabaseConnection, jid: &str) -> Result<bool> {
        let result = Group::delete_by_id(jid).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ActiveValue::Set;

    // Helper to create test group
    fn new_test_group(jid: &str, name: &str, monitored: bool) -> group::ActiveModel {
        group::ActiveModel {
            jid: Set(jid.to_string()),
            name: Set(name.to_string()),
            description: Set(None),
            monitored: Set(monitored),
            added_at: Set(chrono::Utc::now()),
            last_message: Set(None),
            message_count: Set(0),
        }
    }

    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_save_and_get() {
        // This test requires a running database
        // Run with: cargo test --features test-db -- --ignored
    }

    #[test]
    fn test_new_group_defaults() {
        let group = new_test_group("test@g.us", "Test Group", false);
        assert_eq!(group.jid.unwrap(), "test@g.us");
        assert_eq!(group.name.unwrap(), "Test Group");
        assert!(!group.monitored.unwrap());
    }
}
