//! Migration: Create groups table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Groups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Groups::Jid)
                            .string_len(50)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Groups::Name).string_len(100).not_null())
                    .col(ColumnDef::new(Groups::Description).text())
                    .col(
                        ColumnDef::new(Groups::Monitored)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Groups::AddedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Groups::LastMessage).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Groups::MessageCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for monitored groups
        manager
            .create_index(
                Index::create()
                    .name("idx_groups_monitored")
                    .table(Groups::Table)
                    .col(Groups::Monitored)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Groups::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Groups {
    Table,
    Jid,
    Name,
    Description,
    Monitored,
    AddedAt,
    LastMessage,
    MessageCount,
}
