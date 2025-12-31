//! Migration: Create matches table

use sea_orm_migration::prelude::*;

use super::m20251221_000003_create_offers::Offers;
use super::m20251221_000004_create_requests::Requests;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Matches::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Matches::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Matches::OfferId).uuid().not_null())
                    .col(ColumnDef::new(Matches::RequestId).uuid().not_null())
                    .col(ColumnDef::new(Matches::Score).double().not_null())
                    .col(ColumnDef::new(Matches::Reasoning).text())
                    .col(ColumnDef::new(Matches::MatchedBy).string_len(50))
                    .col(
                        ColumnDef::new(Matches::Status)
                            .string_len(20)
                            .not_null()
                            .default("PENDING"),
                    )
                    .col(
                        ColumnDef::new(Matches::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Matches::ConfirmedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Matches::Notes).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_matches_offer")
                            .from(Matches::Table, Matches::OfferId)
                            .to(Offers::Table, Offers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_matches_request")
                            .from(Matches::Table, Matches::RequestId)
                            .to(Requests::Table, Requests::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint on offer_id + request_id
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_offer_request_unique")
                    .table(Matches::Table)
                    .col(Matches::OfferId)
                    .col(Matches::RequestId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_status")
                    .table(Matches::Table)
                    .col(Matches::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_offer_id")
                    .table(Matches::Table)
                    .col(Matches::OfferId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_request_id")
                    .table(Matches::Table)
                    .col(Matches::RequestId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Matches::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Matches {
    Table,
    Id,
    OfferId,
    RequestId,
    Score,
    Reasoning,
    MatchedBy,
    Status,
    CreatedAt,
    ConfirmedAt,
    Notes,
}
