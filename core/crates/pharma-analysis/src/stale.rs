//! Stale Matches Analysis
//! Port of: 14_stale_matches.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

pub struct StaleMatchesAnalysis;

#[async_trait]
impl AnalysisPhase for StaleMatchesAnalysis {
    fn name(&self) -> &'static str {
        "Stale Matches Analysis"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Stale Matches Analysis");

        self.get_match_stats(db).await?;
        self.get_age_distribution(db).await?;
        self.get_stale_samples(db).await?;
        self.show_cleanup_options(db).await?;

        Ok(())
    }
}

impl StaleMatchesAnalysis {
    async fn get_match_stats(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Match Status Overview");

        let sql = r#"
            SELECT 
                status,
                COUNT(*) as count,
                MIN(created_at) as oldest,
                MAX(created_at) as newest,
                ROUND(AVG(score)::numeric, 3) as avg_score
            FROM matches
            GROUP BY status
            ORDER BY count DESC
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No matches found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["Status", "Count", "Oldest", "Newest", "Avg Score"]);

        for row in &rows {
            let status: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let oldest: chrono::DateTime<chrono::Utc> = row.try_get_by_index(2)?;
            let newest: chrono::DateTime<chrono::Utc> = row.try_get_by_index(3)?;
            let avg_score: f64 = row.try_get_by_index(4).unwrap_or(0.0);

            let status_colored = match status.as_str() {
                "CONFIRMED" => status.green(),
                "REJECTED" => status.red(),
                "PENDING" => status.yellow(),
                _ => status.normal(),
            };

            table.add_row(row![
                status_colored,
                fmt_num(count),
                oldest.format("%Y-%m-%d").to_string(),
                newest.format("%Y-%m-%d").to_string(),
                format!("{:.3}", avg_score)
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn get_age_distribution(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Pending Matches by Age");

        let sql = r#"
            SELECT 
                age_bucket,
                COUNT(*) as count,
                ROUND(AVG(score)::numeric, 3) as avg_score
            FROM (
                SELECT 
                    CASE 
                        WHEN created_at > NOW() - INTERVAL '1 day' THEN '< 1 day'
                        WHEN created_at > NOW() - INTERVAL '3 days' THEN '1-3 days'
                        WHEN created_at > NOW() - INTERVAL '7 days' THEN '3-7 days'
                        WHEN created_at > NOW() - INTERVAL '14 days' THEN '1-2 weeks'
                        WHEN created_at > NOW() - INTERVAL '30 days' THEN '2-4 weeks'
                        ELSE '> 1 month'
                    END as age_bucket,
                    CASE 
                        WHEN created_at > NOW() - INTERVAL '1 day' THEN 1
                        WHEN created_at > NOW() - INTERVAL '3 days' THEN 2
                        WHEN created_at > NOW() - INTERVAL '7 days' THEN 3
                        WHEN created_at > NOW() - INTERVAL '14 days' THEN 4
                        WHEN created_at > NOW() - INTERVAL '30 days' THEN 5
                        ELSE 6
                    END as sort_order,
                    score
                FROM matches
                WHERE status = 'PENDING'
            ) sub
            GROUP BY age_bucket, sort_order
            ORDER BY sort_order
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("{}", "✅ No pending matches!".green());
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["Age Bucket", "Count", "Avg Score"]);

        for row in &rows {
            let bucket: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let avg_score: f64 = row.try_get_by_index(2).unwrap_or(0.0);

            let bucket_colored = if bucket.contains("month") || bucket.contains("week") {
                bucket.red()
            } else if bucket.contains("7 days") {
                bucket.yellow()
            } else {
                bucket.green()
            };

            table.add_row(row![
                bucket_colored,
                fmt_num(count),
                format!("{:.3}", avg_score)
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn get_stale_samples(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Sample Stale Matches (>7 days)");

        let sql = r#"
            SELECT 
                m.id, m.score, m.created_at,
                o.medication as offer_med,
                r.medication as request_med
            FROM matches m
            JOIN offers o ON m.offer_id = o.id
            JOIN requests r ON m.request_id = r.id
            WHERE m.status = 'PENDING'
              AND m.created_at < NOW() - INTERVAL '7 days'
            ORDER BY m.created_at ASC
            LIMIT 10
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("{}", "✅ No stale matches older than 7 days!".green());
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["ID", "Offer Med", "Request Med", "Score", "Age"]);

        for row in &rows {
            let id: String = row.try_get_by_index(0)?;
            let score: f64 = row.try_get_by_index(1)?;
            let created_at: chrono::DateTime<chrono::Utc> = row.try_get_by_index(2)?;
            let offer_med: String = row.try_get_by_index(3)?;
            let request_med: String = row.try_get_by_index(4)?;

            let age_days = (chrono::Utc::now() - created_at).num_days();

            table.add_row(row![
                truncate(&id, 8),
                truncate(&offer_med, 20),
                truncate(&request_med, 20),
                format!("{:.2}", score),
                format!("{}d", age_days).red()
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn show_cleanup_options(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Cleanup Options (Dry Run)");

        for days in [7, 14, 30] {
            let sql = format!(
                r#"
                SELECT COUNT(*) FROM matches
                WHERE status = 'PENDING' AND created_at < NOW() - INTERVAL '{} days'
                "#,
                days
            );

            let count: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            println!(
                "  Expire matches > {} days: {} would be affected",
                days,
                fmt_num(count).yellow()
            );
        }

        println!("\n💡 To expire stale matches, use: pharma-analysis expire --days <N>");

        Ok(())
    }

    /// Expire old pending matches (for CLI use)
    pub async fn expire_matches(
        db: &DatabaseConnection,
        days: i64,
        dry_run: bool,
    ) -> anyhow::Result<i64> {
        if dry_run {
            let sql = format!(
                r#"
                SELECT COUNT(*) FROM matches
                WHERE status = 'PENDING' AND created_at < NOW() - INTERVAL '{} days'
                "#,
                days
            );

            let count: i64 = db
                .query_one(Statement::from_string(DbBackend::Postgres, sql))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            Ok(count)
        } else {
            let sql = format!(
                r#"
                UPDATE matches
                SET status = 'EXPIRED', notes = 'Auto-expired: stale match'
                WHERE status = 'PENDING' AND created_at < NOW() - INTERVAL '{} days'
                "#,
                days
            );

            let result = db
                .execute(Statement::from_string(DbBackend::Postgres, sql))
                .await?;

            Ok(result.rows_affected() as i64)
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
