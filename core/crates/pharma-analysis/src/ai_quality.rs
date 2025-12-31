//! AI Parsing Quality Assessment
//! Port of: 07_ai_parsing_quality.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

pub struct AiQualityAnalysis;

#[async_trait]
impl AnalysisPhase for AiQualityAnalysis {
    fn name(&self) -> &'static str {
        "AI Parsing Quality"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("AI Parsing Quality Assessment");

        self.analyze_completeness(db).await?;
        self.analyze_top_medications(db).await?;
        self.analyze_unmapped_medications(db).await?;
        self.analyze_review_queue(db).await?;
        self.analyze_parsing_errors(db).await?;

        Ok(())
    }
}

impl AiQualityAnalysis {
    async fn analyze_completeness(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Extraction Completeness");

        // Offers completeness
        let offers_sql = r#"
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication IS NOT NULL AND medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity IS NOT NULL AND quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN price IS NOT NULL AND price > 0 THEN 1 ELSE 0 END) as has_price,
                   SUM(CASE WHEN unit IS NOT NULL AND unit != '' THEN 1 ELSE 0 END) as has_unit,
                   SUM(CASE WHEN expiry_date IS NOT NULL THEN 1 ELSE 0 END) as has_expiry
            FROM offers
        "#;

        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                offers_sql.to_string(),
            ))
            .await?;

        if let Some(row) = row {
            let total: i64 = row.try_get_by_index(0)?;
            if total > 0 {
                println!("\n{} ({} total):", "OFFERS".cyan().bold(), total);
                let mut table = Table::new();
                table.add_row(row!["Field", "Count", "Percentage", "Status"]);

                let metrics = vec![
                    ("Medication", 1),
                    ("Quantity", 2),
                    ("Price", 3),
                    ("Unit", 4),
                    ("Expiry Date", 5),
                ];

                for (name, idx) in metrics {
                    let val: i64 = row.try_get_by_index(idx).unwrap_or(0);
                    let pct = (val as f64 / total as f64) * 100.0;
                    let status = get_status_icon(pct);
                    table.add_row(row![name, fmt_num(val), format!("{:.1}%", pct), status]);
                }
                table.printstd();
            }
        }

        // Requests completeness
        let req_sql = r#"
            SELECT COUNT(*) as total,
                   SUM(CASE WHEN medication IS NOT NULL AND medication != '' THEN 1 ELSE 0 END) as has_medication,
                   SUM(CASE WHEN quantity IS NOT NULL AND quantity > 0 THEN 1 ELSE 0 END) as has_quantity,
                   SUM(CASE WHEN max_price IS NOT NULL AND max_price > 0 THEN 1 ELSE 0 END) as has_max_price,
                   SUM(CASE WHEN unit IS NOT NULL AND unit != '' THEN 1 ELSE 0 END) as has_unit
            FROM requests
        "#;

        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                req_sql.to_string(),
            ))
            .await?;

        if let Some(row) = row {
            let total: i64 = row.try_get_by_index(0)?;
            if total > 0 {
                println!("\n{} ({} total):", "REQUESTS".cyan().bold(), total);
                let mut table = Table::new();
                table.add_row(row!["Field", "Count", "Percentage", "Status"]);

                let metrics = vec![
                    ("Medication", 1),
                    ("Quantity", 2),
                    ("Max Price", 3),
                    ("Unit", 4),
                ];

                for (name, idx) in metrics {
                    let val: i64 = row.try_get_by_index(idx).unwrap_or(0);
                    let pct = (val as f64 / total as f64) * 100.0;
                    let status = get_status_icon(pct);
                    table.add_row(row![name, fmt_num(val), format!("{:.1}%", pct), status]);
                }
                table.printstd();
            }
        }

        Ok(())
    }

    async fn analyze_top_medications(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Top 15 Medications");

        let sql = r#"
            SELECT medication, COUNT(*) as occurrences
            FROM (
                SELECT medication FROM offers WHERE medication IS NOT NULL AND medication != ''
                UNION ALL 
                SELECT medication FROM requests WHERE medication IS NOT NULL AND medication != ''
            ) combined
            GROUP BY medication 
            ORDER BY occurrences DESC 
            LIMIT 15
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No medication data found.");
            return Ok(());
        }

        let max_count: i64 = rows
            .first()
            .map(|r| r.try_get_by_index(1).unwrap_or(1))
            .unwrap_or(1);

        let mut table = Table::new();
        table.add_row(row!["Medication", "Count", "Distribution"]);

        for row in &rows {
            let name: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;

            let bar_width = ((count as f64 / max_count as f64) * 20.0) as usize;
            let bar = "█".repeat(bar_width);

            table.add_row(row![truncate(&name, 30), fmt_num(count), bar.cyan()]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_unmapped_medications(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Unmapped Medications");

        let sql = r#"
            SELECT raw_text, ai_output, count, reviewed, approved_name
            FROM unmapped_medications 
            ORDER BY count DESC 
            LIMIT 15
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("{}", "✅ No unmapped medications!".green());
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "Raw Text",
            "AI Output",
            "Count",
            "Reviewed",
            "Approved"
        ]);

        for row in &rows {
            let raw_text: String = row.try_get_by_index(0)?;
            let ai_output: String = row.try_get_by_index(1).unwrap_or_default();
            let count: i64 = row.try_get_by_index(2)?;
            let reviewed: bool = row.try_get_by_index(3).unwrap_or(false);
            let approved: Option<String> = row.try_get_by_index(4).ok();

            table.add_row(row![
                truncate(&raw_text, 25),
                truncate(&ai_output, 20),
                count,
                if reviewed { "✅" } else { "❌" },
                approved.unwrap_or_else(|| "-".to_string())
            ]);
        }

        table.printstd();

        // Summary
        let total_sql = "SELECT COUNT(*), SUM(count) FROM unmapped_medications";
        if let Some(row) = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                total_sql.to_string(),
            ))
            .await?
        {
            let unique: i64 = row.try_get_by_index(0)?;
            let total: i64 = row.try_get_by_index(1).unwrap_or(0);
            println!(
                "\n📊 {} unique unmapped medications ({} total occurrences)",
                unique, total
            );
        }

        Ok(())
    }

    async fn analyze_review_queue(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Review Queue Status");

        let sql = r#"
            SELECT status, COUNT(*) as count, 
                   ROUND(AVG(confidence)::numeric, 3) as avg_confidence,
                   MIN(created_at) as oldest
            FROM review_queue 
            GROUP BY status
            ORDER BY count DESC
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("{}", "✅ Review queue is empty!".green());
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["Status", "Count", "Avg Confidence", "Oldest"]);

        for row in &rows {
            let status: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let conf: f64 = row.try_get_by_index(2).unwrap_or(0.0);
            let oldest: chrono::DateTime<chrono::Utc> = row.try_get_by_index(3)?;

            let status_colored = match status.as_str() {
                "PENDING" => status.yellow(),
                "APPROVED" => status.green(),
                "REJECTED" => status.red(),
                _ => status.normal(),
            };

            table.add_row(row![
                status_colored,
                fmt_num(count),
                format!("{:.3}", conf),
                oldest.format("%Y-%m-%d").to_string()
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_parsing_errors(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Parsing Errors");

        let sql = r#"
            SELECT 
                COUNT(*) as total_errors,
                COUNT(DISTINCT error) as unique_errors
            FROM raw_messages 
            WHERE error IS NOT NULL
        "#;

        let row = db
            .query_one(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if let Some(row) = row {
            let total: i64 = row.try_get_by_index(0)?;
            let unique: i64 = row.try_get_by_index(1)?;

            if total == 0 {
                println!("{}", "✅ No parsing errors!".green());
                return Ok(());
            }

            println!("Total errors: {}", fmt_num(total).red());
            println!("Unique error types: {}", unique);

            // Top error types
            let errors_sql = r#"
                SELECT error, COUNT(*) as count
                FROM raw_messages 
                WHERE error IS NOT NULL
                GROUP BY error
                ORDER BY count DESC
                LIMIT 5
            "#;

            let rows = db
                .query_all(Statement::from_string(
                    DbBackend::Postgres,
                    errors_sql.to_string(),
                ))
                .await?;

            if !rows.is_empty() {
                println!("\nTop error types:");
                for row in &rows {
                    let error: String = row.try_get_by_index(0)?;
                    let count: i64 = row.try_get_by_index(1)?;
                    println!("  {} - {}", count, truncate(&error, 60));
                }
            }
        }

        Ok(())
    }
}

fn get_status_icon(pct: f64) -> colored::ColoredString {
    if pct >= 80.0 {
        "✅".green()
    } else if pct >= 50.0 {
        "⚠️".yellow()
    } else {
        "❌".red()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
