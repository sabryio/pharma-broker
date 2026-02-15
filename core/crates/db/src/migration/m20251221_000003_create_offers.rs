//! Migration: Create offers table

use sea_orm_migration::prelude::*;

use super::m20251221_000001_create_groups::Groups;
use super::m20251221_000002_create_raw_messages::RawMessages;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(Offers::Form).string_len(50))
                    .col(ColumnDef::new(Offers::Concentration).string_len(50))
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
                    // Medication curation support
                    .col(ColumnDef::new(Offers::MasterMedicationId).uuid())
                    .col(
                        ColumnDef::new(Offers::MedicationCurated)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Many-to-many matching support
                    .col(
                        ColumnDef::new(Offers::ConfirmedMatchCount)
                            .integer()
                            .not_null()
                            .default(0),
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offers_master_medication")
                            .from(Offers::Table, Offers::MasterMedicationId)
                            .to(Alias::new("medication_master"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::SetNull),
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

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_master_medication_id")
                    .table(Offers::Table)
                    .col(Offers::MasterMedicationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_offers_medication_curated")
                    .table(Offers::Table)
                    .col(Offers::MedicationCurated)
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
    Form,
    Concentration,
    Status,
    UrgencyLevel,
    ExpiryInfo,
    AiConfidence,
    // Medication curation support
    MasterMedicationId,
    MedicationCurated,
    // Many-to-many matching support
    ConfirmedMatchCount,
    // ContentEmbedding is added via raw SQL for vector type support
    CreatedAt,
    UpdatedAt,
}
