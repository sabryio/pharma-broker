//! # AI Parser Benchmark CLI
//!
//! Interactive benchmark tool for AI parsing performance.
//! Uses colored output and dialoguer prompts for beautiful UX.
//!
//! ## Usage
//! ```bash
//! cargo run --bin bench-parser
//! ```

use std::env;
use std::sync::Arc;
use std::time::Instant;

use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};
use pharma_core::ai::{
    BatchMessage, Intent, PharmaParser, PharmaParserConfig, TokenBatchConfig, TokenBatcher,
};
use pharma_core::repository::create_connection;
use pharma_db::entity::raw_message::Entity as RawMessage;
use sea_orm::{EntityTrait, QuerySelect};
use tokio::sync::Semaphore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// ============================================================================
// Interactive Config
// ============================================================================

struct Config {
    limit: i64,
    concurrency: usize,
    database_url: String,
}

fn get_config_interactive() -> Config {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              🚀 AI PARSER BENCHMARK                                          ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "║              Concurrent parsing performance test                             ║".green()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".green()
    );
    println!();

    let theme = ColorfulTheme::default();

    let limit: i64 = Input::with_theme(&theme)
        .with_prompt("📝 Number of messages to benchmark")
        .default(5)
        .interact_text()
        .unwrap_or(5);

    let concurrency: usize = Input::with_theme(&theme)
        .with_prompt("⚡ Concurrent parsing threads")
        .default(3)
        .interact_text()
        .unwrap_or(3);

    let default_db = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    let database_url: String = Input::with_theme(&theme)
        .with_prompt("🗄️  Database URL")
        .default(default_db)
        .interact_text()
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "  {} {} messages, {} workers",
        "Config:".green().bold(),
        limit.to_string().yellow(),
        concurrency.to_string().yellow()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!();

    Config {
        limit,
        concurrency,
        database_url,
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
struct LegacyMessage {
    id: Uuid,
    content: String,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (suppress by default for clean output)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get config via interactive prompts
    let config = get_config_interactive();
    let limit = config.limit;
    let concurrency = config.concurrency;
    let database_url = config.database_url;

    let db = create_connection(&database_url).await?;

    // Fetch legacy messages
    println!("\n📥 Fetching messages from database...");
    let db_messages = RawMessage::find().limit(limit as u64).all(&*db).await?;

    let messages: Vec<LegacyMessage> = db_messages
        .into_iter()
        .map(|m| LegacyMessage {
            id: m.id,
            content: m.content,
        })
        .collect();

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
        .map(|m| BatchMessage::new(m.id, &m.content))
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
        let msg_id = msg.id;

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let start = Instant::now();

            let result = p.parse(&content, None, "", None, None).await;

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
    results.sort_by_key(|(idx, _, _, _, _): &(usize, Uuid, String, _, u128)| *idx);

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
                        if item.item_type == Intent::Offer {
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
