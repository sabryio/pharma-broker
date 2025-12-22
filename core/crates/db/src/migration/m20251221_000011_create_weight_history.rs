//! Migration: Create weight_history table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WeightHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WeightHistory::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::MedicationWeight)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::DosageWeight)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::QuantityWeight)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::PriceWeight)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::RecencyWeight)
                            .double()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WeightHistory::Source).text().not_null())
                    .col(
                        ColumnDef::new(WeightHistory::SampleCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(WeightHistory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for getting most recent weights
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_weight_history_created_at")
                    .table(WeightHistory::Table)
                    .col((WeightHistory::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Insert default initial weights
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO weight_history (medication_weight, dosage_weight, quantity_weight, price_weight, recency_weight, source, sample_count)
                SELECT 0.35, 0.20, 0.15, 0.15, 0.15, 'initial', 0
                WHERE NOT EXISTS (SELECT 1 FROM weight_history)
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WeightHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WeightHistory {
    Table,
    Id,
    MedicationWeight,
    DosageWeight,
    QuantityWeight,
    PriceWeight,
    RecencyWeight,
    Source,
    SampleCount,
    CreatedAt,
}
