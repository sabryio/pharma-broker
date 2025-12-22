//! # Database Health Monitor
//!
//! CLI tool for monitoring database health and performance.
//! Uses DbDiagnostics to analyze query performance and detect issues.
//!
//! ## Usage
//! ```bash
//! cargo run --bin db-health                    # Full health report
//! cargo run --bin db-health -- --json          # JSON output
//! cargo run --bin db-health -- --watch         # Continuous monitoring
//! cargo run --bin db-health -- --analyze       # Analyze critical queries
//! cargo run --bin db-health -- --tables        # Table statistics only
//! cargo run --bin db-health -- --indexes       # Index statistics only
//! ```

use std::env;
use std::time::Duration;

use colored::Colorize;
use pharma_db::{DbDiagnostics, create_connection};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// CLI Arguments
// ============================================================================

struct Args {
    json_output: bool,
    watch_mode: bool,
    watch_interval_secs: u64,
    analyze_queries: bool,
    tables_only: bool,
    indexes_only: bool,
    show_unused_indexes: bool,
    vacuum_threshold: f64,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = env::args().collect();

        Self {
            json_output: args.iter().any(|a| a == "--json" || a == "-j"),
            watch_mode: args.iter().any(|a| a == "--watch" || a == "-w"),
            watch_interval_secs: args
                .iter()
                .position(|a| a == "--interval")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
            analyze_queries: args.iter().any(|a| a == "--analyze" || a == "-a"),
            tables_only: args.iter().any(|a| a == "--tables" || a == "-t"),
            indexes_only: args.iter().any(|a| a == "--indexes" || a == "-i"),
            show_unused_indexes: args.iter().any(|a| a == "--unused"),
            vacuum_threshold: args
                .iter()
                .position(|a| a == "--vacuum-threshold")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.1), // 10% dead tuples
        }
    }

    fn show_help() -> bool {
        let args: Vec<String> = env::args().collect();
        args.iter().any(|a| a == "--help" || a == "-h")
    }
}

fn print_help() {
    println!(
        r#"
Database Health Monitor

USAGE:
    db-health [OPTIONS]

OPTIONS:
    -h, --help              Show this help message
    -j, --json              Output in JSON format
    -w, --watch             Continuous monitoring mode
    --interval <SECS>       Watch interval in seconds (default: 60)
    -a, --analyze           Analyze critical queries with EXPLAIN
    -t, --tables            Show table statistics only
    -i, --indexes           Show index statistics only
    --unused                Show unused indexes
    --vacuum-threshold <F>  Dead tuple ratio threshold for vacuum warning (default: 0.1)

ENVIRONMENT:
    DATABASE_URL            PostgreSQL connection string

EXAMPLES:
    db-health                       # Full health report
    db-health --json                # JSON output for scripting
    db-health --watch --interval 30 # Monitor every 30 seconds
    db-health --analyze             # Analyze query performance
    db-health --unused              # Find unused indexes
"#
    );
}

// ============================================================================
// Output Formatting
// ============================================================================

fn print_header(title: &str) {
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{}", title.bold().cyan());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
}

fn format_ratio(ratio: f64) -> colored::ColoredString {
    if ratio >= 99.0 {
        format!("{:.1}%", ratio).green()
    } else if ratio >= 95.0 {
        format!("{:.1}%", ratio).yellow()
    } else {
        format!("{:.1}%", ratio).red()
    }
}

fn format_connection_usage(current: i64, max: i64) -> colored::ColoredString {
    let ratio = current as f64 / max as f64 * 100.0;
    let text = format!("{}/{} ({:.1}%)", current, max, ratio);
    if ratio < 50.0 {
        text.green()
    } else if ratio < 80.0 {
        text.yellow()
    } else {
        text.red()
    }
}

// ============================================================================
// Report Functions
// ============================================================================

async fn print_overview(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    print_header("📊 Database Overview");

    let (conn_count, max_conn) = DbDiagnostics::get_connection_info(db).await?;
    let db_size = DbDiagnostics::get_database_size(db).await?;
    let cache_ratio = DbDiagnostics::get_cache_hit_ratio(db).await?;

    println!(
        "  Connections:     {}",
        format_connection_usage(conn_count, max_conn)
    );
    println!("  Database Size:   {}", db_size.cyan());
    println!("  Cache Hit Ratio: {}", format_ratio(cache_ratio));

    Ok(())
}

async fn print_table_stats(
    db: &sea_orm::DatabaseConnection,
    vacuum_threshold: f64,
) -> anyhow::Result<()> {
    print_header("📋 Table Statistics");

    let stats = DbDiagnostics::get_table_stats(db).await?;

    if stats.is_empty() {
        println!("  No user tables found.");
        return Ok(());
    }

    println!(
        "  {:<30} {:>12} {:>12} {:>12} {:>12}",
        "Table".bold(),
        "Rows".bold(),
        "Dead".bold(),
        "Table Size".bold(),
        "Total Size".bold()
    );
    println!("  {}", "-".repeat(80));

    for table in &stats {
        let dead_ratio = if table.row_count > 0 {
            table.dead_tuples as f64 / table.row_count as f64
        } else {
            0.0
        };

        let dead_str = if dead_ratio > vacuum_threshold {
            format!("{} ⚠", table.dead_tuples).red()
        } else {
            table.dead_tuples.to_string().normal()
        };

        println!(
            "  {:<30} {:>12} {:>12} {:>12} {:>12}",
            table.table_name, table.row_count, dead_str, table.table_size, table.total_size
        );
    }

    // Check for tables needing vacuum
    let needs_vacuum = DbDiagnostics::get_tables_needing_vacuum(db, vacuum_threshold).await?;
    if !needs_vacuum.is_empty() {
        println!();
        println!(
            "  {} {} table(s) have high dead tuple ratio (>{:.0}%):",
            "⚠".yellow(),
            needs_vacuum.len(),
            vacuum_threshold * 100.0
        );
        for table in needs_vacuum {
            println!("    - {}", table.table_name.yellow());
        }
        println!("  Consider running: {}", "VACUUM ANALYZE <table>".cyan());
    }

    Ok(())
}

async fn print_index_stats(
    db: &sea_orm::DatabaseConnection,
    show_unused: bool,
) -> anyhow::Result<()> {
    print_header("🔍 Index Statistics");

    let stats = DbDiagnostics::get_index_stats(db).await?;

    if stats.is_empty() {
        println!("  No user indexes found.");
        return Ok(());
    }

    println!(
        "  {:<40} {:<20} {:>12} {:>12}",
        "Index".bold(),
        "Table".bold(),
        "Scans".bold(),
        "Size".bold()
    );
    println!("  {}", "-".repeat(90));

    for idx in stats.iter().take(20) {
        let scans_str = if idx.index_scans == 0 {
            "0".red()
        } else {
            idx.index_scans.to_string().green()
        };

        println!(
            "  {:<40} {:<20} {:>12} {:>12}",
            idx.index_name, idx.table_name, scans_str, idx.index_size
        );
    }

    if stats.len() > 20 {
        println!("  ... and {} more indexes", stats.len() - 20);
    }

    // Show unused indexes if requested
    if show_unused {
        let unused = DbDiagnostics::get_unused_indexes(db).await?;
        if !unused.is_empty() {
            println!();
            println!(
                "  {} {} unused index(es) (0 scans, excluding primary keys):",
                "⚠".yellow(),
                unused.len()
            );
            for idx in unused.iter().take(10) {
                println!(
                    "    - {} on {} ({})",
                    idx.index_name.yellow(),
                    idx.table_name,
                    idx.index_size
                );
            }
            if unused.len() > 10 {
                println!("    ... and {} more", unused.len() - 10);
            }
            println!(
                "  Consider dropping unused indexes to save space and improve write performance."
            );
        }
    }

    Ok(())
}

async fn print_query_analysis(db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
    print_header("🔬 Critical Query Analysis");

    let analyses = DbDiagnostics::analyze_critical_queries(db).await?;

    if analyses.is_empty() {
        println!("  No queries analyzed.");
        return Ok(());
    }

    for (name, analysis) in &analyses {
        let status = if analysis.warnings.is_empty() {
            "✓".green()
        } else {
            "⚠".yellow()
        };

        let scan_type = if analysis.uses_index {
            "Index".green()
        } else if analysis.uses_seq_scan {
            "Seq Scan".red()
        } else {
            "Other".normal()
        };

        let time_str = analysis
            .execution_time_ms
            .map(|t| format!("{:.2}ms", t))
            .unwrap_or_else(|| "N/A".to_string());

        println!();
        println!("  {} {}", status, name.bold());
        println!("    Scan Type: {}", scan_type);
        println!("    Execution Time: {}", time_str);

        if !analysis.warnings.is_empty() {
            for warning in &analysis.warnings {
                println!("    {} {}", "Warning:".yellow(), warning);
            }
        }
    }

    Ok(())
}

async fn run_health_check(db: &sea_orm::DatabaseConnection, args: &Args) -> anyhow::Result<()> {
    if args.json_output {
        let health = DbDiagnostics::get_health_summary(db).await?;
        println!("{}", serde_json::to_string_pretty(&health)?);
        return Ok(());
    }

    // Print banner
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              🏥 DATABASE HEALTH MONITOR                                      ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "║              Performance analysis and diagnostics                            ║".green()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".green()
    );

    // Run requested reports
    if args.tables_only {
        print_table_stats(db, args.vacuum_threshold).await?;
    } else if args.indexes_only {
        print_index_stats(db, args.show_unused_indexes).await?;
    } else if args.analyze_queries {
        print_query_analysis(db).await?;
    } else {
        // Full report
        print_overview(db).await?;
        print_table_stats(db, args.vacuum_threshold).await?;
        print_index_stats(db, args.show_unused_indexes).await?;

        // Always analyze queries in full report
        print_query_analysis(db).await?;
    }

    println!();
    println!("✅ Health check complete.");
    println!();

    Ok(())
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

    // Parse arguments
    if Args::show_help() {
        print_help();
        return Ok(());
    }

    let args = Args::parse();

    // Load environment
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!("🗄️  Connecting to database...");
    let db = create_connection(&database_url).await?;
    println!("✅ Connected");

    if args.watch_mode {
        // Continuous monitoring
        println!(
            "👁️  Watch mode enabled (interval: {}s). Press Ctrl+C to stop.",
            args.watch_interval_secs
        );

        loop {
            // Clear screen for fresh output
            print!("\x1B[2J\x1B[1;1H");

            run_health_check(&db, &args).await?;

            println!(
                "Next check in {} seconds... (Ctrl+C to stop)",
                args.watch_interval_secs
            );

            tokio::time::sleep(Duration::from_secs(args.watch_interval_secs)).await;
        }
    } else {
        run_health_check(&db, &args).await?;
    }

    Ok(())
}
