//! Matching Engine Analysis
//! Port of: 08_matching_analysis.py

use async_trait::async_trait;
use colored::*;
use pharma_db::DatabaseConnection;
use prettytable::{Table, row};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

use crate::{AnalysisPhase, fmt_num, print_header, print_subheader};

pub struct MatchingAnalysis;

#[async_trait]
impl AnalysisPhase for MatchingAnalysis {
    fn name(&self) -> &'static str {
        "Matching Engine Analysis"
    }

    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_header("Matching Engine Analysis");

        self.diagnose_scores(db).await?;
        self.analyze_scores(db).await?;
        self.analyze_outcomes(db).await?;
        self.analyze_feedback(db).await?;
        self.analyze_weights(db).await?;
        self.analyze_accuracy(db).await?;

        Ok(())
    }
}

impl MatchingAnalysis {
    /// Diagnostic: Check actual match data to identify score storage issues
    async fn diagnose_scores(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Match Data Diagnostic (Sample)");

        // Get 5 sample matches to inspect raw data
        let sql = r#"
            SELECT id, score, status, 
                   offer_id, request_id, 
                   reasoning,
                   created_at
            FROM matches 
            ORDER BY created_at DESC 
            LIMIT 5
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No matches found in database.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "ID",
            "Score",
            "Status",
            "Offer ID",
            "Request ID",
            "Created"
        ]);

        for row in &rows {
            let id: uuid::Uuid = row.try_get_by_index(0)?;
            let score: f64 = row.try_get_by_index(1).unwrap_or(0.0);
            let status: String = row.try_get_by_index(2)?;
            let offer_id: uuid::Uuid = row.try_get_by_index(3)?;
            let request_id: uuid::Uuid = row.try_get_by_index(4)?;
            let created: chrono::DateTime<chrono::Utc> = row.try_get_by_index(6)?;

            let id_str = id.to_string();
            let offer_str = offer_id.to_string();
            let request_str = request_id.to_string();

            let score_colored = if score >= 0.9 {
                format!("{:.4}", score).green()
            } else if score >= 0.7 {
                format!("{:.4}", score).yellow()
            } else if score > 0.0 {
                format!("{:.4}", score).normal()
            } else {
                format!("{:.4}", score).red().bold()
            };

            table.add_row(row![
                &id_str[..8.min(id_str.len())],
                score_colored,
                status,
                &offer_str[..8.min(offer_str.len())],
                &request_str[..8.min(request_str.len())],
                created.format("%Y-%m-%d %H:%M").to_string()
            ]);
        }

        table.printstd();

        // Count matches with zero scores
        let zero_sql = "SELECT COUNT(*) FROM matches WHERE score = 0";
        let zero_count: i64 = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                zero_sql.to_string(),
            ))
            .await?
            .map(|r| r.try_get_by_index(0).unwrap_or(0))
            .unwrap_or(0);

        let total_sql = "SELECT COUNT(*) FROM matches";
        let total: i64 = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                total_sql.to_string(),
            ))
            .await?
            .map(|r| r.try_get_by_index(0).unwrap_or(0))
            .unwrap_or(0);

        if zero_count > 0 {
            println!(
                "\n{} {}/{} matches have score = 0 ({:.1}%)",
                "⚠️ WARNING:".red().bold(),
                fmt_num(zero_count),
                fmt_num(total),
                (zero_count as f64 / total as f64) * 100.0
            );
            println!(
                "This indicates match scores were not properly saved when matches were created."
            );
        } else {
            println!("\n{} All matches have non-zero scores.", "✅".green());
        }

        Ok(())
    }

    async fn analyze_scores(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Score Distribution by Confidence Band");

        let sql = r#"
            SELECT
                CASE
                    WHEN score >= 0.9 THEN '0.9-1.0 (AUTO)'
                    WHEN score >= 0.7 THEN '0.7-0.9 (SUGGEST)'
                    WHEN score >= 0.5 THEN '0.5-0.7 (REVIEW)'
                    ELSE '0.0-0.5 (NONE)'
                END as score_band,
                COUNT(*) as count,
                ROUND(AVG(score)::numeric, 3) as avg_score,
                SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed,
                SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected,
                SUM(CASE WHEN status = 'PENDING' THEN 1 ELSE 0 END) as pending
            FROM matches
            GROUP BY 1
            ORDER BY avg_score DESC
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No matches found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "Score Band",
            "Count",
            "Avg Score",
            "Confirmed",
            "Rejected",
            "Pending",
            "Confirm Rate"
        ]);

        for row in &rows {
            let band: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let avg_dec: rust_decimal::Decimal = row.try_get_by_index(2).unwrap_or_default();
            let avg: f64 = avg_dec.try_into().unwrap_or(0.0);
            let confirmed: i64 = row.try_get_by_index(3).unwrap_or(0);
            let rejected: i64 = row.try_get_by_index(4).unwrap_or(0);
            let pending: i64 = row.try_get_by_index(5).unwrap_or(0);

            let decided = confirmed + rejected;
            let rate = if decided > 0 {
                (confirmed as f64 / decided as f64) * 100.0
            } else {
                0.0
            };

            let rate_colored = if rate >= 80.0 {
                format!("{:.1}%", rate).green()
            } else if rate >= 50.0 {
                format!("{:.1}%", rate).yellow()
            } else {
                format!("{:.1}%", rate).red()
            };

            table.add_row(row![
                band,
                fmt_num(count),
                format!("{:.3}", avg),
                confirmed,
                rejected,
                pending,
                rate_colored
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_outcomes(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Match Outcomes Summary");

        let sql = r#"
            SELECT status, COUNT(*) as count,
                   ROUND(AVG(score)::numeric, 3) as avg_score,
                   ROUND(MIN(score)::numeric, 3) as min_score,
                   ROUND(MAX(score)::numeric, 3) as max_score,
                   ROUND(STDDEV(score)::numeric, 3) as stddev_score
            FROM matches 
            GROUP BY status
            ORDER BY count DESC
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        let mut table = Table::new();
        table.add_row(row!["Status", "Count", "Avg Score", "Min", "Max", "StdDev"]);

        for row in &rows {
            let status: String = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let avg_dec: rust_decimal::Decimal = row.try_get_by_index(2).unwrap_or_default();
            let min_dec: rust_decimal::Decimal = row.try_get_by_index(3).unwrap_or_default();
            let max_dec: rust_decimal::Decimal = row.try_get_by_index(4).unwrap_or_default();
            let stddev_dec: rust_decimal::Decimal = row.try_get_by_index(5).unwrap_or_default();

            let avg: f64 = avg_dec.try_into().unwrap_or(0.0);
            let min: f64 = min_dec.try_into().unwrap_or(0.0);
            let max: f64 = max_dec.try_into().unwrap_or(0.0);
            let stddev: f64 = stddev_dec.try_into().unwrap_or(0.0);

            let status_colored = match status.as_str() {
                "CONFIRMED" => status.green(),
                "REJECTED" => status.red(),
                "PENDING" => status.yellow(),
                _ => status.normal(),
            };

            table.add_row(row![
                status_colored,
                fmt_num(count),
                format!("{:.3}", avg),
                format!("{:.3}", min),
                format!("{:.3}", max),
                format!("{:.3}", stddev)
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_feedback(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Feedback Records Analysis");

        let sql = r#"
            SELECT confirmed, COUNT(*) as count,
                   ROUND(AVG(medication_score)::numeric, 3) as avg_med_score,
                   ROUND(AVG(quantity_score)::numeric, 3) as avg_qty_score,
                   ROUND(AVG(total_score)::numeric, 3) as avg_total_score
            FROM feedback_records 
            GROUP BY confirmed
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No feedback records found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "Confirmed",
            "Count",
            "Avg Med Score",
            "Avg Qty Score",
            "Avg Total Score"
        ]);

        for row in &rows {
            let confirmed: bool = row.try_get_by_index(0)?;
            let count: i64 = row.try_get_by_index(1)?;
            let med: f64 = row.try_get_by_index(2).unwrap_or(0.0);
            let qty: f64 = row.try_get_by_index(3).unwrap_or(0.0);
            let total: f64 = row.try_get_by_index(4).unwrap_or(0.0);

            table.add_row(row![
                if confirmed { "YES".green() } else { "NO".red() },
                fmt_num(count),
                format!("{:.3}", med),
                format!("{:.3}", qty),
                format!("{:.3}", total)
            ]);
        }

        table.printstd();

        // Learning insights
        let insight_sql = r#"
            SELECT 
                ROUND(AVG(CASE WHEN confirmed THEN total_score END)::numeric, 3) as avg_confirmed,
                ROUND(AVG(CASE WHEN NOT confirmed THEN total_score END)::numeric, 3) as avg_rejected
            FROM feedback_records
        "#;

        if let Some(row) = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                insight_sql.to_string(),
            ))
            .await?
        {
            let avg_confirmed: f64 = row.try_get_by_index(0).unwrap_or(0.0);
            let avg_rejected: f64 = row.try_get_by_index(1).unwrap_or(0.0);

            println!("\n📊 Learning Insights:");
            println!(
                "  Avg score for confirmed matches: {}",
                format!("{:.3}", avg_confirmed).green()
            );
            println!(
                "  Avg score for rejected matches: {}",
                format!("{:.3}", avg_rejected).red()
            );
        }

        Ok(())
    }

    async fn analyze_weights(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Weight History (Last 5 Changes)");

        let sql = r#"
            SELECT id, source, 
                   ROUND(medication_weight::numeric, 3) as med_w,
                   ROUND(dosage_weight::numeric, 3) as dose_w,
                   ROUND(quantity_weight::numeric, 3) as qty_w,
                   ROUND(price_weight::numeric, 3) as price_w,
                   created_at
            FROM weight_history 
            ORDER BY created_at DESC 
            LIMIT 5
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No weight history found.");
            return Ok(());
        }

        let mut table = Table::new();
        table.add_row(row![
            "ID",
            "Source",
            "Med W",
            "Dose W",
            "Qty W",
            "Price W",
            "Applied At"
        ]);

        for row in &rows {
            let id: uuid::Uuid = row.try_get_by_index(0)?;
            let src: String = row.try_get_by_index(1)?;
            let med: f64 = row.try_get_by_index(2).unwrap_or(0.0);
            let dose: f64 = row.try_get_by_index(3).unwrap_or(0.0);
            let qty: f64 = row.try_get_by_index(4).unwrap_or(0.0);
            let price: f64 = row.try_get_by_index(5).unwrap_or(0.0);
            let date: chrono::DateTime<chrono::Utc> = row.try_get_by_index(6)?;

            table.add_row(row![
                truncate(&id.to_string(), 8),
                src,
                format!("{:.3}", med),
                format!("{:.3}", dose),
                format!("{:.3}", qty),
                format!("{:.3}", price),
                date.format("%Y-%m-%d %H:%M").to_string()
            ]);
        }

        table.printstd();

        Ok(())
    }

    async fn analyze_accuracy(&self, db: &DatabaseConnection) -> anyhow::Result<()> {
        print_subheader("Score Accuracy Analysis");

        // Analyze how well scores predict outcomes
        let sql = r#"
            SELECT 
                CASE 
                    WHEN score >= 0.9 THEN 'High (≥0.9)'
                    WHEN score >= 0.7 THEN 'Medium (0.7-0.9)'
                    ELSE 'Low (<0.7)'
                END as score_tier,
                COUNT(*) as total,
                SUM(CASE WHEN status = 'CONFIRMED' THEN 1 ELSE 0 END) as confirmed,
                SUM(CASE WHEN status = 'REJECTED' THEN 1 ELSE 0 END) as rejected
            FROM matches
            WHERE status IN ('CONFIRMED', 'REJECTED')
            GROUP BY 1
            ORDER BY 1
        "#;

        let rows = db
            .query_all(Statement::from_string(DbBackend::Postgres, sql.to_string()))
            .await?;

        if rows.is_empty() {
            println!("No decided matches for accuracy analysis.");
            return Ok(());
        }

        println!("Score tier accuracy (confirmed vs rejected):\n");

        for row in &rows {
            let tier: String = row.try_get_by_index(0)?;
            let total: i64 = row.try_get_by_index(1)?;
            let confirmed: i64 = row.try_get_by_index(2)?;
            let rejected: i64 = row.try_get_by_index(3)?;

            let accuracy = if total > 0 {
                (confirmed as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let bar_width = (accuracy / 5.0) as usize;
            let bar = "█".repeat(bar_width);

            let accuracy_colored = if accuracy >= 80.0 {
                format!("{:.1}%", accuracy).green()
            } else if accuracy >= 50.0 {
                format!("{:.1}%", accuracy).yellow()
            } else {
                format!("{:.1}%", accuracy).red()
            };

            println!(
                "  {:15} {} {} ({}✓ {}✗)",
                tier,
                bar.cyan(),
                accuracy_colored,
                confirmed,
                rejected
            );
        }

        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
