//! Migration: Create requests table

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
                    .table(Requests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Requests::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Requests::RawMessageId).uuid())
                    .col(ColumnDef::new(Requests::ParticipantId).uuid().not_null())
                    .col(ColumnDef::new(Requests::GroupId).uuid().not_null())
                    .col(
                        ColumnDef::new(Requests::Medication)
                            .string_len(200)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Requests::Form).string_len(50))
                    .col(ColumnDef::new(Requests::Concentration).string_len(50))
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
                    // Medication curation support
                    .col(ColumnDef::new(Requests::MasterMedicationId).uuid())
                    .col(
                        ColumnDef::new(Requests::MedicationCurated)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // Many-to-many matching support
                    .col(
                        ColumnDef::new(Requests::ConfirmedMatchCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_requests_participant_id")
                            .from(Requests::Table, Requests::ParticipantId)
                            .to(Alias::new("participants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_requests_group_id")
                            .from(Requests::Table, Requests::GroupId)
                            .to(Groups::Table, Groups::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_requests_master_medication")
                            .from(Requests::Table, Requests::MasterMedicationId)
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
                "ALTER TABLE requests ADD COLUMN IF NOT EXISTS content_embedding vector(768)",
            )
            .await?;

        // B-tree Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_medication")
                    .table(Requests::Table)
                    .col(Requests::Medication)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_status")
                    .table(Requests::Table)
                    .col(Requests::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_participant_id")
                    .table(Requests::Table)
                    .col(Requests::ParticipantId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_group_id")
                    .table(Requests::Table)
                    .col(Requests::GroupId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_created_at")
                    .table(Requests::Table)
                    .col(Requests::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_urgency_level")
                    .table(Requests::Table)
                    .col(Requests::UrgencyLevel)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_master_medication_id")
                    .table(Requests::Table)
                    .col(Requests::MasterMedicationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_requests_medication_curated")
                    .table(Requests::Table)
                    .col(Requests::MedicationCurated)
                    .to_owned(),
            )
            .await?;

        // GIN index for trigram similarity search on medication names
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_requests_medication_trgm ON requests USING gin (medication gin_trgm_ops)",
            )
            .await?;

        // HNSW index for vector similarity search (cosine distance)
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
    ParticipantId,
    GroupId,
    Medication,
    Form,
    Concentration,
    UrgencyLevel,
    ExpiryRequirement,
    AiConfidence,
    // Medication curation support
    MasterMedicationId,
    MedicationCurated,
    // Many-to-many matching support
    ConfirmedMatchCount,
    Status,
    // ContentEmbedding is added via raw SQL for vector type support
    CreatedAt,
    UpdatedAt,
}
