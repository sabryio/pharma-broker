//! Time Series Analysis
//! Port of: 06_time_series.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

pub struct TimeSeriesAnalysis;

#[async_trait]
impl AnalysisPhase for TimeSeriesAnalysis {
    fn name(&self) -> &'static str {
        "Time Series Analysis"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Time Series Analysis");

        self.analyze_message_volume(db).await?;
        self.analyze_processing_rate(db).await?;
        self.analyze_match_creation(db).await?;
        self.analyze_daily_activity(db).await?;

        Ok(())
    }
}

impl TimeSeriesAnalysis {
    async fn analyze_message_volume(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Message Volume by Day (Last 10 Days)");

        let sql = r#"
            SELECT DATE(timestamp) as date, 
                   COUNT(*) as messages,
                   COUNT(DISTINCT group_jid) as groups,
                   COUNT(DISTINCT sender_phone) as senders
            FROM raw_messages 
            GROUP BY DATE(timestamp) 
            ORDER BY date DESC
            LIMIT 10
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No message data found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["Date", "Messages", "Groups", "Senders"]);

        let mut total_messages: i64 = 0;
        let mut days = 0;

        for row in &rows {
            let date: chrono::NaiveDate = row.try_get_by_index(0)?;
            let messages: i64 = row.try_get_by_index(1)?;
            let groups: i64 = row.try_get_by_index(2)?;
            let senders: i64 = row.try_get_by_index(3)?;

            total_messages += messages;
            days += 1;

            table.add_row(row![date.to_string(), fmt_num(messages), groups, senders]);
        }

        table.printstd();

        if days > 0 {
            let avg = total_messages / days;
            println!("\n📊 Average daily messages: {}", fmt_num(avg));
        }

        Ok(())
    }

    async fn analyze_processing_rate(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Processing Success Rate (Last 10 Days)");

        let sql = r#"
            SELECT DATE(timestamp) as date, 
                   COUNT(*) as total,
                   SUM(CASE WHEN processed_at IS NOT NULL THEN 1 ELSE 0 END) as processed,
                   SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END) as errors
            FROM raw_messages 
            GROUP BY DATE(timestamp) 
            ORDER BY date DESC
            LIMIT 10
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No processing data found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row!["Date", "Total", "Processed", "Errors", "Success %"]);

        let mut total_success_rate = 0.0;
        let mut days = 0;

        for row in &rows {
            let date: chrono::NaiveDate = row.try_get_by_index(0)?;
            let total: i64 = row.try_get_by_index(1)?;
            let processed: i64 = row.try_get_by_index(2)?;
            let errors: i64 = row.try_get_by_index(3)?;

            let success_rate = if total > 0 {
                (processed as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            total_success_rate += success_rate;
            days += 1;

            let rate_display = if success_rate >= 90.0 {
                format!("{:.1}%", success_rate).green()
            } else if success_rate >= 70.0 {
                format!("{:.1}%", success_rate).yellow()
            } else {
                format!("{:.1}%", success_rate).red()
            };

            table.add_row(row![
                date.to_string(),
                fmt_num(total),
                fmt_num(processed),
                errors,
                rate_display
            ]);
        }

        table.printstd();

        if days > 0 {
            let avg = total_success_rate / days as f64;
            println!("\n📊 Average success rate: {:.1}%", avg);
        }

        Ok(())
    }

    async fn analyze_match_creation(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Match Creation by Day (Last 10 Days)");

        let sql = r#"
            SELECT DATE(created_at) as date, 
                   COUNT(*) as matches,
                   ROUND(AVG(score)::numeric, 3) as avg_score,
                   SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed,
                   SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected
            FROM matches 
            GROUP BY DATE(created_at) 
            ORDER BY date DESC
            LIMIT 10
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No match data found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "Date",
            "Matches",
            "Avg Score",
            "Confirmed",
            "Rejected"
        ]);

        for row in &rows {
            let date: chrono::NaiveDate = row.try_get_by_index(0)?;
            let matches: i64 = row.try_get_by_index(1)?;
            let avg_score: f64 = row.try_get_by_index(2).unwrap_or(0.0);
            let confirmed: i64 = row.try_get_by_index(3)?;
            let rejected: i64 = row.try_get_by_index(4)?;

            table.add_row(row![
                date.to_string(),
                fmt_num(matches),
                format!("{:.3}", avg_score),
                confirmed,
                rejected
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_daily_activity(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Activity by Hour of Day");

        let sql = r#"
            SELECT EXTRACT(HOUR FROM timestamp)::int as hour,
                   COUNT(*) as messages
            FROM raw_messages
            WHERE timestamp > NOW() - INTERVAL '7 days'
            GROUP BY 1
            ORDER BY 1
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No recent activity data.");
            return Ok(());
        }

        let max_count: i64 = rows
            .iter()
            .map(|r| r.try_get_by_index::<i64>(1).unwrap_or(0))
            .max()
            .unwrap_or(1);

        println!("Hour  Messages  Distribution");
        println!("{}", "-".repeat(50));

        for row in &rows {
            let hour: i32 = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;

            let bar_width = ((count as f64 / max_count as f64) * 30.0) as usize;
            let bar = "█".repeat(bar_width);

            println!("{:02}:00 {:>8}  {}", hour, count, bar.cyan());
        }

        Ok(())
    }
}
