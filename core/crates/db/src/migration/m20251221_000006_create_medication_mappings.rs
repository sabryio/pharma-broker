//! Migration: Create medication_mappings table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Enable pg_trgm extension for trigram search
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MedicationMappings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MedicationMappings::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(MedicationMappings::ArabicName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationMappings::EnglishName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationMappings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MedicationMappings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Add array column for synonyms
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_mappings ADD COLUMN IF NOT EXISTS synonyms TEXT[]",
            )
            .await?;

        // Add vector column for embeddings (768 dimensions for larger models)
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_mappings ADD COLUMN IF NOT EXISTS embedding vector(768)",
            )
            .await?;

        // Trigram indexes for fuzzy search
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_mappings_arabic_trgm ON medication_mappings USING gin (arabic_name gin_trgm_ops)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_mappings_english_trgm ON medication_mappings USING gin (english_name gin_trgm_ops)",
            )
            .await?;

        // Vector similarity index
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_medication_mappings_embedding ON medication_mappings USING hnsw (embedding vector_cosine_ops)",
            )
            .await?;

        // Added check constraints for non-empty names
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_mappings ADD CONSTRAINT check_arabic_name_not_empty CHECK (length(trim(arabic_name)) > 0)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_mappings ADD CONSTRAINT check_english_name_not_empty CHECK (length(trim(english_name)) > 0)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MedicationMappings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MedicationMappings {
    Table,
    Id,
    ArabicName,
    EnglishName,
    // Synonyms and Embedding are added via raw SQL for special type support
    CreatedAt,
    UpdatedAt,
}
