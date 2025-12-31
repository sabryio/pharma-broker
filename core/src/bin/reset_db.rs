//! # Database Reset Tool
//!
//! Reset the database (DANGER: Deletes all data).
//! Ported from legacy/cmd/reset.go
//!
//! ## Usage
//! ```bash
//! cargo run --bin reset-db
//! cargo run --bin reset-db -- --force  # Skip confirmation
//! ```

use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::Path;

use colored::Colorize;
use pharma_core::domain::MedicationMapping;
use pharma_core::repository::{
    MedicationMappingRepository, SeaOrmMedicationMappingRepo, create_connection,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::Deserialize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Configuration
// ============================================================================

const MEDICATIONS_JSON: &str = "medications.json";

/// Tables to truncate in order (respecting FK constraints)
const TABLES: &[&str] = &[
    "feedback_records",
    "weight_history",
    "review_queue",
    "unmapped_medications",
    "audit_logs",
    "demand_leaderboard",
    "match_feedback",
    "failed_messages",
    "match_queue",
    "matches",
    "offers",
    "requests",
    "raw_messages",
    "config",
    "bot_users",
];

// ============================================================================
// JSON Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct MedicationEntry {
    english: String,
    #[serde(default)]
    synonyms: Vec<String>,
}

// ============================================================================
// Helpers
// ============================================================================

/// Mask password in DSN for display
fn mask_dsn(dsn: &str) -> String {
    // postgres://user:password@host:port/db -> postgres://user:***@host:port/db
    if let Some(proto_end) = dsn.find("://") {
        let after_proto = &dsn[proto_end + 3..];
        if let Some(at_pos) = after_proto.find('@') {
            let user_pass = &after_proto[..at_pos];
            if let Some(colon_pos) = user_pass.find(':') {
                let user = &user_pass[..colon_pos];
                let rest = &after_proto[at_pos..];
                return format!("{}://{}:***{}", &dsn[..proto_end], user, rest);
            }
        }
    }
    dsn.to_string()
}

/// Load medications from JSON file
fn load_medications_json() -> anyhow::Result<HashMap<String, MedicationEntry>> {
    let paths = [
        MEDICATIONS_JSON,
        "core/medications.json",
        "../medications.json",
    ];

    for path in &paths {
        if Path::new(path).exists() {
            let content = std::fs::read_to_string(path)?;
            let medications: HashMap<String, MedicationEntry> = serde_json::from_str(&content)?;
            println!("  Reading from: {}", path.cyan());
            return Ok(medications);
        }
    }

    anyhow::bail!("medications.json not found. Tried: {:?}", paths)
}

/// Truncate all tables
async fn truncate_tables(db: &DatabaseConnection) -> anyhow::Result<()> {
    for table in TABLES {
        let sql = format!("TRUNCATE TABLE {} CASCADE", table);
        match db
            .execute(Statement::from_string(db.get_database_backend(), sql))
            .await
        {
            Ok(_) => {
                println!("  {} Truncated {}", "✓".green(), table);
            }
            Err(e) => {
                println!(
                    "  {} Failed to truncate {} (may not exist): {}",
                    "⚠".yellow(),
                    table,
                    e
                );
            }
        }
    }
    Ok(())
}

/// Seed medication mappings
async fn seed_medications(repo: &SeaOrmMedicationMappingRepo) -> anyhow::Result<(usize, usize)> {
    let medications = load_medications_json()?;

    println!(
        "  Found {} medications in JSON",
        medications.len().to_string().yellow()
    );

    let mut count = 0usize;
    let mut synonym_count = 0usize;

    for (arabic_name, entry) in medications {
        let mut mapping = MedicationMapping::new(&arabic_name, &entry.english);
        if !entry.synonyms.is_empty() {
            synonym_count += entry.synonyms.len();
            mapping.synonyms = Some(entry.synonyms);
        }

        if let Err(e) = repo.save(&mapping).await {
            println!("    {} Failed to seed {}: {}", "⚠".yellow(), arabic_name, e);
        } else {
            count += 1;
        }
    }

    Ok((count, synonym_count))
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check for --force flag
    let args: Vec<String> = env::args().collect();
    let force = args.iter().any(|a| a == "--force" || a == "-f");

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".red()
    );
    println!(
        "{}",
        "║              ⚠️  DATABASE RESET TOOL                                          ║"
            .red()
            .bold()
    );
    println!(
        "{}",
        "║              DANGER: This will DELETE ALL DATA!                              ║".red()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".red()
    );
    println!();

    // Load environment
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    // Safety check
    if !force {
        println!(
            "{}",
            "⚠️  DANGER: You are about to TRUNCATE ALL TABLES in database:".yellow()
        );
        println!("   {}", mask_dsn(&database_url).cyan());
        println!();
        print!("Are you sure you want to continue? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "y" {
            println!("{}", "Operation cancelled.".green());
            return Ok(());
        }
    }

    println!();
    println!("🗄️  Connecting to database...");
    let db = create_connection(&database_url).await?;

    // Truncate tables
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{}", "🗑️  Truncating all tables...".yellow().bold());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    truncate_tables(&db).await?;

    println!();
    println!("{}", "✓ Tables truncated successfully".green());

    // Seed medications
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{}", "🌱 Seeding medication mappings...".yellow().bold());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    let repo = SeaOrmMedicationMappingRepo::new(db);
    let (count, synonym_count) = seed_medications(&repo).await?;

    println!();
    println!(
        "{} Seeded {} medications with {} synonyms",
        "✓".green(),
        count.to_string().green(),
        synonym_count.to_string().cyan()
    );

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              ✅ DATABASE RESET COMPLETE                                      ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".green()
    );

    Ok(())
}
