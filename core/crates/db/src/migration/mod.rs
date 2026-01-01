//! Database Migrations
//!
//! SeaORM migrations for schema management.

use sea_orm::DatabaseConnection;
pub use sea_orm_migration::prelude::*;

mod m20251221_000001_create_groups;
mod m20251221_000002_create_raw_messages;
mod m20251221_000003_create_offers;
mod m20251221_000004_create_requests;
mod m20251221_000005_create_matches;
mod m20251221_000006_create_medication_mappings;
mod m20251221_000007_create_match_queue;
mod m20251221_000008_create_review_queue;
mod m20251221_000009_create_audit_logs;
mod m20251221_000010_create_feedback_records;
mod m20251221_000011_create_weight_history;
mod m20251231_000012_create_participants;
mod m20251231_000013_create_medication_master;
mod m20251231_000014_create_medication_aliases;
mod m20260101_000015_add_ai_fields;
mod m20260101_000016_add_confirmed_match_count;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251221_000001_create_groups::Migration),
            Box::new(m20251231_000012_create_participants::Migration),
            Box::new(m20251221_000002_create_raw_messages::Migration),
            Box::new(m20251221_000003_create_offers::Migration),
            Box::new(m20251221_000004_create_requests::Migration),
            Box::new(m20251221_000005_create_matches::Migration),
            Box::new(m20251221_000006_create_medication_mappings::Migration),
            Box::new(m20251221_000007_create_match_queue::Migration),
            Box::new(m20251221_000008_create_review_queue::Migration),
            Box::new(m20251221_000009_create_audit_logs::Migration),
            Box::new(m20251221_000010_create_feedback_records::Migration),
            Box::new(m20251221_000011_create_weight_history::Migration),
            // Medication Curation System (Phase 1)
            Box::new(m20251231_000013_create_medication_master::Migration),
            Box::new(m20251231_000014_create_medication_aliases::Migration),
            // AI Matching & Reviewer (Phase 1)
            Box::new(m20260101_000015_add_ai_fields::Migration),
            // Many-to-Many Matching Support
            Box::new(m20260101_000016_add_confirmed_match_count::Migration),
        ]
    }
}

pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    Migrator::up(db, None).await
}
