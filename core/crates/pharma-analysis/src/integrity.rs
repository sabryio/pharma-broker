//! Referential Integrity Check
//! Port of: 04_referential_integrity.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header};

/// Foreign key relationship definition
struct FkRelationship {
    child_table: &'static str,
    child_col: &'static str,
    parent_table: &'static str,
    parent_col: &'static str,
}

pub struct IntegrityAnalysis;

#[async_trait]
impl AnalysisPhase for IntegrityAnalysis {
    fn name(&self) -> &'static str {
        "Referential Integrity"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Referential Integrity Check");

        let relationships = get_fk_relationships();
        let mut table = Table::new();
        table.add_row(row![
            "Relationship",
            "Total Refs",
            "Orphaned",
            "Orphan %",
            "Status"
        ]);

        let mut total_orphans: i64 = 0;

        for rel in &relationships {
            let desc = format!(
                "{}.{} → {}.{}",
                rel.child_table, rel.child_col, rel.parent_table, rel.parent_col
            );

            // Count orphaned records
            let orphan_sql = format!(
                r#"
                SELECT COUNT(*) FROM "{}" c
                WHERE c."{}" IS NOT NULL
                AND NOT EXISTS (
                    SELECT 1 FROM "{}" p
                    WHERE p."{}" = c."{}"
                )
                "#,
                rel.child_table, rel.child_col, rel.parent_table, rel.parent_col, rel.child_col
            );

            let orphan_count: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, orphan_sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            // Count total with FK
            let total_sql = format!(
                r#"SELECT COUNT(*) FROM "{}" WHERE "{}" IS NOT NULL"#,
                rel.child_table, rel.child_col
            );

            let total: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, total_sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            let orphan_pct = if total > 0 {
                (orphan_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            total_orphans += orphan_count;

            let status = if orphan_count == 0 {
                "✅ OK".green().to_string()
            } else {
                "❌ Orphans".red().to_string()
            };

            table.add_row(row![
                desc,
                fmt_num(total),
                fmt_num(orphan_count),
                format!("{:.2}%", orphan_pct),
                status
            ]);
        }

        table.printstd();

        // Summary
        if total_orphans > 0 {
            println!(
                "\n{}",
                format!("⚠️ TOTAL ORPHANED RECORDS: {}", total_orphans)
                    .yellow()
                    .bold()
            );
        } else {
            println!(
                "\n{}",
                "✅ All foreign key relationships are valid!".green()
            );
        }

        Ok(())
    }
}

fn get_fk_relationships() -> Vec<FkRelationship> {
    vec![
        FkRelationship {
            child_table: "offers",
            child_col: "raw_message_id",
            parent_table: "raw_messages",
            parent_col: "id",
        },
        FkRelationship {
            child_table: "requests",
            child_col: "raw_message_id",
            parent_table: "raw_messages",
            parent_col: "id",
        },
        FkRelationship {
            child_table: "matches",
            child_col: "offer_id",
            parent_table: "offers",
            parent_col: "id",
        },
        FkRelationship {
            child_table: "matches",
            child_col: "request_id",
            parent_table: "requests",
            parent_col: "id",
        },
        FkRelationship {
            child_table: "feedback_records",
            child_col: "match_id",
            parent_table: "matches",
            parent_col: "id",
        },
        FkRelationship {
            child_table: "review_queue",
            child_col: "raw_message_id",
            parent_table: "raw_messages",
            parent_col: "id",
        },
    ]
}
