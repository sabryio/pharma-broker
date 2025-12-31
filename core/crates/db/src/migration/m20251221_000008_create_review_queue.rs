//! Migration: Create review_queue table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReviewQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReviewQueue::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(ReviewQueue::RawMessageId).uuid().not_null())
                    .col(
                        ColumnDef::new(ReviewQueue::AiResult)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReviewQueue::Confidence).double().not_null())
                    .col(
                        ColumnDef::new(ReviewQueue::Reason)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReviewQueue::Status)
                            .string_len(50)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(ReviewQueue::ReviewedBy).string_len(255))
                    .col(ColumnDef::new(ReviewQueue::ReviewNotes).text())
                    .col(
                        ColumnDef::new(ReviewQueue::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(ReviewQueue::ReviewedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_review_queue_status")
                    .table(ReviewQueue::Table)
                    .col(ReviewQueue::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_review_queue_created_at")
                    .table(ReviewQueue::Table)
                    .col(ReviewQueue::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_review_queue_raw_message_id")
                    .table(ReviewQueue::Table)
                    .col(ReviewQueue::RawMessageId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_review_queue_confidence")
                    .table(ReviewQueue::Table)
                    .col(ReviewQueue::Confidence)
                    .to_owned(),
            )
            .await?;

        // Partial index for pending items
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_review_queue_pending_created ON review_queue(status, created_at) WHERE status = 'pending'",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReviewQueue::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ReviewQueue {
    Table,
    Id,
    RawMessageId,
    AiResult,
    Confidence,
    Reason,
    Status,
    ReviewedBy,
    ReviewNotes,
    CreatedAt,
    ReviewedAt,
}
