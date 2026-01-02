//! Migration: Create match_audit_records table
//!
//! Stores complete snapshots of all inputs and parameters for debugging
//! and reproducibility. Designed to integrate with frontend debug recordings.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the match_audit_records table
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS match_audit_records (
                    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                    
                    -- Match identification
                    match_id UUID NOT NULL,
                    offer_id UUID NOT NULL,
                    request_id UUID NOT NULL,
                    
                    -- Pipeline version for reproducibility
                    pipeline_version TEXT NOT NULL DEFAULT '1.0.0',
                    
                    -- Complete snapshots (JSONB for flexibility)
                    offer_snapshot JSONB NOT NULL,
                    request_snapshot JSONB NOT NULL,
                    
                    -- Matching configuration at time of match
                    weights_snapshot JSONB NOT NULL,
                    config_snapshot JSONB,
                    
                    -- Score breakdown
                    score_breakdown JSONB NOT NULL,
                    final_score DOUBLE PRECISION NOT NULL,
                    
                    -- Pipeline execution trace
                    pipeline_stages JSONB NOT NULL,
                    
                    -- AI involvement
                    ai_involved BOOLEAN NOT NULL DEFAULT false,
                    ai_model TEXT,
                    ai_response JSONB,
                    ai_latency_ms INTEGER,
                    
                    -- Resolution path
                    resolution_stage TEXT NOT NULL,
                    resolution_details JSONB,
                    
                    -- Timing
                    total_latency_ms INTEGER NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    
                    -- Review outcome (updated after human review)
                    review_status TEXT,
                    reviewed_by UUID,
                    reviewed_at TIMESTAMPTZ,
                    review_notes TEXT,
                    
                    -- Session tracking for frontend debug recordings
                    session_id TEXT,
                    client_metadata JSONB
                )
                "#,
            )
            .await?;

        // Create indexes for common queries
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_match_id ON match_audit_records (match_id)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_offer_id ON match_audit_records (offer_id)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_request_id ON match_audit_records (request_id)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_created_at ON match_audit_records (created_at DESC)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_resolution_stage ON match_audit_records (resolution_stage)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_review_status ON match_audit_records (review_status) WHERE review_status IS NOT NULL",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_session_id ON match_audit_records (session_id) WHERE session_id IS NOT NULL",
            )
            .await?;

        // GIN index for JSONB queries
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_match_audit_pipeline_stages ON match_audit_records USING GIN (pipeline_stages)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MatchAuditRecords::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum MatchAuditRecords {
    Table,
}
