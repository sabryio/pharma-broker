//! Data Quality Analysis
//! Port of: 02_null_analysis.py, 03_data_quality.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::collections::HashMap;

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

lazy_static::lazy_static! {
    /// Expected nullable fields by table (migrated from Python config)
    static ref EXPECTED_NULLS: HashMap<&'static str, Vec<&'static str>> = {
        let mut m = HashMap::new();
        m.insert("raw_messages", vec!["sender_name", "reply_to_id", "reply_to_content", "reply_to_sender", "processed_at", "error", "external_id"]);
        m.insert("offers", vec!["raw_message_id", "source_name", "group_name", "unit", "price", "expiry_date", "batch_number", "notes", "content_embedding"]);
        m.insert("requests", vec!["raw_message_id", "source_name", "group_name", "unit", "max_price", "notes", "content_embedding"]);
        m.insert("matches", vec!["reasoning", "matched_by", "confirmed_at", "notes"]);
        m.insert("groups", vec!["description", "last_message"]);
        m.insert("review_queue", vec!["reply_context", "failure_reason", "reviewed_by", "reviewed_at", "review_note", "corrected_items"]);
        m.insert("unmapped_medications", vec!["approved_name", "reviewed_at"]);
        m.insert("feedback_records", vec!["user_id"]);
        m.insert("weight_history", vec!["improvement", "notes", "performance_metrics"]);
        m.insert("audit_logs", vec!["entity_id", "old_value", "new_value", "details", "ip_address"]);
        m.insert("medication_mappings", vec!["embedding"]);
        m
    };

    /// Valid status values by table
    static ref VALID_STATUSES: HashMap<&'static str, Vec<&'static str>> = {
        let mut m = HashMap::new();
        m.insert("offers", vec!["ACTIVE", "MATCHED", "EXPIRED", "ARCHIVED"]);
        m.insert("requests", vec!["ACTIVE", "MATCHED", "EXPIRED", "ARCHIVED"]);
        m.insert("matches", vec!["PENDING", "CONFIRMED", "REJECTED", "EXPIRED"]);
        m.insert("review_queue", vec!["PENDING", "APPROVED", "REJECTED"]);
        m
    };
}

pub struct QualityAnalysis;

#[async_trait]
impl AnalysisPhase for QualityAnalysis {
    fn name(&self) -> &'static str {
        "Data Quality"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Data Quality Analysis");

        self.analyze_nulls(db).await?;
        self.analyze_duplicates(db).await?;
        self.analyze_statuses(db).await?;
        self.analyze_data_freshness(db).await?;

        Ok(())
    }
}

impl QualityAnalysis {
    async fn analyze_nulls(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Null Value Analysis");

        let tables = vec![
            "raw_messages",
            "offers",
            "requests",
            "matches",
            "groups",
            "review_queue",
            "medication_mappings",
            "audit_logs",
            "feedback_records",
            "weight_history",
        ];

        let mut table = Table::new();
        table.add_row(row![
            "Table",
            "Column",
            "Null Count",
            "Total",
            "Null %",
            "Status"
        ]);

        let mut unexpected_nulls = 0;

        for table_name in tables {
            // Get columns
            let cols_sql = format!(
                "SELECT column_name FROM information_schema.columns WHERE table_name = '{}' AND table_schema = 'public'",
                table_name
            );
            let rows = db
                .query_all(Statement::from_string(DbBackend::Postgres, cols_sql))
                .await?;

            // Get total row count
            let total_sql = format!("SELECT COUNT(*) FROM \"{}\"", table_name);
            let total: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, total_sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            if total == 0 {
                continue;
            }

            for row in rows {
                let col_name: String = row.try_get_by_index(0)?;
                let count_sql = format!(
                    "SELECT COUNT(*) FROM \"{}\" WHERE \"{}\" IS NULL",
                    table_name, col_name
                );
                let null_count: i64 = db
                    .query_one(Statement::from_string(DbBackend::Postgres, count_sql))
                    .await?
                    .map(|r| r.try_get_by_index(0).unwrap_or(0))
                    .unwrap_or(0);

                if null_count > 0 {
                    let expected = EXPECTED_NULLS
                        .get(table_name)
                        .map(|v| v.contains(&col_name.as_str()))
                        .unwrap_or(false);

                    let null_pct = (null_count as f64 / total as f64) * 100.0;

                    let status = if expected {
                        "Expected".green()
                    } else {
                        unexpected_nulls += 1;
                        "⚠️ Review".red().bold()
                    };

                    table.add_row(row![
                        table_name,
                        col_name,
                        fmt_num(null_count),
                        fmt_num(total),
                        format!("{:.1}%", null_pct),
                        status
                    ]);
                }
            }
        }

        table.printstd();

        if unexpected_nulls > 0 {
            println!(
                "\n{}",
                format!("⚠️ {} unexpected null columns found", unexpected_nulls)
                    .yellow()
                    .bold()
            );
        } else {
            println!("\n{}", "✅ All null values are expected".green());
        }

        Ok(())
    }

    async fn analyze_duplicates(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Duplicate Check");

        let checks = vec![
            ("raw_messages", "external_id", "Duplicate WhatsApp IDs"),
            (
                "offers",
                "raw_message_id, medication",
                "Duplicate offers/msg",
            ),
            (
                "requests",
                "raw_message_id, medication",
                "Duplicate requests/msg",
            ),
            ("matches", "offer_id, request_id", "Duplicate matches"),
            ("groups", "jid", "Duplicate group JIDs"),
            (
                "medication_mappings",
                "arabic_name",
                "Duplicate medication mappings",
            ),
        ];

        let mut table = Table::new();
        table.add_row(row!["Table", "Check", "Duplicates", "Status"]);

        let mut total_duplicates = 0;

        for (table_name, cols, desc) in checks {
            let first_col = cols.split(',').next().unwrap().trim();
            let sql = format!(
                "SELECT COUNT(*) FROM (SELECT {}, COUNT(*) FROM \"{}\" WHERE {} IS NOT NULL GROUP BY {} HAVING COUNT(*) > 1) as sub",
                cols, table_name, first_col, cols
            );

            let count: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            total_duplicates += count;

            let status = if count == 0 {
                "✅ OK".green()
            } else {
                "❌ FAIL".red().bold()
            };

            table.add_row(row![table_name, desc, fmt_num(count), status]);
        }

        table.printstd();

        if total_duplicates > 0 {
            println!(
                "\n{}",
                format!("⚠️ {} duplicate groups found", total_duplicates)
                    .yellow()
                    .bold()
            );
        } else {
            println!("\n{}", "✅ No duplicates found".green());
        }

        Ok(())
    }

    async fn analyze_statuses(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Status Validation");

        let mut table = Table::new();
        table.add_row(row!["Table", "Status", "Count", "Valid"]);

        let mut invalid_statuses = 0;

        for (table_name, valid_values) in VALID_STATUSES.iter() {
            let sql = format!(
                "SELECT status, COUNT(*) FROM \"{}\" GROUP BY status ORDER BY COUNT(*) DESC",
                table_name
            );
            let rows = db
                .query_all(Statement::from_string(DbBackend::Postgres, sql))
                .await?;

            for row in rows {
                let status: String = row
                    .try_get_by_index(0)
                    .unwrap_or_else(|_| "NULL".to_string());
                let count: i64 = row.try_get_by_index(1)?;
                let is_valid = valid_values.contains(&status.as_str());

                if !is_valid {
                    invalid_statuses += 1;
                }

                table.add_row(row![
                    table_name,
                    status,
                    fmt_num(count),
                    if is_valid { "✅".green() } else { "❌".red() }
                ]);
            }
        }

        table.printstd();

        if invalid_statuses > 0 {
            println!(
                "\n{}",
                format!("⚠️ {} invalid status values found", invalid_statuses)
                    .yellow()
                    .bold()
            );
        } else {
            println!("\n{}", "✅ All status values are valid".green());
        }

        Ok(())
    }

    async fn analyze_data_freshness(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Data Freshness");

        let checks = vec![
            ("raw_messages", "timestamp", "Last message"),
            ("offers", "created_at", "Last offer"),
            ("requests", "created_at", "Last request"),
            ("matches", "created_at", "Last match"),
        ];

        let mut table = Table::new();
        table.add_row(row!["Table", "Description", "Last Activity", "Age"]);

        for (table_name, col, desc) in checks {
            let sql = format!("SELECT MAX(\"{}\") FROM \"{}\"", col, table_name);

            let result = db
                .query_one(Statement::from_string(DbBackend::Postgres, sql))
                .await?;

            if let Some(row) = result
                && let Ok(last_time) = row.try_get_by_index::<chrono::DateTime<chrono::Utc>>(0)
            {
                let age = chrono::Utc::now() - last_time;
                let age_str = if age.num_days() > 0 {
                    format!("{} days ago", age.num_days())
                } else if age.num_hours() > 0 {
                    format!("{} hours ago", age.num_hours())
                } else {
                    format!("{} minutes ago", age.num_minutes())
                };

                let age_colored = if age.num_days() > 7 {
                    age_str.red()
                } else if age.num_days() > 1 {
                    age_str.yellow()
                } else {
                    age_str.green()
                };

                table.add_row(row![
                    table_name,
                    desc,
                    last_time.format("%Y-%m-%d %H:%M").to_string(),
                    age_colored
                ]);
            }
        }

        table.printstd();

        Ok(())
    }
}
