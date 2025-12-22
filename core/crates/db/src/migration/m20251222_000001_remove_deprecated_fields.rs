//! Migration: Remove deprecated fields from offers and requests tables
//!
//! This migration removes:
//! - `urgent` boolean field (replaced by `urgency_level` enum)
//! - `group_name` field (denormalized, can be joined from groups table)
//! - `raw_message` field (denormalized, already stored in raw_messages table)
//!
//! These fields were kept for backward compatibility during the transition period.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop index on urgent column first (offers)
        manager
            .drop_index(
                Index::drop()
                    .name("idx_offers_urgent")
                    .table(Offers::Table)
                    .to_owned(),
            )
            .await
            .ok(); // Ignore if doesn't exist

        // Drop index on urgent column (requests) if exists
        manager
            .drop_index(
                Index::drop()
                    .name("idx_requests_urgent")
                    .table(Requests::Table)
                    .to_owned(),
            )
            .await
            .ok(); // Ignore if doesn't exist

        // Remove deprecated columns from offers table
        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .drop_column(Offers::Urgent)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .drop_column(Offers::GroupName)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .drop_column(Offers::RawMessage)
                    .to_owned(),
            )
            .await?;

        // Remove deprecated columns from requests table
        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .drop_column(Requests::Urgent)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .drop_column(Requests::GroupName)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .drop_column(Requests::RawMessage)
                    .to_owned(),
            )
            .await?;

        // Add index on urgency_level for both tables (if not exists)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_urgency_level")
                    .table(Offers::Table)
                    .col(Offers::UrgencyLevel)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_urgency_level")
                    .table(Requests::Table)
                    .col(Requests::UrgencyLevel)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add columns to offers table
        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .add_column(
                        ColumnDef::new(Offers::Urgent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .add_column(ColumnDef::new(Offers::GroupName).string_len(100))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .add_column(ColumnDef::new(Offers::RawMessage).text())
                    .to_owned(),
            )
            .await?;

        // Re-add columns to requests table
        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .add_column(
                        ColumnDef::new(Requests::Urgent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .add_column(ColumnDef::new(Requests::GroupName).string_len(100))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .add_column(ColumnDef::new(Requests::RawMessage).text())
                    .to_owned(),
            )
            .await?;

        // Re-create index on urgent
        manager
            .create_index(
                Index::create()
                    .name("idx_offers_urgent")
                    .table(Offers::Table)
                    .col(Offers::Urgent)
                    .to_owned(),
            )
            .await?;

        // Populate urgent from urgency_level
        manager
            .get_connection()
            .execute_unprepared("UPDATE offers SET urgent = (urgency_level != 'NORMAL')")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("UPDATE requests SET urgent = (urgency_level != 'NORMAL')")
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Offers {
    Table,
    Urgent,
    GroupName,
    RawMessage,
    UrgencyLevel,
}

#[derive(DeriveIden)]
enum Requests {
    Table,
    Urgent,
    GroupName,
    RawMessage,
    UrgencyLevel,
}
