//! Migration: Create priority_medications table
//!
//! This table stores medications that should be processed with higher priority.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PriorityMedication::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PriorityMedication::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::MedicationName)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::MedicationNameAr)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::PriorityLevel)
                            .integer()
                            .not_null()
                            .default(3), // Normal priority
                    )
                    .col(ColumnDef::new(PriorityMedication::Reason).string().null())
                    .col(
                        ColumnDef::new(PriorityMedication::Active)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::ActiveFrom)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::ActiveUntil)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(PriorityMedication::CreatedBy).uuid().null())
                    .col(
                        ColumnDef::new(PriorityMedication::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PriorityMedication::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on medication_name for fast lookups
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_priority_medications_name")
                    .table(PriorityMedication::Table)
                    .col(PriorityMedication::MedicationName)
                    .to_owned(),
            )
            .await?;

        // Create index on active + active_from + active_until for filtering active priorities
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_priority_medications_active")
                    .table(PriorityMedication::Table)
                    .col(PriorityMedication::Active)
                    .col(PriorityMedication::ActiveFrom)
                    .col(PriorityMedication::ActiveUntil)
                    .to_owned(),
            )
            .await?;

        // Create index on priority_level for sorting
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_priority_medications_level")
                    .table(PriorityMedication::Table)
                    .col(PriorityMedication::PriorityLevel)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PriorityMedication::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PriorityMedication {
    Table,
    Id,
    MedicationName,
    MedicationNameAr,
    PriorityLevel,
    Reason,
    Active,
    ActiveFrom,
    ActiveUntil,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}
