//! Migration: Create feedback_records table

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
                    .table(FeedbackRecords::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FeedbackRecords::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(FeedbackRecords::MatchId).uuid().not_null())
                    .col(ColumnDef::new(FeedbackRecords::UserId).text().not_null())
                    .col(
                        ColumnDef::new(FeedbackRecords::Confirmed)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::MedicationScore)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::DosageScore)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::QuantityScore)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::PriceScore)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::RecencyScore)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedbackRecords::TotalScore)
                            .double()
                            .not_null(),
                    )
                    // AI Logic score
                    .col(ColumnDef::new(FeedbackRecords::AiLogicScore).double())
                    .col(
                        ColumnDef::new(FeedbackRecords::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_feedback_records_match")
                            .from(FeedbackRecords::Table, FeedbackRecords::MatchId)
                            .to(Matches::Table, Matches::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_feedback_records_created_at")
                    .table(FeedbackRecords::Table)
                    .col(FeedbackRecords::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_feedback_records_confirmed")
                    .table(FeedbackRecords::Table)
                    .col(FeedbackRecords::Confirmed)
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one feedback per match (this also serves as an index for match_id Lookups)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_feedback_records_match_unique")
                    .table(FeedbackRecords::Table)
                    .col(FeedbackRecords::MatchId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Covering index for statistics queries
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_feedback_records_stats_covering ON feedback_records (created_at, total_score)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FeedbackRecords::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum FeedbackRecords {
    Table,
    Id,
    MatchId,
    UserId,
    Confirmed,
    MedicationScore,
    DosageScore,
    QuantityScore,
    PriceScore,
    RecencyScore,
    TotalScore,
    // AI Logic score
    AiLogicScore,
    CreatedAt,
}
