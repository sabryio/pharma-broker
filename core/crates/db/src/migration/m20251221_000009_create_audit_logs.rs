//! Migration: Create audit_logs table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create partitioned table using raw SQL as SeaORM builder doesn't natively support PARTITION BY in all versions
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS audit_logs (
                    id UUID NOT NULL DEFAULT gen_random_uuid(),
                    action TEXT NOT NULL,
                    entity_type TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    actor TEXT NOT NULL,
                    details JSONB,
                    ip_address TEXT,
                    user_agent TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (id, created_at)
                ) PARTITION BY RANGE (created_at)",
            )
            .await?;

        // Create a default partition to handle data outside specific ranges
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS audit_logs_default PARTITION OF audit_logs DEFAULT",
            )
            .await?;

        // Indexes (Primary key (id, created_at) is already an index)
        manager
            .get_connection()
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs (entity_type, entity_id)")
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_audit_logs_actor ON audit_logs (actor)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs (action)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum AuditLogs {
    Table,
}
