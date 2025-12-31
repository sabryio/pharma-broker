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
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RawMessages::ExternalId).string_len(50))
                    .col(ColumnDef::new(RawMessages::ParticipantId).uuid().not_null())
                    .col(ColumnDef::new(RawMessages::GroupId).uuid().not_null())
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_raw_messages_participant_id")
                            .from(RawMessages::Table, RawMessages::ParticipantId)
                            .to(Alias::new("participants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_raw_messages_group_id")
                            .from(RawMessages::Table, RawMessages::GroupId)
                            .to(Alias::new("groups"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_raw_messages_group_id")
                    .table(RawMessages::Table)
                    .col(RawMessages::GroupId)
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

        // Partial index for unprocessed messages
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_raw_messages_unprocessed ON raw_messages (timestamp) WHERE processed_at IS NULL",
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
    ParticipantId,
    GroupId,
    Content,
    Timestamp,
    ProcessedAt,
    Error,
    ReplyToId,
    ReplyToContent,
    ReplyToSender,
    CreatedAt,
}
