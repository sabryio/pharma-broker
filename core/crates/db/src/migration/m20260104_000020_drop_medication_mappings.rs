//! Migration: Drop medication_mappings table
//!
//! This migration removes the legacy medication_mappings table.
//! All functionality has been consolidated into medication_master.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the medication_mappings table
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("medication_mappings"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate the medication_mappings table (for rollback)
        // Enable pg_trgm extension for trigram search
        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("medication_mappings"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Alias::new("arabic_name")).text().not_null())
                    .col(ColumnDef::new(Alias::new("english_name")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
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

        // Add vector column for embeddings
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE medication_mappings ADD COLUMN IF NOT EXISTS embedding vector(768)",
            )
            .await?;

        Ok(())
    }
}
