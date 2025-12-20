//! Playground to test AI parsing
//!
//! Usage:
//!   cargo run --bin ai_playground                               # Load from database (default 10)
//!   cargo run --bin ai_playground -- -l 20                      # Load 20 messages from database
//!   cargo run --bin ai_playground -- -i messages.json           # Load from file
//!   cargo run --bin ai_playground -- -i messages.json -o results.json  # Save results
//!   cargo run --bin ai_playground -- --help

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::{ParsedItem, PharmaParser, PharmaParserConfig};

// ============================================================================
// Types
// ============================================================================

/// Test message loaded from JSON or database
#[derive(Debug, Clone, Deserialize)]
struct TestMessage {
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    group_jid: String,
    #[serde(default)]
    group_name: String,
    content: String,
    #[serde(default)]
    sender_name: Option<String>,
    #[serde(default)]
    reply_to: Option<String>,
}

/// Database row for raw_messages
#[derive(Debug, Clone)]
struct DbMessage {
    id: String,
    content: String,
    sender_name: Option<String>,
    group_name: Option<String>,
}

/// Test result for a single message
#[derive(Debug, Clone, Serialize)]
struct TestResult {
    message_id: String,
    content: String,
    parsed_items: Vec<ParsedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    elapsed_ms: u128,
}

/// Full test summary
#[derive(Debug, Clone, Serialize)]
struct TestSummary {
    gateway_url: String,
    total_time: String,
    total_messages: usize,
    successful: usize,
    failed: usize,
    total_items: usize,
    results: Vec<TestResult>,
    tested_at: DateTime<Utc>,
}

// ============================================================================
// CLI Args
// ============================================================================

struct Args {
    input_file: Option<PathBuf>,
    output_file: Option<PathBuf>,
    mappings_file: Option<PathBuf>,
    limit: i64,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();
    let mut input_file = None;
    let mut output_file = None;
    let mut mappings_file = None;
    let mut limit: i64 = 10;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input" => {
                if i + 1 < args.len() {
                    input_file = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_file = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "-m" | "--mappings" => {
                if i + 1 < args.len() {
                    mappings_file = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "-l" | "--limit" => {
                if i + 1 < args.len() {
                    limit = args[i + 1].parse().unwrap_or(10);
                    i += 1;
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    Args {
        input_file,
        output_file,
        mappings_file,
        limit,
    }
}

fn print_help() {
    println!(
        r#"AI Playground - Test AI Gateway parsing

USAGE:
    cargo run --bin ai_playground -- [OPTIONS]

OPTIONS:
    -i, --input <FILE>      JSON file with test messages (optional, loads from DB if not provided)
    -o, --output <FILE>     JSON file to save results
    -m, --mappings <FILE>   JSON file with medication mappings
    -l, --limit <N>         Number of messages to load from database (default: 10)
    -h, --help              Print help information

EXAMPLES:
    cargo run --bin ai_playground                    # Load 10 messages from database
    cargo run --bin ai_playground -- -l 20           # Load 20 messages from database
    cargo run --bin ai_playground -- -i messages.json -o results.json

INPUT FORMAT (messages.json):
    [
      {{
        "id": "msg-1",
        "group_jid": "group@g.us",
        "group_name": "Pharmacy Group",
        "content": "عندي اوجمنتين 1 جم 50 علبة بـ 150",
        "sender_name": "Dr. Ahmed"
      }}
    ]
"#
    );
}

// ============================================================================
// File I/O
// ============================================================================

fn load_messages_from_file(path: &PathBuf) -> anyhow::Result<Vec<TestMessage>> {
    let data = fs::read_to_string(path)?;
    let messages: Vec<TestMessage> = serde_json::from_str(&data)?;
    Ok(messages)
}

fn load_mappings_from_file(path: &PathBuf) -> anyhow::Result<Vec<String>> {
    let data = fs::read_to_string(path)?;

    // Try to parse as array of objects with "arabic" and "english" fields
    #[derive(Deserialize)]
    struct Mapping {
        arabic: Option<String>,
        english: Option<String>,
    }

    if let Ok(mappings) = serde_json::from_str::<Vec<Mapping>>(&data) {
        let result: Vec<String> = mappings
            .iter()
            .filter_map(|m| match (&m.arabic, &m.english) {
                (Some(ar), Some(en)) => Some(format!("{} = {}", ar, en)),
                _ => None,
            })
            .collect();
        return Ok(result);
    }

    // Fallback: try as simple string array
    let mappings: Vec<String> = serde_json::from_str(&data)?;
    Ok(mappings)
}

fn save_results_to_file(path: &PathBuf, summary: &TestSummary) -> anyhow::Result<()> {
    let data = serde_json::to_string_pretty(summary)?;
    fs::write(path, data)?;
    Ok(())
}

async fn load_messages_from_database(limit: i64) -> anyhow::Result<Vec<TestMessage>> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let db_messages: Vec<DbMessage> = sqlx::query_as!(
        DbMessage,
        r#"SELECT id, content, sender_name, group_name FROM raw_messages LIMIT $1"#,
        limit
    )
    .fetch_all(&pool)
    .await?;

    let messages: Vec<TestMessage> = db_messages
        .into_iter()
        .map(|m| TestMessage {
            id: m.id,
            group_jid: String::new(),
            group_name: m.group_name.unwrap_or_default(),
            content: m.content,
            sender_name: m.sender_name,
            reply_to: None,
        })
        .collect();

    Ok(messages)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse CLI args
    let args = parse_args();

    // Create AI client
    let parser_config = PharmaParserConfig::from_env();
    let ai_parser = PharmaParser::new(parser_config);

    info!("Testing Direct AI Client (no gateway)");

    // Load test messages from file or database
    let test_messages = if let Some(ref input_file) = args.input_file {
        match load_messages_from_file(input_file) {
            Ok(msgs) => {
                info!(
                    count = msgs.len(),
                    file = ?input_file,
                    "Loaded messages from file"
                );
                msgs
            }
            Err(e) => {
                error!(error = %e, file = ?input_file, "Failed to load messages from file");
                std::process::exit(1);
            }
        }
    } else {
        info!(limit = args.limit, "Loading messages from database...");
        match load_messages_from_database(args.limit).await {
            Ok(msgs) => {
                if msgs.is_empty() {
                    error!("No messages found in database");
                    std::process::exit(1);
                }
                info!(count = msgs.len(), "Loaded messages from database");
                msgs
            }
            Err(e) => {
                error!(error = %e, "Failed to load messages from database");
                std::process::exit(1);
            }
        }
    };

    // Load medication mappings if provided
    let mappings: Option<Vec<String>> = if let Some(ref mappings_file) = args.mappings_file {
        match load_mappings_from_file(mappings_file) {
            Ok(m) => {
                info!(count = m.len(), file = ?mappings_file, "Loaded medication mappings");
                Some(m)
            }
            Err(e) => {
                warn!(error = %e, file = ?mappings_file, "Failed to load mappings");
                None
            }
        }
    } else {
        None
    };

    // Print header
    let separator = "=".repeat(60);
    println!("\n{}", separator);
    println!("PARSING TEST");
    println!("{}", separator);
    println!("AI: Direct (no gateway)");
    println!("Messages: {}", test_messages.len());
    println!("{}", separator);

    // Run parsing
    let total_start = Instant::now();
    let mut results: Vec<TestResult> = Vec::with_capacity(test_messages.len());
    let mut successful = 0;
    let mut failed = 0;
    let mut total_items = 0;

    for (i, msg) in test_messages.iter().enumerate() {
        let start = Instant::now();

        let parse_result = ai_parser
            .parse(
                &msg.content,
                msg.sender_name.as_deref(),
                Some(&msg.group_name),
                msg.reply_to.as_deref(),
                mappings.as_deref(),
            )
            .await;

        let elapsed = start.elapsed();

        // Print result
        println!("\n--- Message {} (ID: {}) ---", i + 1, msg.id);
        println!("Content:\n{}", msg.content);

        match parse_result {
            Ok(items) => {
                successful += 1;
                total_items += items.len();

                println!("\n✅ Parsed {} items:", items.len());
                for (j, item) in items.iter().enumerate() {
                    print!("  {}. [{}] {}", j + 1, item.item_type, item.medication);
                    if item.quantity > 0.0 {
                        let unit = item.unit.as_deref().unwrap_or("");
                        print!(" (qty: {:.0} {})", item.quantity, unit);
                    }
                    if item.price > 0.0 {
                        print!(" @ {:.0}", item.price);
                    }
                    if item.urgent {
                        print!(" ⚠️URGENT");
                    }
                    print!(" [conf: {:.0}%]", item.ai_confidence * 100.0);
                    println!();
                }

                results.push(TestResult {
                    message_id: msg.id.clone(),
                    content: msg.content.clone(),
                    parsed_items: items,
                    error: None,
                    elapsed_ms: elapsed.as_millis(),
                });
            }
            Err(e) => {
                failed += 1;
                println!("\n❌ ERROR: {}", e);

                results.push(TestResult {
                    message_id: msg.id.clone(),
                    content: msg.content.clone(),
                    parsed_items: vec![],
                    error: Some(e.to_string()),
                    elapsed_ms: elapsed.as_millis(),
                });
            }
        }

        println!("   ⏱️  {}ms", elapsed.as_millis());
    }

    let total_elapsed = total_start.elapsed();

    // Print summary
    println!("\n{}", separator);
    println!("📊 SUMMARY");
    println!("{}", separator);
    println!("Total messages: {}", test_messages.len());
    println!("Successful: {}", successful);
    println!("Failed: {}", failed);
    println!("Total items extracted: {}", total_items);
    println!("Total time: {:?}", total_elapsed);
    if !results.is_empty() {
        let avg_ms = total_elapsed.as_millis() / results.len() as u128;
        println!("Average per message: {}ms", avg_ms);
    }

    // Save results if output file specified
    if let Some(ref output_file) = args.output_file {
        let summary = TestSummary {
            gateway_url: "Direct AI (no gateway)".to_string(),
            total_time: format!("{:?}", total_elapsed),
            total_messages: test_messages.len(),
            successful,
            failed,
            total_items,
            results,
            tested_at: Utc::now(),
        };

        match save_results_to_file(output_file, &summary) {
            Ok(_) => {
                info!(file = ?output_file, "Results saved to file");
                println!("\n💾 Results saved to: {:?}", output_file);
            }
            Err(e) => {
                error!(error = %e, file = ?output_file, "Failed to save results");
            }
        }
    }

    println!("\n{}", separator);
    println!("✅ Completed in {:?}", total_elapsed);

    Ok(())
}
