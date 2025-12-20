//! # Model Comparison CLI
//!
//! Interactive tool for comparing AI parsing across multiple LLM models.
//! Uses colored output and dialoguer prompts for beautiful UX.
//!
//! ## Usage
//! ```bash
//! cargo run --bin compare-models
//! ```

use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};
use prettytable::format::consts::FORMAT_BOX_CHARS;
use prettytable::{Table, row};
use sqlx::postgres::PgPoolOptions;
use tokio::time::timeout;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::{ParsedItem, PharmaParser, PharmaParserConfig};

// ============================================================================
// Interactive Config
// ============================================================================

struct Config {
    limit: i64,
    timeout_secs: u64,
    database_url: String,
}

fn get_config_interactive() -> Config {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║              🔬 LLM MODEL COMPARISON TOOL                                    ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Comparing: Qwen3-VL, Ministral3, Gemma3                         ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();

    let theme = ColorfulTheme::default();

    let limit: i64 = Input::with_theme(&theme)
        .with_prompt("📝 Number of messages to test")
        .default(3)
        .interact_text()
        .unwrap_or(3);

    let timeout_secs: u64 = Input::with_theme(&theme)
        .with_prompt("⏱️  Timeout per model (seconds)")
        .default(60)
        .interact_text()
        .unwrap_or(60);

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
        "  {} {} messages, {} {}s timeout",
        "Config:".green().bold(),
        limit.to_string().yellow(),
        "⏱️".dimmed(),
        timeout_secs.to_string().yellow()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!();

    Config {
        limit,
        timeout_secs,
        database_url,
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
struct TestMessage {
    #[allow(dead_code)]
    id: String,
    content: String,
    sender_name: Option<String>,
    group_name: Option<String>,
}

#[derive(Debug, Clone)]
struct ModelConfig {
    name: String,
    base_url: String,
    model: String,
}

#[derive(Debug, Clone, Default)]
struct ModelResult {
    model_name: String,
    items_count: usize,
    latency_ms: u128,
    success: bool,
    error: Option<String>,
    items: Vec<ParsedItem>,
}

#[derive(Debug, Clone, Default)]
struct ModelStats {
    model_name: String,
    total_messages: usize,
    successful: usize,
    failed: usize,
    total_items: usize,
    avg_latency_ms: u128,
    min_latency_ms: u128,
    max_latency_ms: u128,
}

// ============================================================================
// Model Configurations
// ============================================================================

fn get_model_configs() -> Vec<ModelConfig> {
    vec![
        ModelConfig {
            name: "Qwen3-VL".to_string(),
            base_url: env::var("AI_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string()),
            model: env::var("AI_MODEL").unwrap_or_else(|_| "ai/qwen3-vl:latest".to_string()),
        },
        ModelConfig {
            name: "Ministral3".to_string(),
            base_url: env::var("MINISTRAL_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string()),
            model: env::var("MINISTRAL_MODEL")
                .unwrap_or_else(|_| "ai/ministral3:latest".to_string()),
        },
        ModelConfig {
            name: "Gemma3".to_string(),
            base_url: env::var("GEMMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string()),
            model: env::var("GEMMA_MODEL").unwrap_or_else(|_| "ai/gemma3:latest".to_string()),
        },
    ]
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (before interactive prompts)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get config via interactive prompts
    let config = get_config_interactive();
    let limit = config.limit;
    let timeout_secs = config.timeout_secs;

    // Get model configurations
    let models = get_model_configs();

    // Print model configuration table
    let mut config_table = Table::new();
    config_table.set_format(*FORMAT_BOX_CHARS);
    config_table.add_row(row!["Model", "Endpoint", "Model ID"]);
    for m in &models {
        config_table.add_row(row![m.name, m.base_url, m.model]);
    }
    println!("📋 Model Configuration:");
    config_table.printstd();

    // Connect to database
    let database_url = config.database_url;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Fetch test messages
    println!("\n📥 Loading {} test messages from database...", limit);
    let messages: Vec<TestMessage> = sqlx::query_as!(
        TestMessage,
        r#"SELECT id, content, sender_name, group_name FROM raw_messages LIMIT $1"#,
        limit
    )
    .fetch_all(&pool)
    .await?;

    if messages.is_empty() {
        println!("❌ No messages found in database.");
        return Ok(());
    }

    println!("   ✅ Loaded {} messages\n", messages.len());

    // Create parsers for each model
    let mut parsers: Vec<(String, Arc<PharmaParser>)> = Vec::new();

    for model in &models {
        let client_config = pharma_core::ai::ClientConfig {
            base_url: model.base_url.clone(),
            model: model.model.clone(),
            timeout: Duration::from_secs(timeout_secs),
            ..Default::default()
        };

        let config = PharmaParserConfig {
            client: client_config,
            ..Default::default()
        };
        parsers.push((model.name.clone(), Arc::new(PharmaParser::new(config))));
    }

    // Run comparisons
    let mut all_results: Vec<Vec<ModelResult>> = Vec::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        println!(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );
        let content_preview: String = msg
            .content
            .chars()
            .take(60)
            .collect::<String>()
            .replace('\n', " ");
        println!(
            "📝 Message {}/{}: \"{}{}\"",
            msg_idx + 1,
            messages.len(),
            content_preview,
            if msg.content.len() > 60 { "..." } else { "" }
        );
        println!();

        // Run all models concurrently
        println!("   ⏳ Running {} models concurrently...", parsers.len());

        let mut handles = Vec::new();

        for (model_name, parser) in &parsers {
            let model_name = model_name.clone();
            let parser = Arc::clone(parser);
            let content = msg.content.clone();
            let sender = msg.sender_name.clone();
            let group = msg.group_name.clone();

            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let parse_fut =
                    parser.parse(&content, sender.as_deref(), group.as_deref(), None, None);

                let result = timeout(Duration::from_secs(timeout_secs), parse_fut).await;
                let latency = start.elapsed().as_millis();

                match result {
                    Ok(Ok(items)) => ModelResult {
                        model_name,
                        items_count: items.len(),
                        latency_ms: latency,
                        success: true,
                        error: None,
                        items,
                    },
                    Ok(Err(e)) => ModelResult {
                        model_name,
                        items_count: 0,
                        latency_ms: latency,
                        success: false,
                        error: Some(e.to_string()),
                        items: vec![],
                    },
                    Err(_) => ModelResult {
                        model_name,
                        items_count: 0,
                        latency_ms: latency,
                        success: false,
                        error: Some(format!("Timeout after {}s", timeout_secs)),
                        items: vec![],
                    },
                }
            });

            handles.push(handle);
        }

        // Wait for all models to complete
        let mut msg_results: Vec<ModelResult> = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                println!(
                    "   {} {} - {} items in {}ms",
                    if result.success { "✅" } else { "❌" },
                    result.model_name,
                    result.items_count,
                    result.latency_ms
                );
                msg_results.push(result);
            }
        }

        // Display comparison table for this message
        println!();
        let mut comparison_table = Table::new();
        comparison_table.set_format(*FORMAT_BOX_CHARS);
        comparison_table.add_row(row!["Model", "Items", "Latency", "Status"]);

        for r in &msg_results {
            let status = if r.success {
                "✅ OK".to_string()
            } else {
                format!("❌ {}", r.error.as_deref().unwrap_or("Error"))
            };
            comparison_table.add_row(row![
                r.model_name,
                r.items_count,
                format!("{}ms", r.latency_ms),
                status
            ]);
        }
        comparison_table.printstd();

        // Show parsed items comparison
        let all_successful: Vec<_> = msg_results.iter().filter(|r| r.success).collect();
        if !all_successful.is_empty() {
            println!("\n   📊 Parsed Items Comparison:");

            let mut items_table = Table::new();
            items_table.set_format(*FORMAT_BOX_CHARS);
            items_table.add_row(row!["Model", "Type", "Medication", "Qty", "Price", "Conf%"]);

            for r in &msg_results {
                for item in &r.items {
                    items_table.add_row(row![
                        r.model_name,
                        format!("{:?}", item.item_type),
                        item.medication,
                        item.quantity,
                        item.price,
                        format!("{:.0}%", item.ai_confidence * 100.0)
                    ]);
                }
            }

            if items_table.len() > 1 {
                items_table.printstd();
            }
        }

        all_results.push(msg_results);
        println!();
    }

    // ========================================================================
    // Final Summary
    // ========================================================================
    println!("\n╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                         📊 FINAL SUMMARY                                     ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝\n");

    // Calculate stats per model
    let mut model_stats: Vec<ModelStats> = Vec::new();

    for (model_name, _) in &parsers {
        let model_results: Vec<&ModelResult> = all_results
            .iter()
            .flat_map(|r| r.iter())
            .filter(|r| &r.model_name == model_name)
            .collect();

        let successful = model_results.iter().filter(|r| r.success).count();
        let total_items: usize = model_results.iter().map(|r| r.items_count).sum();
        let latencies: Vec<u128> = model_results.iter().map(|r| r.latency_ms).collect();

        let avg_latency = if !latencies.is_empty() {
            latencies.iter().sum::<u128>() / latencies.len() as u128
        } else {
            0
        };
        let min_latency = latencies.iter().min().copied().unwrap_or(0);
        let max_latency = latencies.iter().max().copied().unwrap_or(0);

        model_stats.push(ModelStats {
            model_name: model_name.clone(),
            total_messages: model_results.len(),
            successful,
            failed: model_results.len() - successful,
            total_items,
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
        });
    }

    // Summary table
    let mut summary_table = Table::new();
    summary_table.set_format(*FORMAT_BOX_CHARS);
    summary_table.add_row(row![
        "Model",
        "Success",
        "Failed",
        "Items",
        "Avg Latency",
        "Min",
        "Max"
    ]);

    for s in &model_stats {
        let success_rate = if s.total_messages > 0 {
            (s.successful as f64 / s.total_messages as f64) * 100.0
        } else {
            0.0
        };

        summary_table.add_row(row![
            s.model_name,
            format!(
                "{}/{} ({:.0}%)",
                s.successful, s.total_messages, success_rate
            ),
            s.failed,
            s.total_items,
            format!("{}ms", s.avg_latency_ms),
            format!("{}ms", s.min_latency_ms),
            format!("{}ms", s.max_latency_ms)
        ]);
    }

    summary_table.printstd();

    // Find winner (use i64 to avoid underflow, prioritize success rate then speed)
    if let Some(best) = model_stats.iter().max_by(|a, b| {
        // Compare by success count first, then by inverse latency (lower is better)
        let a_score = (a.successful as i64 * 100000) - (a.avg_latency_ms as i64);
        let b_score = (b.successful as i64 * 100000) - (b.avg_latency_ms as i64);
        a_score.cmp(&b_score)
    }) {
        println!(
            "\n🏆 Best Overall: {} ({:.0}% success, {}ms avg latency)",
            best.model_name,
            if best.total_messages > 0 {
                (best.successful as f64 / best.total_messages as f64) * 100.0
            } else {
                0.0
            },
            best.avg_latency_ms
        );
    }

    if let Some(fastest) = model_stats
        .iter()
        .filter(|s| s.successful > 0)
        .min_by_key(|s| s.avg_latency_ms)
    {
        println!(
            "⚡ Fastest: {} ({}ms avg)",
            fastest.model_name, fastest.avg_latency_ms
        );
    }

    if let Some(most_items) = model_stats.iter().max_by_key(|s| s.total_items) {
        println!(
            "📦 Most Items Extracted: {} ({} items)",
            most_items.model_name, most_items.total_items
        );
    }

    println!("\n✅ Comparison complete!");

    Ok(())
}
