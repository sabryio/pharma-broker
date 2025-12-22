//! Migration: Create raw_messages table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RawMessages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RawMessages::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RawMessages::ExternalId).string_len(50))
                    .col(
                        ColumnDef::new(RawMessages::GroupJid)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(RawMessages::GroupName).string_len(100))
                    .col(
                        ColumnDef::new(RawMessages::SenderJid)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(RawMessages::SenderPhone).string_len(20))
                    .col(ColumnDef::new(RawMessages::SenderName).string_len(100))
                    .col(ColumnDef::new(RawMessages::Content).text().not_null())
                    .col(
                        ColumnDef::new(RawMessages::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RawMessages::ProcessedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(RawMessages::Error).text())
                    .col(ColumnDef::new(RawMessages::ReplyToId).string_len(50))
                    .col(ColumnDef::new(RawMessages::ReplyToContent).text())
                    .col(ColumnDef::new(RawMessages::ReplyToSender).string_len(50))
                    .col(
                        ColumnDef::new(RawMessages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_raw_messages_group_jid")
                    .table(RawMessages::Table)
                    .col(RawMessages::GroupJid)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_raw_messages_processed_at")
                    .table(RawMessages::Table)
                    .col(RawMessages::ProcessedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_raw_messages_timestamp")
                    .table(RawMessages::Table)
                    .col(RawMessages::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RawMessages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum RawMessages {
    Table,
    Id,
    ExternalId,
    GroupJid,
    GroupName,
    SenderJid,
    SenderPhone,
    SenderName,
    Content,
    Timestamp,
    ProcessedAt,
    Error,
    ReplyToId,
    ReplyToContent,
    ReplyToSender,
    CreatedAt,
}
