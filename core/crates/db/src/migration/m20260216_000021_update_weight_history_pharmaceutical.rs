//! Migration: Update weight_history table for pharmaceutical validation
//!
//! Changes:
//! 1. Add pharmaceutical_weight column
//! 2. Add expiry_weight column (for future use)
//! 3. Add supplier_weight column (for future use)
//! 4. Drop deprecated columns: dosage_weight, quantity_weight, price_weight

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add new columns
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .add_column(
                        ColumnDef::new(WeightHistory::PharmaceuticalWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .add_column(
                        ColumnDef::new(WeightHistory::ExpiryWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .add_column(
                        ColumnDef::new(WeightHistory::SupplierWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await?;

        // Update existing records to have pharmaceutical weight of 0.20 if medication weight is 0.55 or higher
        // This migrates old records to the new weight distribution
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE weight_history
                SET pharmaceutical_weight = 0.20
                WHERE medication_weight >= 0.55
                AND pharmaceutical_weight = 0.0
                "#,
            )
            .await?;

        // Drop deprecated columns
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .drop_column(WeightHistory::DosageWeight)
                    .drop_column(WeightHistory::QuantityWeight)
                    .drop_column(WeightHistory::PriceWeight)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add deprecated columns
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .add_column(
                        ColumnDef::new(WeightHistory::DosageWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .add_column(
                        ColumnDef::new(WeightHistory::QuantityWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .add_column(
                        ColumnDef::new(WeightHistory::PriceWeight)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await?;

        // Drop new columns
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .drop_column(WeightHistory::PharmaceuticalWeight)
                    .drop_column(WeightHistory::ExpiryWeight)
                    .drop_column(WeightHistory::SupplierWeight)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum WeightHistory {
    Table,
    // Deprecated columns (for rollback)
    DosageWeight,
    QuantityWeight,
    PriceWeight,
    // New columns
    PharmaceuticalWeight,
    ExpiryWeight,
    SupplierWeight,
}
