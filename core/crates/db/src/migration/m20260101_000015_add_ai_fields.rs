//! Migration: Add AI-related fields to matches, feedback_records, and weight_history tables

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Add columns to matches table
        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(ColumnDef::new(Matches::AiStatus).string_len(20))
                    .add_column(ColumnDef::new(Matches::AiConfidence).double())
                    .add_column(ColumnDef::new(Matches::AiExplanation).text())
                    .to_owned(),
            )
            .await?;

        // 2. Add column to feedback_records table
        manager
            .alter_table(
                Table::alter()
                    .table(FeedbackRecords::Table)
                    .add_column(ColumnDef::new(FeedbackRecords::AiLogicScore).double())
                    .to_owned(),
            )
            .await?;

        // 3. Add column to weight_history table
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .add_column(ColumnDef::new(WeightHistory::AiLogicWeight).double())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop columns in weight_history
        manager
            .alter_table(
                Table::alter()
                    .table(WeightHistory::Table)
                    .drop_column(WeightHistory::AiLogicWeight)
                    .to_owned(),
            )
            .await?;

        // Drop columns in feedback_records
        manager
            .alter_table(
                Table::alter()
                    .table(FeedbackRecords::Table)
                    .drop_column(FeedbackRecords::AiLogicScore)
                    .to_owned(),
            )
            .await?;

        // Drop columns in matches
        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(Matches::AiExplanation)
                    .drop_column(Matches::AiConfidence)
                    .drop_column(Matches::AiStatus)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Matches {
    Table,
    AiStatus,
    AiConfidence,
    AiExplanation,
}

#[derive(DeriveIden)]
enum FeedbackRecords {
    Table,
    AiLogicScore,
}

#[derive(DeriveIden)]
enum WeightHistory {
    Table,
    AiLogicWeight,
}
