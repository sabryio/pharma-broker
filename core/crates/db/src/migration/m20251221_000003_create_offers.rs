//! Migration: Create offers table

use sea_orm_migration::prelude::*;

use super::m20251221_000002_create_raw_messages::RawMessages;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Enable pgvector extension
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS vector")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Offers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Offers::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Offers::RawMessageId).string_len(36))
                    .col(
                        ColumnDef::new(Offers::SourcePhone)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Offers::SourceName).string_len(100))
                    .col(
                        ColumnDef::new(Offers::SourceGroup)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Offers::GroupName).string_len(100))
                    .col(
                        ColumnDef::new(Offers::Medication)
                            .string_len(200)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Offers::MedicationRaw).string_len(500))
                    .col(ColumnDef::new(Offers::Quantity).decimal_len(10, 2))
                    .col(ColumnDef::new(Offers::Unit).string_len(20))
                    .col(ColumnDef::new(Offers::Price).decimal_len(10, 2))
                    .col(
                        ColumnDef::new(Offers::Currency)
                            .string_len(10)
                            .default("EGP"),
                    )
                    .col(ColumnDef::new(Offers::ExpiryDate).date())
                    .col(ColumnDef::new(Offers::BatchNumber).string_len(50))
                    .col(ColumnDef::new(Offers::Notes).text())
                    .col(ColumnDef::new(Offers::RawMessage).text())
                    .col(
                        ColumnDef::new(Offers::Status)
                            .string_len(20)
                            .not_null()
                            .default("ACTIVE"),
                    )
                    .col(
                        ColumnDef::new(Offers::Urgent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Offers::UrgencyLevel)
                            .string_len(20)
                            .not_null()
                            .default("NORMAL"),
                    )
                    .col(ColumnDef::new(Offers::ExpiryInfo).string_len(50))
                    .col(
                        ColumnDef::new(Offers::AiConfidence)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(Offers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Offers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offers_raw_message")
                            .from(Offers::Table, Offers::RawMessageId)
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
                "ALTER TABLE offers ADD COLUMN IF NOT EXISTS content_embedding vector(384)",
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_offers_medication")
                    .table(Offers::Table)
                    .col(Offers::Medication)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_offers_status")
                    .table(Offers::Table)
                    .col(Offers::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_offers_source_phone")
                    .table(Offers::Table)
                    .col(Offers::SourcePhone)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_offers_created_at")
                    .table(Offers::Table)
                    .col(Offers::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_offers_urgent")
                    .table(Offers::Table)
                    .col(Offers::Urgent)
                    .to_owned(),
            )
            .await?;

        // Vector similarity index (HNSW for better performance)
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_offers_embedding ON offers USING hnsw (content_embedding vector_cosine_ops)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Offers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Offers {
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
    Price,
    Currency,
    ExpiryDate,
    BatchNumber,
    Notes,
    RawMessage,
    Status,
    Urgent,
    UrgencyLevel,
    ExpiryInfo,
    AiConfidence,
    ContentEmbedding,
    CreatedAt,
    UpdatedAt,
}
