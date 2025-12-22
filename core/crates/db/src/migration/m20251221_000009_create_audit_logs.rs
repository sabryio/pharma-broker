//! Migration: Create audit_logs table

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLogs::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(AuditLogs::Action).text().not_null())
                    .col(ColumnDef::new(AuditLogs::EntityType).text().not_null())
                    .col(ColumnDef::new(AuditLogs::EntityId).text().not_null())
                    .col(ColumnDef::new(AuditLogs::Actor).text().not_null())
                    .col(ColumnDef::new(AuditLogs::Details).json_binary())
                    .col(ColumnDef::new(AuditLogs::IpAddress).text())
                    .col(ColumnDef::new(AuditLogs::UserAgent).text())
                    .col(
                        ColumnDef::new(AuditLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_audit_logs_entity")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::EntityType)
                    .col(AuditLogs::EntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_audit_logs_actor")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::Actor)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_audit_logs_action")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::Action)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_audit_logs_created_at")
                    .table(AuditLogs::Table)
                    .col((AuditLogs::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
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
    Id,
    Action,
    EntityType,
    EntityId,
    Actor,
    Details,
    IpAddress,
    UserAgent,
    CreatedAt,
}
