use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RetryQueueItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RetryQueueItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()"),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::RawMessageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::Status)
                            .string_len(20)
                            .not_null()
                            .default("PENDING"),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::FailureReason)
                            .string_len(30)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::OriginalError)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RetryQueueItems::LastError).text())
                    .col(
                        ColumnDef::new(RetryQueueItems::NextAttemptAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RetryQueueItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(RetryQueueItems::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_retry_queue_raw_message")
                            .from(RetryQueueItems::Table, RetryQueueItems::RawMessageId)
                            .to(RawMessages::Table, RawMessages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for efficient querying
        manager
            .create_index(
                Index::create()
                    .name("idx_retry_queue_status")
                    .table(RetryQueueItems::Table)
                    .col(RetryQueueItems::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_retry_queue_next_attempt")
                    .table(RetryQueueItems::Table)
                    .col(RetryQueueItems::NextAttemptAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_retry_queue_raw_message")
                    .table(RetryQueueItems::Table)
                    .col(RetryQueueItems::RawMessageId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_retry_queue_failure_reason")
                    .table(RetryQueueItems::Table)
                    .col(RetryQueueItems::FailureReason)
                    .to_owned(),
            )
            .await?;

        // Composite index for efficient queue processing
        manager
            .create_index(
                Index::create()
                    .name("idx_retry_queue_processing")
                    .table(RetryQueueItems::Table)
                    .col(RetryQueueItems::Status)
                    .col(RetryQueueItems::NextAttemptAt)
                    .col(RetryQueueItems::Priority)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RetryQueueItems::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RetryQueueItems {
    Table,
    Id,
    RawMessageId,
    Status,
    Priority,
    Attempts,
    MaxAttempts,
    FailureReason,
    OriginalError,
    LastError,
    NextAttemptAt,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum RawMessages {
    Table,
    Id,
}
