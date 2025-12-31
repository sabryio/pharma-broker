//! PharmaBroker Data Analysis CLI
//!
//! Rust port of Python analysis scripts for comprehensive data quality assessment.
//!
//! # Usage
//! ```bash
//! # Run all analysis phases
//! pharma-analysis all
//!
//! # Run specific phase
//! pharma-analysis health
//! pharma-analysis quality
//! pharma-analysis integrity
//! pharma-analysis business
//! pharma-analysis timeseries
//! pharma-analysis ai-quality
//! pharma-analysis matching
//! pharma-analysis stale
//!
//! # Expire stale matches
//! pharma-analysis expire --days 14
//! pharma-analysis expire --days 14 --execute
//! ```

use clap::{Parser, Subcommand};
use colored::*;
use dotenvy::dotenv;
use pharma_analysis::{
    AnalysisPhase, ai_quality::AiQualityAnalysis, business::BusinessLogicAnalysis,
    health::HealthAnalysis, integrity::IntegrityAnalysis, matching::MatchingAnalysis,
    quality::QualityAnalysis, stale::StaleMatchesAnalysis, timeseries::TimeSeriesAnalysis,
};
use pharma_db::{ConnectOptions, Database};
use std::env;

// ============================================================================
// Arabic RTL Text Handling for Terminal Display
// ============================================================================

/// Formats Arabic text for proper terminal display by reversing RTL segments.
fn format_arabic(text: &str) -> String {
    if !contains_arabic(text) {
        return text.to_string();
    }
    // For terminal display, reverse Arabic text
    reverse_string(text)
}

/// Checks if text contains any Arabic characters.
fn contains_arabic(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{0600}'..='\u{06FF}' |  // Arabic
            '\u{0750}'..='\u{077F}' |  // Arabic Supplement
            '\u{08A0}'..='\u{08FF}' |  // Arabic Extended-A
            '\u{FB50}'..='\u{FDFF}' |  // Arabic Presentation Forms-A
            '\u{FE70}'..='\u{FEFF}'    // Arabic Presentation Forms-B
        )
    })
}

/// Reverses a string (for RTL display).
fn reverse_string(text: &str) -> String {
    text.chars().rev().collect()
}

#[derive(Parser)]
#[command(
    name = "pharma-analysis",
    author = "Sabry Awad",
    version,
    about = "PharmaBroker Data Analysis Tool - Rust port of Python analysis scripts",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Database URL (overrides DATABASE_URL env var)
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Database health check & schema discovery (01_schema_discovery.py)
    Health,

    /// Data quality analysis - nulls, duplicates, statuses (02, 03)
    Quality,

    /// Referential integrity check (04_referential_integrity.py)
    Integrity,

    /// Business logic validation (05_business_logic.py)
    Business,

    /// Time series analysis (06_time_series.py)
    #[command(name = "timeseries")]
    TimeSeries,

    /// AI parsing quality assessment (07_ai_parsing_quality.py)
    #[command(name = "ai-quality")]
    AiQuality,

    /// Matching engine analysis (08_matching_analysis.py)
    Matching,

    /// Stale matches analysis (14_stale_matches.py)
    Stale,

    /// Expire old pending matches
    Expire {
        /// Number of days after which matches are considered stale
        #[arg(long, default_value = "14")]
        days: i64,

        /// Actually execute the expiration (default is dry-run)
        #[arg(long)]
        execute: bool,
    },

    /// Auto-confirm high-confidence matches and generate feedback records
    #[command(name = "auto-confirm")]
    AutoConfirm {
        /// Minimum score threshold for auto-confirmation (default: 0.9)
        #[arg(long, default_value = "0.9")]
        threshold: f64,

        /// Maximum number of matches to confirm (default: all)
        #[arg(long, short = 'n')]
        limit: Option<usize>,

        /// Actually execute the confirmations (default is dry-run)
        #[arg(long)]
        execute: bool,
    },

    /// Run ALL analysis phases
    All,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Get database URL
    let database_url = cli
        .database_url
        .or_else(|| env::var("DATABASE_URL").ok())
        .or_else(|| env::var("PB_DATABASE_DSN").ok())
        .unwrap_or_else(|| {
            "postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable".to_string()
        });

    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.sqlx_logging(false);

    // Print header
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║     📊 PharmaBroker Data Analysis (Rust Edition)         ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════╝".cyan()
    );
    println!(
        "📅 {} | 🔗 {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        truncate_url(&database_url)
    );
    println!();

    // Connect to database
    let db = Database::connect(opt).await?;
    println!("{}", "✅ Database connected".green());

    let command = cli.command.unwrap_or(Commands::All);

    match command {
        Commands::Health => {
            run_phase(&HealthAnalysis, &db).await?;
        }
        Commands::Quality => {
            run_phase(&QualityAnalysis, &db).await?;
        }
        Commands::Integrity => {
            run_phase(&IntegrityAnalysis, &db).await?;
        }
        Commands::Business => {
            run_phase(&BusinessLogicAnalysis, &db).await?;
        }
        Commands::TimeSeries => {
            run_phase(&TimeSeriesAnalysis, &db).await?;
        }
        Commands::AiQuality => {
            run_phase(&AiQualityAnalysis, &db).await?;
        }
        Commands::Matching => {
            run_phase(&MatchingAnalysis, &db).await?;
        }
        Commands::Stale => {
            run_phase(&StaleMatchesAnalysis, &db).await?;
        }
        Commands::Expire { days, execute } => {
            expire_matches(&db, days, execute).await?;
        }
        Commands::AutoConfirm {
            threshold,
            limit,
            execute,
        } => {
            auto_confirm_matches(&db, threshold, limit, execute).await?;
        }
        Commands::All => {
            run_all_phases(&db).await?;
        }
    }

    println!(
        "\n{}",
        "╔══════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║           ✨ Analysis completed successfully              ║".green()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════╝".green()
    );

    Ok(())
}

async fn run_phase(
    phase: &dyn AnalysisPhase,
    db: &pharma_db::DatabaseConnection,
) -> anyhow::Result<()> {
    println!("\n🚀 Running: {}", phase.name().bold().magenta());
    phase.run(db).await
}

async fn run_all_phases(db: &pharma_db::DatabaseConnection) -> anyhow::Result<()> {
    let phases: Vec<Box<dyn AnalysisPhase>> = vec![
        Box::new(HealthAnalysis),
        Box::new(QualityAnalysis),
        Box::new(IntegrityAnalysis),
        Box::new(BusinessLogicAnalysis),
        Box::new(TimeSeriesAnalysis),
        Box::new(AiQualityAnalysis),
        Box::new(MatchingAnalysis),
        Box::new(StaleMatchesAnalysis),
    ];

    println!(
        "\n📋 Running {} analysis phases...\n",
        phases.len().to_string().cyan()
    );

    for (i, phase) in phases.iter().enumerate() {
        println!(
            "\n{} Phase {}/{}: {}",
            "▶".cyan(),
            i + 1,
            phases.len(),
            phase.name().bold().magenta()
        );
        phase.run(db).await?;
    }

    Ok(())
}

async fn expire_matches(
    db: &pharma_db::DatabaseConnection,
    days: i64,
    execute: bool,
) -> anyhow::Result<()> {
    println!(
        "\n🗑️  {} matches older than {} days",
        if execute { "Expiring" } else { "Would expire" },
        days
    );

    let count = StaleMatchesAnalysis::expire_matches(db, days, !execute).await?;

    if execute {
        println!(
            "{}",
            format!("✅ Expired {} stale matches", count).green().bold()
        );
    } else {
        println!(
            "{}",
            format!("📊 {} matches would be expired (dry-run)", count).yellow()
        );
        println!("\n💡 Add --execute flag to actually expire these matches");
    }

    Ok(())
}

async fn auto_confirm_matches(
    db: &pharma_db::DatabaseConnection,
    threshold: f64,
    limit: Option<usize>,
    execute: bool,
) -> anyhow::Result<()> {
    use pharma_db::entity::feedback_record::Model as FeedbackRecord;
    use prettytable::{Table, row};
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let limit_str = limit
        .map(|n| format!(" (limit: {})", n))
        .unwrap_or_default();
    println!(
        "\n✅ {} high-confidence matches (score >= {:.2}){}",
        if execute {
            "Auto-confirming"
        } else {
            "Would auto-confirm"
        },
        threshold,
        limit_str
    );

    // Count matches that would be affected
    let count_sql = format!(
        "SELECT COUNT(*) FROM matches WHERE score >= {} AND status = 'PENDING'",
        threshold
    );
    let total_count: i64 = db
        .query_one(Statement::from_string(DbBackend::Postgres, count_sql))
        .await?
        .map(|r| r.try_get_by_index(0).unwrap_or(0))
        .unwrap_or(0);

    let effective_count = match limit {
        Some(n) => (total_count as usize).min(n),
        None => total_count as usize,
    };

    println!(
        "📊 Found {} pending matches with score >= {:.2}{}",
        total_count,
        threshold,
        if limit.is_some() {
            format!(" (will process {})", effective_count)
        } else {
            String::new()
        }
    );

    if total_count == 0 {
        println!("{}", "No matches to confirm.".yellow());
        return Ok(());
    }

    // Show preview of first 5 matches
    let preview_sql = format!(
        "SELECT m.id, m.score, 
                o.medication as offer_med, o.medication_raw as offer_raw, o.source_phone as offer_phone, o.source_name as offer_name, og.name as offer_group,
                r.medication as request_med, r.medication_raw as request_raw, r.source_phone as request_phone, r.source_name as request_name, rg.name as request_group
         FROM matches m
         LEFT JOIN offers o ON m.offer_id = o.id
         LEFT JOIN requests r ON m.request_id = r.id
         LEFT JOIN groups og ON o.source_group = og.jid
         LEFT JOIN groups rg ON r.source_group = rg.jid
         WHERE m.score >= {} AND m.status = 'PENDING'
         ORDER BY m.score DESC
         LIMIT 5",
        threshold
    );
    let preview_rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, preview_sql))
        .await?;

    println!("\n📋 Preview of top matches to confirm:");
    let mut table = Table::new();
    table.add_row(row![
        "ID",
        "Score",
        "Offer (Med | Sender | Group)",
        "Offer Phone",
        "Request (Med | Sender | Group)",
        "Request Phone"
    ]);

    for row in &preview_rows {
        let id: uuid::Uuid = row.try_get_by_index(0)?;
        let score: f64 = row.try_get_by_index(1)?;
        let offer_med: Option<String> = row.try_get_by_index(2).ok();
        let offer_raw: Option<String> = row.try_get_by_index(3).ok();
        let offer_phone: Option<String> = row.try_get_by_index(4).ok();
        let offer_name: Option<String> = row.try_get_by_index(5).ok();
        let offer_group: Option<String> = row.try_get_by_index(6).ok();
        let request_med: Option<String> = row.try_get_by_index(7).ok();
        let request_raw: Option<String> = row.try_get_by_index(8).ok();
        let request_phone: Option<String> = row.try_get_by_index(9).ok();
        let request_name: Option<String> = row.try_get_by_index(10).ok();
        let request_group: Option<String> = row.try_get_by_index(11).ok();

        // Use name if available, otherwise raw Arabic text
        let o_sender = offer_name
            .map(|s| format_arabic(&s))
            .unwrap_or_else(|| format_arabic(&offer_raw.unwrap_or_else(|| "-".to_string())));
        let r_sender = request_name
            .map(|s| format_arabic(&s))
            .unwrap_or_else(|| format_arabic(&request_raw.unwrap_or_else(|| "-".to_string())));

        // Apply Arabic formatting to med and group names as well
        let o_med = format_arabic(&offer_med.unwrap_or_else(|| "-".to_string()));
        let o_group = format_arabic(&offer_group.unwrap_or_else(|| "Unknown Group".to_string()));
        let r_med = format_arabic(&request_med.unwrap_or_else(|| "-".to_string()));
        let r_group = format_arabic(&request_group.unwrap_or_else(|| "Unknown Group".to_string()));

        table.add_row(row![
            &id.to_string()[..8],
            format!("{:.4}", score).green(),
            format!("{} | {} | {}", o_med, o_sender, o_group),
            offer_phone.unwrap_or_else(|| "-".to_string()),
            format!("{} | {} | {}", r_med, r_sender, r_group),
            request_phone.unwrap_or_else(|| "-".to_string())
        ]);
    }
    table.printstd();

    if !execute {
        println!(
            "\n{}",
            format!(
                "📊 {} matches would be auto-confirmed (dry-run)",
                effective_count
            )
            .yellow()
        );
        println!(
            "\n💡 Add --execute flag to actually confirm these matches and create feedback records"
        );
        return Ok(());
    }

    // Get match IDs and scores to process (apply limit if specified)
    let limit_clause = limit.map(|n| format!(" LIMIT {}", n)).unwrap_or_default();
    let select_sql = format!(
        "SELECT id, score FROM matches WHERE score >= {} AND status = 'PENDING' ORDER BY score DESC{}",
        threshold, limit_clause
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, select_sql))
        .await?;

    let mut confirmed_count = 0;
    let mut feedback_count = 0;

    for row in &rows {
        let match_id: uuid::Uuid = row.try_get_by_index(0)?;
        let score: f64 = row.try_get_by_index(1)?;

        // 1. Update match status to CONFIRMED
        let update_sql = format!(
            "UPDATE matches SET status = 'CONFIRMED', confirmed_at = NOW(), matched_by = 'auto-confirm' WHERE id = '{}'",
            match_id
        );
        if let Err(e) = db
            .execute(Statement::from_string(DbBackend::Postgres, update_sql))
            .await
        {
            eprintln!("  ⚠️ Failed to confirm match {}: {}", match_id, e);
            continue;
        }
        confirmed_count += 1;

        // 2. Create feedback record using the entity model (validates entity structure)
        let feedback = FeedbackRecord::confirmed(match_id, "auto-confirm", score);

        // Insert using the entity's ActiveModel
        use pharma_db::entity::feedback_record::ActiveModel;
        use sea_orm::ActiveModelTrait;
        use sea_orm::Set;

        let active = ActiveModel {
            id: Set(feedback.id),
            match_id: Set(feedback.match_id),
            user_id: Set(feedback.user_id.clone()),
            confirmed: Set(feedback.confirmed),
            medication_score: Set(feedback.medication_score),
            dosage_score: Set(feedback.dosage_score),
            quantity_score: Set(feedback.quantity_score),
            price_score: Set(feedback.price_score),
            recency_score: Set(feedback.recency_score),
            total_score: Set(feedback.total_score),
            created_at: Set(feedback.created_at),
        };

        if let Err(e) = active.insert(db).await {
            eprintln!(
                "  ⚠️ Failed to create feedback for match {}: {}",
                match_id, e
            );
        } else {
            feedback_count += 1;
        }
    }

    println!(
        "{}",
        format!(
            "✅ Confirmed {} matches, created {} feedback records",
            confirmed_count, feedback_count
        )
        .green()
        .bold()
    );

    Ok(())
}

fn truncate_url(url: &str) -> String {
    // Hide password in URL for display
    if let Some(at_pos) = url.find('@')
        && let Some(colon_pos) = url[..at_pos].rfind(':')
    {
        let prefix = &url[..colon_pos + 1];
        let suffix = &url[at_pos..];
        return format!("{}****{}", prefix, suffix);
    }
    url.to_string()
}
