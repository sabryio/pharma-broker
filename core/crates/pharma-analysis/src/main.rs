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
use pharma_db::Database;
use std::env;

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
    let db = Database::connect(&database_url).await?;
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
