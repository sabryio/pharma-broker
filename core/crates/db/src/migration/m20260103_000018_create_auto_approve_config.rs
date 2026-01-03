//! Migration: Create auto_approve_config table
//!
//! Configuration storage for AI auto-approval settings.
//! Requirements: 5.1, 5.2, 5.3, 5.4, 5.5

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AutoApproveConfig::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AutoApproveConfig::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::ConfidenceThreshold)
                            .double()
                            .not_null()
                            .default(0.85),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::BatchSize)
                            .integer()
                            .not_null()
                            .default(50),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::ProcessingIntervalSecs)
                            .integer()
                            .not_null()
                            .default(30),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::UndoWindowMins)
                            .integer()
                            .not_null()
                            .default(30),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::OverrideRatePauseThreshold)
                            .double()
                            .not_null()
                            .default(0.10),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::ConsecutiveOverrideLimit)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::OverrideCooldownMins)
                            .integer()
                            .not_null()
                            .default(60),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::CategoryThresholds)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::Schedule)
                            .string_len(100)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(AutoApproveConfig::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(AutoApproveConfig::UpdatedBy).uuid().null())
                    .to_owned(),
            )
            .await?;

        // Insert default configuration row
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO auto_approve_config (
                    id, enabled, confidence_threshold, batch_size, 
                    processing_interval_secs, undo_window_mins,
                    override_rate_pause_threshold, consecutive_override_limit,
                    override_cooldown_mins, category_thresholds
                ) VALUES (
                    gen_random_uuid(), false, 0.85, 50, 
                    30, 30, 0.10, 5, 60, '{}'
                ) ON CONFLICT DO NOTHING",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AutoApproveConfig::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum AutoApproveConfig {
    Table,
    /// Unique identifier
    Id,
    /// Whether auto-approval is enabled globally
    Enabled,
    /// Minimum AI confidence for auto-approval (0.70-0.99)
    ConfidenceThreshold,
    /// Maximum batch size per processing cycle
    BatchSize,
    /// Processing interval in seconds
    ProcessingIntervalSecs,
    /// Undo window in minutes
    UndoWindowMins,
    /// Override rate threshold to pause (0.0-1.0)
    OverrideRatePauseThreshold,
    /// Consecutive overrides to disable
    ConsecutiveOverrideLimit,
    /// Cooldown period after override (minutes)
    OverrideCooldownMins,
    /// Category-specific threshold overrides (JSON)
    CategoryThresholds,
    /// Schedule for auto-approval (cron expression)
    Schedule,
    /// Last update timestamp
    UpdatedAt,
    /// User who last updated the config
    UpdatedBy,
}
