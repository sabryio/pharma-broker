//! Migration: Add master_medication_id to offers and requests
//!
//! Links offers and requests to the medication_master table for deterministic matching.
//! This is Phase 1 of the Medication Curation System.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add master_medication_id column to offers
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("offers"))
                    .add_column(
                        ColumnDef::new(Alias::new("master_medication_id"))
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add medication_curated boolean to offers
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("offers"))
                    .add_column(
                        ColumnDef::new(Alias::new("medication_curated"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Add FK constraint for offers.master_medication_id
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE offers 
                 ADD CONSTRAINT fk_offers_master_medication 
                 FOREIGN KEY (master_medication_id) 
                 REFERENCES medication_master(id) 
                 ON DELETE SET NULL",
            )
            .await?;

        // Add index for offers.master_medication_id
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_offers_master_med 
                 ON offers(master_medication_id) 
                 WHERE master_medication_id IS NOT NULL",
            )
            .await?;

        // Add master_medication_id column to requests
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("requests"))
                    .add_column(
                        ColumnDef::new(Alias::new("master_medication_id"))
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add medication_curated boolean to requests
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("requests"))
                    .add_column(
                        ColumnDef::new(Alias::new("medication_curated"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Add FK constraint for requests.master_medication_id
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE requests 
                 ADD CONSTRAINT fk_requests_master_medication 
                 FOREIGN KEY (master_medication_id) 
                 REFERENCES medication_master(id) 
                 ON DELETE SET NULL",
            )
            .await?;

        // Add index for requests.master_medication_id
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_requests_master_med 
                 ON requests(master_medication_id) 
                 WHERE master_medication_id IS NOT NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop FK constraints
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE offers DROP CONSTRAINT IF EXISTS fk_offers_master_medication",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE requests DROP CONSTRAINT IF EXISTS fk_requests_master_medication",
            )
            .await?;

        // Drop columns from offers
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("offers"))
                    .drop_column(Alias::new("master_medication_id"))
                    .drop_column(Alias::new("medication_curated"))
                    .to_owned(),
            )
            .await?;

        // Drop columns from requests
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("requests"))
                    .drop_column(Alias::new("master_medication_id"))
                    .drop_column(Alias::new("medication_curated"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
