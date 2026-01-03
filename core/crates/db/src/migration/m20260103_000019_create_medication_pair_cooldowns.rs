//! Migration: Create medication_pair_cooldowns table
//!
//! Tracks cooldown periods for medication pairs after overrides.
//! Requirements: 4.3

use sea_orm_migration::prelude::*;

use super::m20251221_000005_create_matches::Matches;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MedicationPairCooldowns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::OfferMedication)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::RequestMedication)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::CooldownUntil)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::OverrideMatchId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MedicationPairCooldowns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cooldowns_override_match")
                            .from(
                                MedicationPairCooldowns::Table,
                                MedicationPairCooldowns::OverrideMatchId,
                            )
                            .to(Matches::Table, Matches::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for medication pair lookups (most common query)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cooldowns_medications")
                    .table(MedicationPairCooldowns::Table)
                    .col(MedicationPairCooldowns::OfferMedication)
                    .col(MedicationPairCooldowns::RequestMedication)
                    .to_owned(),
            )
            .await?;

        // Index for expiration queries (cleanup of expired cooldowns)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cooldowns_until")
                    .table(MedicationPairCooldowns::Table)
                    .col(MedicationPairCooldowns::CooldownUntil)
                    .to_owned(),
            )
            .await?;

        // Unique constraint to prevent duplicate cooldowns for same pair
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cooldowns_pair_unique")
                    .table(MedicationPairCooldowns::Table)
                    .col(MedicationPairCooldowns::OfferMedication)
                    .col(MedicationPairCooldowns::RequestMedication)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MedicationPairCooldowns::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
pub enum MedicationPairCooldowns {
    Table,
    /// Unique identifier
    Id,
    /// Medication name from the offer
    OfferMedication,
    /// Medication name from the request
    RequestMedication,
    /// When the cooldown expires
    CooldownUntil,
    /// Reference to the match that triggered the cooldown
    OverrideMatchId,
    /// When the cooldown was created
    CreatedAt,
}
