//! Migration: Create supervision_audit_log table
//!
//! Comprehensive audit logging for AI auto-approval decisions.
//! Requirements: 2.1, 2.2, 2.3

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
                    .table(SupervisionAuditLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SupervisionAuditLog::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(SupervisionAuditLog::MatchId).uuid().null())
                    .col(
                        ColumnDef::new(SupervisionAuditLog::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::EventType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::AiConfidence)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::AiExplanation)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::Decision)
                            .string_len(50)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::SafetyChecks)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::Overridden)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::OverrideBy)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::OverrideReason)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::OverrideAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SupervisionAuditLog::Metadata)
                            .json_binary()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_supervision_audit_match")
                            .from(SupervisionAuditLog::Table, SupervisionAuditLog::MatchId)
                            .to(Matches::Table, Matches::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for timestamp-based queries (most common for audit trails)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_supervision_audit_timestamp")
                    .table(SupervisionAuditLog::Table)
                    .col((SupervisionAuditLog::Timestamp, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Index for match_id lookups
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_supervision_audit_match_id")
                    .table(SupervisionAuditLog::Table)
                    .col(SupervisionAuditLog::MatchId)
                    .to_owned(),
            )
            .await?;

        // Index for event_type filtering
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_supervision_audit_event_type")
                    .table(SupervisionAuditLog::Table)
                    .col(SupervisionAuditLog::EventType)
                    .to_owned(),
            )
            .await?;

        // Composite index for common filter combinations
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_supervision_audit_type_timestamp")
                    .table(SupervisionAuditLog::Table)
                    .col(SupervisionAuditLog::EventType)
                    .col((SupervisionAuditLog::Timestamp, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Index for override status filtering
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_supervision_audit_overridden")
                    .table(SupervisionAuditLog::Table)
                    .col(SupervisionAuditLog::Overridden)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SupervisionAuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum SupervisionAuditLog {
    Table,
    /// Unique identifier for the audit entry
    Id,
    /// Reference to the match (nullable for config changes)
    MatchId,
    /// When the event occurred
    Timestamp,
    /// Type of event: AutoApproved, QueuedForReview, Blocked, Overridden, etc.
    EventType,
    /// AI confidence score at time of decision
    AiConfidence,
    /// AI explanation/reasoning
    AiExplanation,
    /// Decision made: Approved, QueuedForReview, Blocked
    Decision,
    /// JSON array of safety check results
    SafetyChecks,
    /// Whether this decision was later overridden
    Overridden,
    /// User ID who performed the override
    OverrideBy,
    /// Reason provided for override
    OverrideReason,
    /// When the override occurred
    OverrideAt,
    /// Additional metadata (JSON)
    Metadata,
}
