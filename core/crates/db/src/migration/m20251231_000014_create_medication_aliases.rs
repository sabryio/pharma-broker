//! Migration: Create medication_aliases table
//!
//! Maps parsed medication variations to master records.
//! This is Phase 1 of the Medication Curation System.

use sea_orm_migration::prelude::*;

use super::m20251231_000013_create_medication_master::MedicationMaster;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create medication_aliases table
        manager
            .create_table(
                Table::create()
                    .table(MedicationAliases::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MedicationAliases::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    // The raw/parsed medication name
                    .col(
                        ColumnDef::new(MedicationAliases::AliasName)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationAliases::AliasNameNormalized)
                            .string_len(500)
                            .not_null(),
                    )
                    // Link to master record (nullable until curated)
                    .col(ColumnDef::new(MedicationAliases::MasterMedicationId).uuid())
                    // Curation metadata
                    .col(ColumnDef::new(MedicationAliases::AiSuggestionConfidence).double())
                    .col(
                        ColumnDef::new(MedicationAliases::CurationStatus)
                            .string_len(20)
                            .not_null()
                            .default("PENDING"),
                    )
                    .col(ColumnDef::new(MedicationAliases::CuratedBy).string_len(255))
                    .col(ColumnDef::new(MedicationAliases::CuratedAt).timestamp_with_time_zone())
                    // Statistics
                    .col(
                        ColumnDef::new(MedicationAliases::OccurrenceCount)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(MedicationAliases::FirstSeenAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MedicationAliases::LastSeenAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Foreign key to master medication
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_medication_aliases_master")
                            .from(
                                MedicationAliases::Table,
                                MedicationAliases::MasterMedicationId,
                            )
                            .to(MedicationMaster::Table, MedicationMaster::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on normalized alias name
        manager
            .create_index(
                Index::create()
                    .name("idx_medication_aliases_unique_normalized")
                    .table(MedicationAliases::Table)
                    .col(MedicationAliases::AliasNameNormalized)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for fast alias lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_medication_aliases_lookup")
                    .table(MedicationAliases::Table)
                    .col(MedicationAliases::AliasNameNormalized)
                    .to_owned(),
            )
            .await?;

        // Index for finding pending curation items
        manager
            .create_index(
                Index::create()
                    .name("idx_medication_aliases_pending")
                    .table(MedicationAliases::Table)
                    .col(MedicationAliases::CurationStatus)
                    .to_owned(),
            )
            .await?;

        // Trigram index for fuzzy alias search
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_aliases_trgm 
                 ON medication_aliases USING gin(alias_name gin_trgm_ops)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MedicationAliases::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MedicationAliases {
    Table,
    Id,
    AliasName,
    AliasNameNormalized,
    MasterMedicationId,
    AiSuggestionConfidence,
    CurationStatus,
    CuratedBy,
    CuratedAt,
    OccurrenceCount,
    FirstSeenAt,
    LastSeenAt,
}
