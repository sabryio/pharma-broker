//! # AI Parser Benchmark CLI
//!
//! Benchmarks AI parsing performance with concurrent requests.
//! Loads messages from database and measures parsing latency per message.
//!
//! ## Usage
//! ```bash
//! cargo run --bin bench-parser -- --limit 5 --concurrency 3
//! cargo run --bin bench-parser -- -l 10 -c 5
//! ```
//!
//! ## Environment Variables
//! - `AI_BASE_URL` / `AI_MODEL` - AI model configuration
//! - `DATABASE_URL` - PostgreSQL connection string
//! - `RUST_LOG` - Logging level (default: warn)

use std::env;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use pharma_core::ai::{
    BatchMessage, ItemType, PharmaParser, PharmaParserConfig, TokenBatchConfig, TokenBatcher,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Semaphore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// CLI Arguments
// ============================================================================

/// Benchmark AI parsing performance with concurrent requests
#[derive(Parser, Debug)]
#[command(name = "bench-parser")]
#[command(author = "PharmaBroker Team")]
#[command(version = "0.1.0")]
#[command(about = "Benchmark AI parsing with concurrency control", long_about = None)]
struct Args {
    /// Number of messages to process from database
    #[arg(short, long, default_value_t = 5)]
    limit: i64,

    /// Maximum concurrent parsing requests
    #[arg(short, long, default_value_t = 3)]
    concurrency: usize,

    /// Database URL (defaults to DATABASE_URL env var)
    #[arg(long)]
    database_url: Option<String>,
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
struct LegacyMessage {
    #[allow(dead_code)]
    id: String,
    content: String,
    sender_name: Option<String>,
    group_name: Option<String>,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize tracing (suppress by default for clean output)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let limit = args.limit;
    let concurrency = args.concurrency;

    println!("🚀 AI Parser Benchmark CLI");
    println!("   --limit: {}", limit);
    println!("   --concurrency: {}", concurrency);

    // Connect to database
    let database_url = args.database_url.unwrap_or_else(|| {
        env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:password@localhost:5432/pharmabroker".to_string()
        })
    });

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Fetch legacy messages
    println!("\n📥 Fetching messages from database...");
    let messages: Vec<LegacyMessage> = sqlx::query_as!(
        LegacyMessage,
        r#"SELECT id, content, sender_name, group_name FROM raw_messages LIMIT $1"#,
        limit
    )
    .fetch_all(&pool)
    .await?;

    if messages.is_empty() {
        println!("❌ No messages found in database.");
        return Ok(());
    }

    println!("   Found {} messages", messages.len());

    // Create direct AI parser (uses Docker Model Runner directly)
    let parser_config = PharmaParserConfig::from_env();
    println!(
        "\n🔗 AI Backend: {}",
        env::var("AI_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string())
    );

    let parser = Arc::new(PharmaParser::new(parser_config));

    // Create token batcher for context management
    let batcher = TokenBatcher::new(TokenBatchConfig::default());

    // Convert to batch messages
    let batch_messages: Vec<BatchMessage> = messages
        .iter()
        .map(|m| {
            let mut msg = BatchMessage::new(&m.id, &m.content);
            if let Some(ref sender) = m.sender_name {
                msg = msg.with_sender(sender);
            }
            if let Some(ref group) = m.group_name {
                msg = msg.with_group(group);
            }
            msg
        })
        .collect();

    // Split into token-aware batches
    let batches = batcher.split_into_batches(batch_messages.clone());
    println!("📦 Split into {} token-aware batches", batches.len());

    // Process with concurrency limit
    println!("\n⏳ Benchmarking with Direct AI Client...\n");
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let mut handles = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        let p = Arc::clone(&parser);
        let sem = Arc::clone(&semaphore);
        let content = msg.content.clone();
        let sender = msg.sender_name.clone();
        let group = msg.group_name.clone();
        let msg_id = msg.id.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let start = Instant::now();

            let result = p
                .parse(&content, sender.as_deref(), group.as_deref(), None, None)
                .await;

            let latency = start.elapsed().as_millis();

            (idx, msg_id, content, result, latency)
        });

        handles.push(handle);
    }

    // Collect results
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await?);
    }

    // Sort by index to maintain order
    results.sort_by_key(|(idx, _, _, _, _)| *idx);

    // Print results
    println!("{}", "=".repeat(100));
    println!(
        "{:<5} {:<50} {:<8} {:<10} {:<20}",
        "IDX", "CONTENT", "ITEMS", "LATENCY", "STATUS"
    );
    println!("{}", "=".repeat(100));

    let mut total_items = 0;
    let mut total_latency = 0u128;
    let mut success_count = 0;
    let mut error_count = 0;

    for (idx, _msg_id, content, result, latency) in &results {
        let content_preview: String = content
            .chars()
            .take(47)
            .collect::<String>()
            .replace('\n', " ");
        let content_display = if content.len() > 47 {
            format!("{}...", content_preview)
        } else {
            content_preview
        };

        match result {
            Ok(items) => {
                println!(
                    "{:<5} {:<50} {:<8} {:<10} {:<20}",
                    idx,
                    content_display,
                    items.len(),
                    format!("{}ms", latency),
                    "✅ OK"
                );
                total_items += items.len();
                success_count += 1;

                // Print parsed items
                for item in items {
                    println!(
                        "       └─ {} {} (qty: {}, price: {}, conf: {:.0}%)",
                        if item.item_type == ItemType::Offer {
                            "🟢"
                        } else {
                            "🔵"
                        },
                        item.medication,
                        item.quantity,
                        item.price,
                        item.ai_confidence * 100.0
                    );
                }
            }
            Err(e) => {
                println!(
                    "{:<5} {:<50} {:<8} {:<10} {:<20}",
                    idx,
                    content_display,
                    "-",
                    format!("{}ms", latency),
                    format!("❌ {}", e)
                );
                error_count += 1;
            }
        }

        total_latency += latency;
    }

    println!("{}", "=".repeat(100));

    // Summary
    let avg_latency = if !results.is_empty() {
        total_latency / results.len() as u128
    } else {
        0
    };

    println!("\n📊 Summary:");
    println!("   - Total messages: {}", results.len());
    println!("   - Successful: {}", success_count);
    println!("   - Failed: {}", error_count);
    println!("   - Total items parsed: {}", total_items);
    println!("   - Average latency: {}ms", avg_latency);

    // Token batcher stats
    let stats = batcher.stats();
    println!("\n📦 Token Batcher Stats:");
    println!("   - Total batches: {}", stats.total_batches);
    println!("   - Total tokens estimated: {}", stats.total_tokens);
    println!("   - Split batches: {}", stats.split_batches);
    println!("   - Oversized messages: {}", stats.oversized_messages);

    // Circuit breaker state
    println!("\n🔌 Circuit Breaker: {:?}", parser.circuit_state());

    println!("\n✅ Benchmark complete.");

    Ok(())
}
