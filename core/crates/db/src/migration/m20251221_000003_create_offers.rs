//! Migration: Create offers table

use sea_orm_migration::prelude::*;

use super::m20251221_000001_create_groups::Groups;
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

        // Enable pg_trgm extension for trigram search
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Offers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Offers::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Offers::RawMessageId).uuid())
                    .col(ColumnDef::new(Offers::ParticipantId).uuid().not_null())
                    .col(ColumnDef::new(Offers::GroupId).uuid().not_null())
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
                    .col(
                        ColumnDef::new(Offers::Status)
                            .string_len(20)
                            .not_null()
                            .default("ACTIVE"),
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offers_participant_id")
                            .from(Offers::Table, Offers::ParticipantId)
                            .to(Alias::new("participants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offers_group_id")
                            .from(Offers::Table, Offers::GroupId)
                            .to(Groups::Table, Groups::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Add vector column for embeddings
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE offers ADD COLUMN IF NOT EXISTS content_embedding vector(768)",
            )
            .await?;

        // B-tree Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_medication")
                    .table(Offers::Table)
                    .col(Offers::Medication)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_status")
                    .table(Offers::Table)
                    .col(Offers::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_participant_id")
                    .table(Offers::Table)
                    .col(Offers::ParticipantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_group_id")
                    .table(Offers::Table)
                    .col(Offers::GroupId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_created_at")
                    .table(Offers::Table)
                    .col(Offers::CreatedAt)
                    .to_owned(),
            )
            .await?;

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

        // GIN index for trigram similarity search on medication names
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_offers_medication_trgm ON offers USING gin (medication gin_trgm_ops)",
            )
            .await?;

        // GIN index for trigram search on medication_raw
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_offers_medication_raw_trgm ON offers USING gin (medication_raw gin_trgm_ops)",
            )
            .await?;

        // HNSW index for vector similarity search (cosine distance)
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
    ParticipantId,
    GroupId,
    Medication,
    MedicationRaw,
    Quantity,
    Unit,
    Price,
    Currency,
    ExpiryDate,
    BatchNumber,
    Notes,
    Status,
    UrgencyLevel,
    ExpiryInfo,
    AiConfidence,
    // ContentEmbedding is added via raw SQL for vector type support
    CreatedAt,
    UpdatedAt,
}
