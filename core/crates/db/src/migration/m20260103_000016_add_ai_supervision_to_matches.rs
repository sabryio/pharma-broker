//! Migration: Add AI supervision fields to matches table
//!
//! Adds columns for tracking AI auto-approval decisions and human overrides.
//! Requirements: 2.1, 2.2, 4.1

use sea_orm_migration::prelude::*;

use super::m20251221_000005_create_matches::Matches;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add AI auto-approval tracking columns
        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiAutoApproved)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiApprovedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add override tracking columns
        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiOverridden)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiOverrideBy)
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiOverrideReason)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .add_column(
                        ColumnDef::new(MatchesAiSupervision::AiOverrideAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for querying AI auto-approved matches
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_ai_auto_approved")
                    .table(Matches::Table)
                    .col(MatchesAiSupervision::AiAutoApproved)
                    .to_owned(),
            )
            .await?;

        // Create index for querying overridden matches
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_matches_ai_overridden")
                    .table(Matches::Table)
                    .col(MatchesAiSupervision::AiOverridden)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_matches_ai_overridden")
                    .table(Matches::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_matches_ai_auto_approved")
                    .table(Matches::Table)
                    .to_owned(),
            )
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiOverrideAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiOverrideReason)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiOverrideBy)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiOverridden)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiApprovedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Matches::Table)
                    .drop_column(MatchesAiSupervision::AiAutoApproved)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

/// New columns for AI supervision on the matches table
#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
pub enum MatchesAiSupervision {
    /// Whether this match was auto-approved by AI
    AiAutoApproved,
    /// Timestamp when AI auto-approved the match
    AiApprovedAt,
    /// Whether a human has overridden the AI decision
    AiOverridden,
    /// User ID of who overrode the AI decision
    AiOverrideBy,
    /// Reason provided for the override
    AiOverrideReason,
    /// Timestamp when the override occurred
    AiOverrideAt,
}
