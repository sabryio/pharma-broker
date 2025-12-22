//! Migration: Create match_queue_items table

use sea_orm_migration::prelude::*;

use super::m20251221_000004_create_requests::Requests;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MatchQueueItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MatchQueueItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::RequestId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::Status)
                            .string_len(20)
                            .not_null()
                            .default("PENDING"),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(MatchQueueItems::LastError).text())
                    .col(
                        ColumnDef::new(MatchQueueItems::NextAttemptAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MatchQueueItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_match_queue_request")
                            .from(MatchQueueItems::Table, MatchQueueItems::RequestId)
                            .to(Requests::Table, Requests::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for efficient queue processing
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_queue_status_priority ON match_queue_items(status, priority DESC, next_attempt_at ASC) WHERE status = 'PENDING'",
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_match_queue_request_id")
                    .table(MatchQueueItems::Table)
                    .col(MatchQueueItems::RequestId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MatchQueueItems::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MatchQueueItems {
    Table,
    Id,
    RequestId,
    Status,
    Priority,
    Attempts,
    LastError,
    NextAttemptAt,
    CreatedAt,
    UpdatedAt,
}
