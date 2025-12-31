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
use pharma_core::grpc::pharma::{ConnectMatchRequest, pharma_bridge_client::PharmaBridgeClient};
use pharma_db::{ConnectOptions, Database};
use prettytable::format;
use sea_orm::ConnectionTrait;
use std::env;
use unicode_width::UnicodeWidthStr;

// Column width constants for aligned table output
const COL_WIDTH_MED: usize = 35;
const COL_WIDTH_INFO: usize = 40;

/// Pad a string to a fixed display width using Unicode-aware width calculation
fn pad_to_width(text: &str, width: usize) -> String {
    let current_width = UnicodeWidthStr::width(text);
    if current_width >= width {
        text.to_string()
    } else {
        format!("{}{}", text, " ".repeat(width - current_width))
    }
}

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

/// Formats a phone number for display, hiding LIDs.
fn format_phone(phone: &str) -> String {
    if phone.is_empty() {
        return "-".to_string();
    }
    // LIDs are usually long numeric strings (14+ digits) or end in @lid
    if phone.len() > 13 || phone.contains("@lid") {
        return "Hidden (LID)".bright_black().to_string();
    }
    // Format regular phone numbers with a '+' if missing
    if phone.starts_with('2') || phone.starts_with('1') {
        format!("+{}", phone)
    } else {
        phone.to_string()
    }
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

    /// Medication curation workflow - validate and link parsed medications
    #[command(name = "curate")]
    Curate {
        #[command(subcommand)]
        action: CurateAction,
    },

    /// Run ALL analysis phases
    All,
}

/// Curation subcommands
#[derive(Subcommand)]
enum CurateAction {
    /// List medications pending curation
    List {
        /// Maximum number of items to display
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Show only uncurated medications
        #[arg(long)]
        pending: bool,
    },

    /// Show curation statistics
    Stats,

    /// Create a new master medication
    #[command(name = "create-master")]
    CreateMaster {
        /// Canonical English name
        #[arg(long)]
        name: String,

        /// Arabic name (optional)
        #[arg(long)]
        name_ar: Option<String>,

        /// Strength (e.g., "12.5mg")
        #[arg(long)]
        strength: Option<String>,

        /// Active ingredient
        #[arg(long)]
        ingredient: Option<String>,

        /// Manufacturer
        #[arg(long)]
        manufacturer: Option<String>,
    },

    /// Approve an alias and link it to a master medication
    Approve {
        /// The alias name to approve
        #[arg(long)]
        alias: String,

        /// ID of the master medication to link to
        #[arg(long)]
        master_id: String,

        /// Operator name for audit
        #[arg(long, default_value = "cli")]
        operator: String,
    },

    /// Sync uncurated medications from offers/requests to medication_aliases
    Sync,

    /// Show AI suggestions for a medication alias
    Suggest {
        /// The alias name to find suggestions for
        #[arg(long)]
        alias: String,
    },

    /// Automatically resolve high-confidence matches
    Resolve {
        /// Confidence threshold (0.0 - 1.0)
        #[arg(long, default_value = "0.95")]
        threshold: f64,

        /// Actually execute changes (default is dry-run)
        #[arg(long)]
        execute: bool,

        /// Limit number of items to process
        #[arg(long, default_value = "50")]
        limit: usize,
    },
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
        Commands::Curate { action } => {
            handle_curate_action(&db, action).await?;
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
        "║           ✨ Analysis completed successfully             ║".green()
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
                o.medication as offer_med, op.push_name as offer_name, op.phone as offer_phone, og.name as offer_group, op.jid as offer_jid, om.english_name as offer_eng,
                r.medication as request_med, rp.push_name as request_name, rp.phone as request_phone, rg.name as request_group, rp.jid as request_jid, rm.english_name as request_eng
         FROM matches m
         LEFT JOIN offers o ON m.offer_id = o.id
         LEFT JOIN requests r ON m.request_id = r.id
         LEFT JOIN participants op ON o.participant_id = op.id
         LEFT JOIN groups og ON o.group_id = og.id
         LEFT JOIN participants rp ON r.participant_id = rp.id
         LEFT JOIN groups rg ON r.group_id = rg.id
         LEFT JOIN medication_mappings om ON o.medication = om.arabic_name
         LEFT JOIN medication_mappings rm ON r.medication = rm.arabic_name
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
    table.set_format(*format::consts::FORMAT_CLEAN);
    table.add_row(row![
        "ID".bold(),
        "Score".bold(),
        "Medication".bold(),
        "Seller Information".bold(),
        "Buyer Information".bold()
    ]);

    for row in &preview_rows {
        let id: uuid::Uuid = row.try_get_by_index(0)?;
        let score: f64 = row.try_get_by_index(1)?;
        let offer_med: Option<String> = row.try_get_by_index(2).ok();
        let offer_name: Option<String> = row.try_get_by_index(3).ok();
        let offer_phone: Option<String> = row.try_get_by_index(4).ok();
        let offer_group: Option<String> = row.try_get_by_index(5).ok();
        let offer_jid: String = row.try_get_by_index(6).unwrap_or_default();
        let offer_eng: Option<String> = row.try_get_by_index(7).ok();

        let request_med: Option<String> = row.try_get_by_index(8).ok();
        let request_name: Option<String> = row.try_get_by_index(9).ok();
        let request_phone: Option<String> = row.try_get_by_index(10).ok();
        let request_group: Option<String> = row.try_get_by_index(11).ok();
        let request_jid: String = row.try_get_by_index(12).unwrap_or_default();
        let request_eng: Option<String> = row.try_get_by_index(13).ok();

        // Use name if available, otherwise raw identifier (cleaned)
        let o_name = offer_name
            .map(|s| format_arabic(&s))
            .unwrap_or_else(|| "Unknown".to_string());
        let r_name = request_name
            .map(|s| format_arabic(&s))
            .unwrap_or_else(|| "Unknown".to_string());

        let o_phone = format_phone(&offer_phone.unwrap_or_default());
        let r_phone = format_phone(&request_phone.unwrap_or_default());

        let o_med_ar = format_arabic(&offer_med.unwrap_or_else(|| "-".to_string()));
        let o_med_en = offer_eng.unwrap_or_else(|| "-".to_string());
        let r_med_ar = format_arabic(&request_med.unwrap_or_else(|| "-".to_string()));
        let r_med_en = request_eng.unwrap_or_else(|| "-".to_string());

        let o_group_str = offer_group
            .map(|s| format!("🏠 {}", format_arabic(&s)))
            .unwrap_or_else(|| "🏠 Direct Chat".to_string());
        let r_group_str = request_group
            .map(|s| format!("🏠 {}", format_arabic(&s)))
            .unwrap_or_else(|| "🏠 Direct Chat".to_string());

        // Build padded cell content for proper alignment
        let med_line1 = pad_to_width(
            &format!("OFFER: {} | {}", o_med_ar, o_med_en),
            COL_WIDTH_MED,
        );
        let med_line2 = pad_to_width(
            &format!("REQ:   {} | {}", r_med_ar, r_med_en),
            COL_WIDTH_MED,
        );

        let seller_line1 = pad_to_width(&o_name, COL_WIDTH_INFO);
        let seller_line2 = pad_to_width(
            &format!("📞 {} | 🆔 {}", o_phone, offer_jid),
            COL_WIDTH_INFO,
        );
        let seller_line3 = pad_to_width(&o_group_str, COL_WIDTH_INFO);

        let buyer_line1 = pad_to_width(&r_name, COL_WIDTH_INFO);
        let buyer_line2 = pad_to_width(
            &format!("📞 {} | 🆔 {}", r_phone, request_jid),
            COL_WIDTH_INFO,
        );
        let buyer_line3 = pad_to_width(&r_group_str, COL_WIDTH_INFO);

        table.add_row(row![
            pad_to_width(&id.to_string()[..8], 8),
            pad_to_width(&format!("{:.4}", score), 6),
            format!("{}\n{}", med_line1, med_line2),
            format!("{}\n{}\n{}", seller_line1, seller_line2, seller_line3),
            format!("{}\n{}\n{}", buyer_line1, buyer_line2, buyer_line3)
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
        "SELECT m.id, m.score, \
                op.phone as offer_phone, op.push_name as offer_name, o.medication as offer_med, \
                rp.phone as request_phone, rp.push_name as request_name, r.medication as request_med, \
                op.jid as offerer_jid, rp.jid as requester_jid \
         FROM matches m \
         LEFT JOIN offers o ON m.offer_id = o.id \
         LEFT JOIN requests r ON m.request_id = r.id \
         LEFT JOIN participants op ON o.participant_id = op.id \
         LEFT JOIN participants rp ON r.participant_id = rp.id \
         WHERE m.score >= {} AND m.status = 'PENDING' \
         ORDER BY m.score DESC{}",
        threshold, limit_clause
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, select_sql))
        .await?;

    let mut confirmed_count = 0;
    let mut feedback_count = 0;
    let mut notification_count = 0;

    // Connect to Bridge gRPC server
    let bridge_url =
        std::env::var("BRIDGE_GRPC_URL").unwrap_or_else(|_| "http://bridge:50052".into());
    let mut bridge_client = match PharmaBridgeClient::connect(bridge_url.clone()).await {
        Ok(client) => Some(client),
        Err(e) => {
            eprintln!("  ⚠️ Could not connect to bridge at {}: {}", bridge_url, e);
            None
        }
    };

    for row in rows {
        let match_id: uuid::Uuid = row.try_get_by_index(0)?;
        let score: f64 = row.try_get_by_index(1)?;

        let offer_phone: String = row.try_get_by_index(2).unwrap_or_default();
        let offer_name: String = row.try_get_by_index(3).unwrap_or_default();
        let offer_med: String = row.try_get_by_index(4).unwrap_or_default();
        let request_phone: String = row.try_get_by_index(5).unwrap_or_default();
        let request_name: String = row.try_get_by_index(6).unwrap_or_default();
        let _request_med: String = row.try_get_by_index(7).unwrap_or_default();
        let offerer_jid: String = row.try_get_by_index(8).unwrap_or_default();
        let requester_jid: String = row.try_get_by_index(9).unwrap_or_default();

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

        // 2. Create feedback record
        let feedback = FeedbackRecord::confirmed(match_id, "auto-confirm", score);

        use pharma_db::entity::feedback_record::ActiveModel;
        use sea_orm::{ActiveModelTrait, Set};

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

        // 3. Notify bridge to connect parties
        if let Some(ref mut client) = bridge_client {
            let req = tonic::Request::new(ConnectMatchRequest {
                match_id: match_id.to_string(),
                offerer_jid,
                offerer_phone: offer_phone,
                offerer_name: offer_name,
                requester_jid,
                requester_phone: request_phone,
                requester_name: request_name,
                medication: offer_med,
            });

            match client.connect_match(req).await {
                Ok(_) => notification_count += 1,
                Err(e) => eprintln!("  ⚠️ Failed to notify bridge for match {}: {}", match_id, e),
            }
        }
    }

    println!(
        "\n{}",
        format!(
            "✅ Confirmed {} matches, created {} feedback records, notified bridge for {} matches",
            confirmed_count, feedback_count, notification_count
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

// ============================================================================
// Medication Curation Commands
// ============================================================================

async fn handle_curate_action(
    db: &pharma_db::DatabaseConnection,
    action: CurateAction,
) -> anyhow::Result<()> {
    use prettytable::{Table, row};
    use sea_orm::{DbBackend, Statement};

    match action {
        CurateAction::List { limit, pending } => {
            println!(
                "\n📋 Medications {}",
                if pending {
                    "(pending curation only)"
                } else {
                    ""
                }
            );

            // Query distinct medications from offers that don't have master_medication_id
            let sql = format!(
                "SELECT 
                    medication,
                    COUNT(*) as count,
                    MIN(created_at) as first_seen,
                    MAX(created_at) as last_seen
                 FROM offers 
                 {} 
                 GROUP BY medication
                 ORDER BY count DESC
                 LIMIT {}",
                if pending {
                    "WHERE master_medication_id IS NULL"
                } else {
                    ""
                },
                limit
            );

            let rows = db
                .query_all(Statement::from_string(DbBackend::Postgres, sql))
                .await?;

            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_CLEAN);
            table.add_row(row![
                "Medication".bold(),
                "Count".bold(),
                "First Seen".bold(),
                "Last Seen".bold()
            ]);

            for row in &rows {
                let medication: String = row.try_get_by_index(0)?;
                let count: i64 = row.try_get_by_index(1)?;
                let first_seen: chrono::DateTime<chrono::Utc> = row.try_get_by_index(2)?;
                let last_seen: chrono::DateTime<chrono::Utc> = row.try_get_by_index(3)?;

                table.add_row(row![
                    format_arabic(&medication),
                    count.to_string().green(),
                    first_seen.format("%Y-%m-%d").to_string(),
                    last_seen.format("%Y-%m-%d").to_string()
                ]);
            }

            table.printstd();
            println!(
                "\n💡 Tip: Use `curate create-master --name \"...\"` to create a master medication"
            );
        }

        CurateAction::Stats => {
            println!("\n📊 Curation Statistics");

            // Count total offers
            let total_sql = "SELECT COUNT(*) FROM offers";
            let total: i64 = db
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    total_sql.to_string(),
                ))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            // Count curated offers
            let curated_sql = "SELECT COUNT(*) FROM offers WHERE master_medication_id IS NOT NULL";
            let curated: i64 = db
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    curated_sql.to_string(),
                ))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            // Count master medications
            let master_sql = "SELECT COUNT(*) FROM medication_master";
            let masters: i64 = db
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    master_sql.to_string(),
                ))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            // Count aliases
            let alias_sql = "SELECT COUNT(*) FROM medication_aliases";
            let aliases: i64 = db
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    alias_sql.to_string(),
                ))
                .await?
                .map(|r| r.try_get_by_index(0).unwrap_or(0))
                .unwrap_or(0);

            println!("  📦 Total offers:        {}", total);
            println!(
                "  ✅ Curated offers:      {} ({:.1}%)",
                curated,
                if total > 0 {
                    curated as f64 / total as f64 * 100.0
                } else {
                    0.0
                }
            );
            println!("  🗂️  Master medications: {}", masters);
            println!("  🔗 Aliases defined:     {}", aliases);
        }

        CurateAction::CreateMaster {
            name,
            name_ar,
            strength,
            ingredient,
            manufacturer,
        } => {
            use pharma_db::entity::medication_master;
            use sea_orm::{ActiveModelTrait, Set};

            let model = medication_master::ActiveModel {
                id: Set(uuid::Uuid::new_v4()),
                canonical_name: Set(name.clone()),
                canonical_name_ar: Set(name_ar),
                strength: Set(strength),
                active_ingredient: Set(ingredient),
                manufacturer: Set(manufacturer),
                dosage_form: Set(None),
                eda_registration: Set(None),
                therapeutic_class: Set(None),
                atc_code: Set(None),
                status: Set(medication_master::MedicationStatus::Active),
                created_at: Set(chrono::Utc::now()),
                updated_at: Set(chrono::Utc::now()),
                created_by: Set(Some("cli".to_string())),
            };

            let result = model.insert(db).await?;
            println!("✅ Created master medication: {} (ID: {})", name, result.id);
        }

        CurateAction::Approve {
            alias,
            master_id,
            operator,
        } => {
            use pharma_db::entity::medication_alias;
            use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

            let master_uuid = uuid::Uuid::parse_str(&master_id)?;

            // Find existing alias or create new one
            let normalized = medication_alias::Model::normalize(&alias);
            let existing = medication_alias::Entity::find()
                .filter(medication_alias::Column::AliasNameNormalized.eq(&normalized))
                .one(db)
                .await?;

            if let Some(existing) = existing {
                // Update existing
                let mut model: medication_alias::ActiveModel = existing.into();
                model.master_medication_id = Set(Some(master_uuid));
                model.curation_status = Set(medication_alias::CurationStatus::Approved);
                model.curated_by = Set(Some(operator.clone()));
                model.curated_at = Set(Some(chrono::Utc::now()));
                model.update(db).await?;
                println!("✅ Updated alias: \"{}\" → master {}", alias, master_id);
            } else {
                // Create new
                let model = medication_alias::ActiveModel {
                    id: Set(uuid::Uuid::new_v4()),
                    alias_name: Set(alias.clone()),
                    alias_name_normalized: Set(normalized.clone()),
                    master_medication_id: Set(Some(master_uuid)),
                    ai_suggestion_confidence: Set(None),
                    curation_status: Set(medication_alias::CurationStatus::Approved),
                    curated_by: Set(Some(operator)),
                    curated_at: Set(Some(chrono::Utc::now())),
                    occurrence_count: Set(1),
                    first_seen_at: Set(chrono::Utc::now()),
                    last_seen_at: Set(chrono::Utc::now()),
                };
                model.insert(db).await?;
                println!(
                    "✅ Created and approved alias: \"{}\" → master {}",
                    alias, master_id
                );
            }

            // Backfill offers with this medication
            let backfill_sql = format!(
                "UPDATE offers SET master_medication_id = '{}', medication_curated = true WHERE LOWER(TRIM(medication)) = '{}'",
                master_uuid,
                normalized.replace("'", "''")
            );
            let result = db
                .execute(Statement::from_string(DbBackend::Postgres, backfill_sql))
                .await?;
            println!("   Backfilled {} offers", result.rows_affected());

            // Backfill requests
            let backfill_sql = format!(
                "UPDATE requests SET master_medication_id = '{}', medication_curated = true WHERE LOWER(TRIM(medication)) = '{}'",
                master_uuid,
                normalized.replace("'", "''")
            );
            let result = db
                .execute(Statement::from_string(DbBackend::Postgres, backfill_sql))
                .await?;
            println!("   Backfilled {} requests", result.rows_affected());
        }

        CurateAction::Sync => {
            println!("\n🔄 Syncing uncurated medications to aliases...");

            let sync_sql = "
                INSERT INTO medication_aliases (
                    alias_name, 
                    alias_name_normalized, 
                    occurrence_count, 
                    first_seen_at, 
                    last_seen_at
                )
                SELECT 
                    medication,
                    LOWER(TRIM(medication)),
                    COUNT(*),
                    MIN(created_at),
                    MAX(created_at)
                FROM offers
                WHERE master_medication_id IS NULL
                GROUP BY medication
                ON CONFLICT (alias_name_normalized) DO UPDATE SET
                    occurrence_count = EXCLUDED.occurrence_count,
                    last_seen_at = EXCLUDED.last_seen_at
            ";

            let result = db
                .execute(Statement::from_string(
                    DbBackend::Postgres,
                    sync_sql.to_string(),
                ))
                .await?;

            println!("✅ Synced {} medication aliases", result.rows_affected());
        }

        CurateAction::Suggest { alias } => {
            println!("\n🔍 Suggestions for: \"{}\"", alias);

            let normalized = alias.to_lowercase().trim().to_string();

            // Use trigram similarity to find candidates
            let suggest_sql = format!(
                "SELECT 
                    id, 
                    canonical_name, 
                    strength,
                    similarity(canonical_name, '{}') as score
                 FROM medication_master
                 WHERE canonical_name % '{}' OR canonical_name ILIKE '%{}%'
                 ORDER BY score DESC
                 LIMIT 5",
                normalized.replace("'", "''"),
                normalized.replace("'", "''"),
                normalized.replace("'", "''")
            );

            let rows = db
                .query_all(Statement::from_string(DbBackend::Postgres, suggest_sql))
                .await?;

            if rows.is_empty() {
                println!("❌ No suggestions found.");
            } else {
                let mut table = Table::new();
                table.set_format(*format::consts::FORMAT_CLEAN);
                table.add_row(row![
                    "ID".bold(),
                    "Master Medication".bold(),
                    "Strength".bold(),
                    "Confidence".bold()
                ]);

                for row in &rows {
                    let id: uuid::Uuid = row.try_get_by_index(0)?;
                    let name: String = row.try_get_by_index(1)?;
                    let strength: Option<String> = row.try_get_by_index(2)?;
                    let score: f32 = row.try_get_by_index(3)?;

                    table.add_row(row![
                        id.to_string().dimmed(),
                        name.green(),
                        strength.unwrap_or_default().yellow(),
                        format!("{:.1}%", score * 100.0).bold()
                    ]);
                }
                table.printstd();
            }
        }

        CurateAction::Resolve {
            threshold,
            execute,
            limit,
        } => {
            println!(
                "\n🤖 Auto-resolving medications (threshold: {:.1}%, execution: {})",
                threshold * 100.0,
                execute
            );

            // Fetch top uncurated aliases
            let alias_sql = format!(
                "SELECT alias_name, alias_name_normalized, occurrence_count 
                 FROM medication_aliases 
                 WHERE curation_status = 'PENDING' 
                 ORDER BY occurrence_count DESC 
                 LIMIT {}",
                limit
            );

            let aliases = db
                .query_all(Statement::from_string(DbBackend::Postgres, alias_sql))
                .await?;
            let mut resolved_count = 0;

            for alias_row in aliases {
                let alias_name: String = alias_row.try_get_by_index(0)?;
                let normalized: String = alias_row.try_get_by_index(1)?;
                let count: i32 = alias_row.try_get_by_index(2)?;

                // Find best master match
                let suggest_sql = format!(
                    "SELECT id, canonical_name, similarity(canonical_name, '{}') as score
                     FROM medication_master
                     ORDER BY score DESC
                     LIMIT 1",
                    normalized.replace("'", "''")
                );

                if let Some(suggest_row) = db
                    .query_one(Statement::from_string(DbBackend::Postgres, suggest_sql))
                    .await?
                {
                    let master_id: uuid::Uuid = suggest_row.try_get_by_index(0)?;
                    let master_name: String = suggest_row.try_get_by_index(1)?;
                    let score: f32 = suggest_row.try_get_by_index(2)?;

                    if score as f64 >= threshold {
                        println!(
                            "✨ Match: \"{}\" ({}) → \"{}\" ({:.1}%)",
                            alias_name,
                            count,
                            master_name,
                            score * 100.0
                        );

                        if execute {
                            // Link alias
                            let update_alias = format!(
                                "UPDATE medication_aliases SET 
                                    master_medication_id = '{}', 
                                    curation_status = 'APPROVED',
                                    curated_by = 'auto-resolve',
                                    curated_at = NOW()
                                 WHERE alias_name_normalized = '{}'",
                                master_id,
                                normalized.replace("'", "''")
                            );
                            db.execute(Statement::from_string(DbBackend::Postgres, update_alias))
                                .await?;

                            // Backfill offers
                            let backfill_offers = format!(
                                "UPDATE offers SET master_medication_id = '{}', medication_curated = true WHERE LOWER(TRIM(medication)) = '{}'",
                                master_id,
                                normalized.replace("'", "''")
                            );
                            db.execute(Statement::from_string(
                                DbBackend::Postgres,
                                backfill_offers,
                            ))
                            .await?;

                            // Backfill requests
                            let backfill_requests = format!(
                                "UPDATE requests SET master_medication_id = '{}', medication_curated = true WHERE LOWER(TRIM(medication)) = '{}'",
                                master_id,
                                normalized.replace("'", "''")
                            );
                            db.execute(Statement::from_string(
                                DbBackend::Postgres,
                                backfill_requests,
                            ))
                            .await?;

                            resolved_count += 1;
                        }
                    }
                }
            }

            if execute {
                println!("\n✅ Auto-resolved {} medication aliases", resolved_count);
            } else {
                println!("\n💡 This was a dry-run. Use `--execute` to apply changes.");
            }
        }
    }

    Ok(())
}
