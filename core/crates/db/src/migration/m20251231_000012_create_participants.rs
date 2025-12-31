//! Migration: Create participants and participant_groups tables

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create Participants table
        manager
            .create_table(
                Table::create()
                    .table(Participants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Participants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Participants::Jid)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Participants::Phone)
                            .string_len(50)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Participants::PushName).string_len(100))
                    .col(ColumnDef::new(Participants::DisplayName).string_len(100))
                    .col(ColumnDef::new(Participants::Label).string_len(50))
                    .col(ColumnDef::new(Participants::Notes).text())
                    .col(
                        ColumnDef::new(Participants::IsBlocked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Participants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Participants::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create ParticipantGroups junction table
        manager
            .create_table(
                Table::create()
                    .table(ParticipantGroups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ParticipantGroups::ParticipantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ParticipantGroups::GroupId).uuid().not_null())
                    .col(
                        ColumnDef::new(ParticipantGroups::LastSeenAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ParticipantGroups::ParticipantId)
                            .col(ParticipantGroups::GroupId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_participant_id")
                            .from(ParticipantGroups::Table, ParticipantGroups::ParticipantId)
                            .to(Participants::Table, Participants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_id")
                            .from(ParticipantGroups::Table, ParticipantGroups::GroupId)
                            .to(
                                super::m20251221_000001_create_groups::Groups::Table,
                                super::m20251221_000001_create_groups::Groups::Id,
                            )
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ParticipantGroups::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Participants::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Participants {
    Table,
    Id,
    Jid,
    Phone,
    PushName,
    DisplayName,
    Label,
    Notes,
    IsBlocked,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum ParticipantGroups {
    Table,
    ParticipantId,
    GroupId,
    LastSeenAt,
}
