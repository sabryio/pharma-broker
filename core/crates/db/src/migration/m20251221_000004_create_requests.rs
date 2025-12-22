//! Migration: Create requests table

use sea_orm_migration::prelude::*;

use super::m20251221_000002_create_raw_messages::RawMessages;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Requests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Requests::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Requests::RawMessageId).string_len(36))
                    .col(
                        ColumnDef::new(Requests::SourcePhone)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Requests::SourceName).string_len(100))
                    .col(
                        ColumnDef::new(Requests::SourceGroup)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Requests::GroupName).string_len(100))
                    .col(
                        ColumnDef::new(Requests::Medication)
                            .string_len(200)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Requests::MedicationRaw).string_len(500))
                    .col(ColumnDef::new(Requests::Quantity).decimal_len(10, 2))
                    .col(ColumnDef::new(Requests::Unit).string_len(20))
                    .col(ColumnDef::new(Requests::MaxPrice).decimal_len(10, 2))
                    .col(
                        ColumnDef::new(Requests::Currency)
                            .string_len(10)
                            .default("EGP"),
                    )
                    .col(
                        ColumnDef::new(Requests::Urgent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Requests::UrgencyLevel)
                            .string_len(20)
                            .not_null()
                            .default("NORMAL"),
                    )
                    .col(ColumnDef::new(Requests::ExpiryRequirement).string_len(50))
                    .col(
                        ColumnDef::new(Requests::AiConfidence)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(ColumnDef::new(Requests::Notes).text())
                    .col(ColumnDef::new(Requests::RawMessage).text())
                    .col(
                        ColumnDef::new(Requests::Status)
                            .string_len(20)
                            .not_null()
                            .default("ACTIVE"),
                    )
                    .col(
                        ColumnDef::new(Requests::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Requests::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_requests_raw_message")
                            .from(Requests::Table, Requests::RawMessageId)
                            .to(RawMessages::Table, RawMessages::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Add vector column for embeddings
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE requests ADD COLUMN IF NOT EXISTS content_embedding vector(384)",
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_requests_medication")
                    .table(Requests::Table)
                    .col(Requests::Medication)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_requests_status")
                    .table(Requests::Table)
                    .col(Requests::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_requests_source_phone")
                    .table(Requests::Table)
                    .col(Requests::SourcePhone)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_requests_created_at")
                    .table(Requests::Table)
                    .col(Requests::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_requests_urgent")
                    .table(Requests::Table)
                    .col(Requests::Urgent)
                    .to_owned(),
            )
            .await?;

        // Vector similarity index
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_requests_embedding ON requests USING hnsw (content_embedding vector_cosine_ops)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Requests::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Requests {
    Table,
    Id,
    RawMessageId,
    SourcePhone,
    SourceName,
    SourceGroup,
    GroupName,
    Medication,
    MedicationRaw,
    Quantity,
    Unit,
    MaxPrice,
    Currency,
    Urgent,
    UrgencyLevel,
    ExpiryRequirement,
    AiConfidence,
    Notes,
    RawMessage,
    Status,
    #[allow(dead_code)]
    ContentEmbedding,
    CreatedAt,
    UpdatedAt,
}
