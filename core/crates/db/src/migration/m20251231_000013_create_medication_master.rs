//! Migration: Create medication_master table
//!
//! Authoritative master table for all curated medications.
//! This is Phase 1 of the Medication Curation System.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Enable pgvector extension (must be done before creating vector columns)
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS vector")
            .await?;

        // Enable pg_trgm extension for trigram search
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await?;

        // Create medication_master table
        manager
            .create_table(
                Table::create()
                    .table(MedicationMaster::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MedicationMaster::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    // Core identification
                    .col(
                        ColumnDef::new(MedicationMaster::CanonicalName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MedicationMaster::CanonicalNameAr).string_len(255))
                    // Pharmaceutical details
                    .col(ColumnDef::new(MedicationMaster::ActiveIngredient).string_len(255))
                    .col(ColumnDef::new(MedicationMaster::Strength).string_len(100))
                    .col(ColumnDef::new(MedicationMaster::DosageForm).string_len(100))
                    .col(ColumnDef::new(MedicationMaster::Manufacturer).string_len(255))
                    // Regulatory
                    .col(ColumnDef::new(MedicationMaster::EdaRegistration).string_len(100))
                    // Classification
                    .col(ColumnDef::new(MedicationMaster::TherapeuticClass).string_len(255))
                    .col(ColumnDef::new(MedicationMaster::AtcCode).string_len(20))
                    // Status
                    .col(
                        ColumnDef::new(MedicationMaster::Status)
                            .string_len(20)
                            .not_null()
                            .default("ACTIVE"),
                    )
                    // Metadata
                    .col(
                        ColumnDef::new(MedicationMaster::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MedicationMaster::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(MedicationMaster::CreatedBy).string_len(255))
                    .to_owned(),
            )
            .await?;

        // Add embedding column (Vector(768)) separately since sea-orm-migration doesn't natively support vector type yet
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_master ADD COLUMN IF NOT EXISTS embedding vector(768)",
            )
            .await?;

        // Add unique constraint on canonical_name + strength
        manager
            .create_index(
                Index::create()
                    .name("idx_medication_master_unique_name_strength")
                    .table(MedicationMaster::Table)
                    .col(MedicationMaster::CanonicalName)
                    .col(MedicationMaster::Strength)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Full-text search index for name lookups
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_master_search 
                 ON medication_master USING gin(
                     to_tsvector('simple', 
                         canonical_name || ' ' || 
                         COALESCE(canonical_name_ar, '') || ' ' || 
                         COALESCE(active_ingredient, '')
                     )
                 )",
            )
            .await?;

        // Trigram index for fuzzy search on canonical_name
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_master_name_trgm 
                 ON medication_master USING gin(canonical_name gin_trgm_ops)",
            )
            .await?;

        // Vector index for AI-driven suggestions (HNSW for performance)
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_master_embedding 
                 ON medication_master USING hnsw (embedding vector_cosine_ops)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MedicationMaster::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MedicationMaster {
    Table,
    Id,
    CanonicalName,
    CanonicalNameAr,
    ActiveIngredient,
    Strength,
    DosageForm,
    Manufacturer,
    EdaRegistration,
    TherapeuticClass,
    AtcCode,
    Status,
    CreatedAt,
    UpdatedAt,
    CreatedBy,
}
