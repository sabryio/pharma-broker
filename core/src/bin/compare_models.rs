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
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::{PharmaParser, PharmaParserConfig};

// ============================================================================
// Interactive Config
// ============================================================================

struct Config {
    limit: i64,
    concurrency: usize,
    timeout_secs: u64,
    database_url: String,
    selected_models: Vec<String>, // Empty means all models
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

    // Model selection
    let model_options = vec!["All Models", "Qwen3-VL", "Ministral3", "Gemma3"];
    let model_selection = dialoguer::Select::with_theme(&theme)
        .with_prompt("🤖 Select model(s) to test")
        .items(&model_options)
        .default(0)
        .interact()
        .unwrap_or(0);

    let selected_models: Vec<String> = if model_selection == 0 {
        vec![] // Empty = all models
    } else {
        vec![model_options[model_selection].to_string()]
    };

    let limit: i64 = Input::with_theme(&theme)
        .with_prompt("📝 Number of messages to test")
        .default(50)
        .interact_text()
        .unwrap_or(50);

    let concurrency: usize = Input::with_theme(&theme)
        .with_prompt("⚡ Concurrent requests per model")
        .default(3)
        .interact_text()
        .unwrap_or(3);

    let timeout_secs: u64 = Input::with_theme(&theme)
        .with_prompt("⏱️  Timeout per request (seconds)")
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

    let model_display = if selected_models.is_empty() {
        "All (3 models)"
    } else {
        &selected_models[0]
    };

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "  {} {} | {} msgs | {} concurrent | {}s timeout",
        "Config:".green().bold(),
        model_display.yellow(),
        limit.to_string().yellow(),
        concurrency.to_string().yellow(),
        timeout_secs.to_string().yellow()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!();

    Config {
        limit,
        concurrency,
        timeout_secs,
        database_url,
        selected_models,
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
struct TestMessage {
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

fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() <= max_len {
        s
    } else {
        format!("{}...", &s[..max_len])
    }
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
    let _timeout_secs = config.timeout_secs;

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

    // Filter models based on selection
    let filtered_models: Vec<ModelConfig> = if config.selected_models.is_empty() {
        models
    } else {
        models
            .into_iter()
            .filter(|m| config.selected_models.contains(&m.name))
            .collect()
    };

    if filtered_models.is_empty() {
        println!("{}", "❌ No matching models found.".red());
        return Ok(());
    }

    // Results storage: message_results[msg_idx][model_idx]
    let mut all_message_results: Vec<Vec<ModelResult>> =
        vec![vec![ModelResult::default(); filtered_models.len()]; messages.len()];

    // Process models SEQUENTIALLY
    for (model_idx, model) in filtered_models.iter().enumerate() {
        println!(
            "\n🚀 Testing Model {}/{} [{}]",
            model_idx + 1,
            filtered_models.len(),
            model.name.green().bold()
        );

        let client_config = pharma_core::ai::ClientConfig {
            base_url: model.base_url.clone(),
            model: model.model.clone(),
            timeout: Duration::from_secs(config.timeout_secs),
            ..Default::default()
        };

        let parser_config = PharmaParserConfig {
            client: client_config,
            ..Default::default()
        };
        let parser = Arc::new(PharmaParser::new(parser_config));

        // Process messages CONCURRENTLY for this model
        let semaphore = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
        let mut f_handles = Vec::new();

        for (msg_idx, msg) in messages.iter().enumerate() {
            let permit = Arc::clone(&semaphore).acquire_owned();
            let parser = Arc::clone(&parser);
            let content = msg.content.clone();
            let sender = msg.sender_name.clone();
            let group = msg.group_name.clone();
            let model_name = model.name.clone();
            let timeout_secs = config.timeout_secs;

            let handle = tokio::spawn(async move {
                let _permit = permit.await.unwrap();
                let start = Instant::now();
                let parse_fut = parser.parse(
                    &content,
                    sender.as_deref(),
                    group.as_deref().unwrap_or_default(),
                    None,
                    None,
                );

                let result =
                    tokio::time::timeout(Duration::from_secs(timeout_secs), parse_fut).await;
                let latency = start.elapsed().as_millis();

                match result {
                    Ok(Ok(items)) => ModelResult {
                        model_name,
                        items_count: items.len(),
                        latency_ms: latency,
                        success: true,
                    },
                    Ok(Err(_)) => ModelResult {
                        model_name,
                        items_count: 0,
                        latency_ms: latency,
                        success: false,
                    },
                    Err(_) => ModelResult {
                        model_name,
                        items_count: 0,
                        latency_ms: latency,
                        success: false,
                    },
                }
            });
            f_handles.push((msg_idx, handle));
        }

        // Wait for all messages for this model to finish
        for (msg_idx, handle) in f_handles {
            let res = handle.await?;
            all_message_results[msg_idx][model_idx] = res;

            // Simple progress indicator
            if (msg_idx + 1) % 10 == 0 || msg_idx + 1 == messages.len() {
                print!(
                    "{} ",
                    format!("(Ok:{}/{})", msg_idx + 1, messages.len()).dimmed()
                );
            }
        }
        println!("{}", "\n   ✅ Model complete.".green());
    }

    // ========================================================================
    // Per-Message Comparison (Only for small counts)
    // ========================================================================
    if messages.len() <= 5 {
        for (msg_idx, msg) in messages.iter().enumerate() {
            println!(
                "\n{}",
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                    .dimmed()
            );
            println!(
                "📝 {}: \"{}\"",
                format!("Message {}/{}", msg_idx + 1, messages.len())
                    .cyan()
                    .bold(),
                truncate(&msg.content, 100)
            );

            let mut msg_table = Table::new();
            msg_table.set_format(*FORMAT_BOX_CHARS);
            msg_table.add_row(row!["Model", "Items", "Latency", "Status"]);

            for (model_idx, _) in filtered_models.iter().enumerate() {
                let r = &all_message_results[msg_idx][model_idx];
                let status = if r.success {
                    "SUCCESS".green()
                } else {
                    "FAILED".red()
                };
                msg_table.add_row(row![
                    r.model_name,
                    r.items_count.to_string().yellow(),
                    format!("{}ms", r.latency_ms).dimmed(),
                    status
                ]);
            }
            msg_table.printstd();
        }
    }

    // ========================================================================
    // Final Summary
    // ========================================================================
    println!("\n╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                         📊 FINAL SUMMARY                                     ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝\n");

    // Calculate stats per model
    let mut model_stats: Vec<ModelStats> = Vec::new();

    for (model_idx, model) in filtered_models.iter().enumerate() {
        let model_results: Vec<&ModelResult> = all_message_results
            .iter()
            .map(|row| &row[model_idx])
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
            model_name: model.name.clone(),
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
