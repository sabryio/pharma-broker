//! Migration: Add confirmed_match_count to offers and requests
//!
//! Tracks how many confirmed matches each offer/request has for many-to-many matching.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add confirmed_match_count to offers
        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .add_column(
                        ColumnDef::new(Offers::ConfirmedMatchCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Add confirmed_match_count to requests
        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .add_column(
                        ColumnDef::new(Requests::ConfirmedMatchCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Offers::Table)
                    .drop_column(Offers::ConfirmedMatchCount)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Requests::Table)
                    .drop_column(Requests::ConfirmedMatchCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Offers {
    Table,
    ConfirmedMatchCount,
}

#[derive(DeriveIden)]
enum Requests {
    Table,
    ConfirmedMatchCount,
}
