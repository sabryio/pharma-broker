//! # AI Model Quality Analyzer
//!
//! Deep analysis of AI parsing quality across multiple LLM models.
//! Provides comprehensive metrics on extraction completeness, confidence,
//! and field-level quality.
//!
//! ## Usage
//! ```bash
//! cargo run --bin analyze-models
//! ```

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};
use prettytable::format::consts::FORMAT_BOX_CHARS;
use prettytable::{Table, row};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::{Intent, ParsedItem, PharmaParser, PharmaParserConfig};

// ============================================================================
// Configuration
// ============================================================================

struct Config {
    limit: i64,
    concurrency: usize,
    timeout_secs: u64,
    database_url: String,
    selected_models: Vec<String>,
    export_json: bool,
}

fn get_config_interactive() -> Config {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║              🔬 AI MODEL QUALITY ANALYZER                                    ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Deep Analysis of Parsing Quality                                ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();

    let theme = ColorfulTheme::default();

    let model_options = vec!["All Models", "Qwen3-VL", "Ministral3", "Gemma3"];
    let model_selection = dialoguer::Select::with_theme(&theme)
        .with_prompt("🤖 Select model(s) to analyze")
        .items(&model_options)
        .default(0)
        .interact()
        .unwrap_or(0);

    let selected_models: Vec<String> = if model_selection == 0 {
        vec![]
    } else {
        vec![model_options[model_selection].to_string()]
    };

    let limit: i64 = Input::with_theme(&theme)
        .with_prompt("📝 Number of messages to analyze")
        .default(100)
        .interact_text()
        .unwrap_or(100);

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

    let export_json = dialoguer::Confirm::with_theme(&theme)
        .with_prompt("💾 Export detailed results to JSON?")
        .default(true)
        .interact()
        .unwrap_or(true);

    let default_db = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    let database_url: String = Input::with_theme(&theme)
        .with_prompt("🗄️  Database URL")
        .default(default_db)
        .interact_text()
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!();
    print_separator();
    println!(
        "  {} {} models | {} msgs | {} concurrent | {}s timeout | JSON: {}",
        "Config:".green().bold(),
        if selected_models.is_empty() {
            "All".to_string()
        } else {
            selected_models.join(", ")
        }
        .yellow(),
        limit.to_string().yellow(),
        concurrency.to_string().yellow(),
        timeout_secs.to_string().yellow(),
        if export_json {
            "Yes".green()
        } else {
            "No".red()
        }
    );
    print_separator();
    println!();

    Config {
        limit,
        concurrency,
        timeout_secs,
        database_url,
        selected_models,
        export_json,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModelResult {
    model_name: String,
    message_id: String,
    message_content: String,
    items_count: usize,
    latency_ms: u128,
    success: bool,
    error: Option<String>,
    items: Vec<ParsedItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FieldStats {
    total: usize,
    has_value: usize,
    percentage: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConfidenceStats {
    min: f64,
    max: f64,
    avg: f64,
    median: f64,
    std_dev: f64,
    distribution: HashMap<String, usize>, // "0.0-0.2", "0.2-0.4", etc.
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct QualityMetrics {
    model_name: String,

    // Basic stats
    total_messages: usize,
    successful_parses: usize,
    failed_parses: usize,
    success_rate: f64,

    // Item stats
    total_items: usize,
    offers_count: usize,
    requests_count: usize,
    avg_items_per_message: f64,

    // Field completeness
    medication_completeness: FieldStats,
    quantity_completeness: FieldStats,
    price_completeness: FieldStats,
    unit_completeness: FieldStats,

    // Confidence analysis
    confidence_stats: ConfidenceStats,

    // Latency stats
    avg_latency_ms: u128,
    min_latency_ms: u128,
    max_latency_ms: u128,
    p50_latency_ms: u128,
    p95_latency_ms: u128,
    p99_latency_ms: u128,

    // Quality score (0-100)
    quality_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisReport {
    timestamp: String,
    total_messages: usize,
    models_analyzed: Vec<String>,
    metrics: Vec<QualityMetrics>,
    detailed_results: Vec<ModelResult>,
    winner: Option<String>,
    recommendations: Vec<String>,
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
// Helpers
// ============================================================================

fn print_separator() {
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
}

fn print_header(title: &str) {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!("║ {:^76} ║", title.cyan().bold());
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );
}

fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() <= max_len {
        s
    } else {
        format!("{}...", &s[..max_len])
    }
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn std_deviation(values: &[f64], mean: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}

fn confidence_bucket(conf: f64) -> String {
    match conf {
        c if c < 0.2 => "0.0-0.2".to_string(),
        c if c < 0.4 => "0.2-0.4".to_string(),
        c if c < 0.6 => "0.4-0.6".to_string(),
        c if c < 0.8 => "0.6-0.8".to_string(),
        _ => "0.8-1.0".to_string(),
    }
}

fn quality_indicator(pct: f64) -> colored::ColoredString {
    match pct {
        p if p >= 90.0 => "██████████".green(),
        p if p >= 80.0 => "████████░░".green(),
        p if p >= 70.0 => "███████░░░".yellow(),
        p if p >= 60.0 => "██████░░░░".yellow(),
        p if p >= 50.0 => "█████░░░░░".yellow(),
        p if p >= 40.0 => "████░░░░░░".red(),
        p if p >= 30.0 => "███░░░░░░░".red(),
        p if p >= 20.0 => "██░░░░░░░░".red(),
        p if p >= 10.0 => "█░░░░░░░░░".red(),
        _ => "░░░░░░░░░░".red(),
    }
}

// ============================================================================
// Analysis Functions
// ============================================================================

fn calculate_quality_metrics(model_name: &str, results: &[ModelResult]) -> QualityMetrics {
    let successful: Vec<&ModelResult> = results.iter().filter(|r| r.success).collect();
    let all_items: Vec<&ParsedItem> = successful.iter().flat_map(|r| &r.items).collect();

    // Basic stats
    let total_messages = results.len();
    let successful_parses = successful.len();
    let failed_parses = total_messages - successful_parses;
    let success_rate = if total_messages > 0 {
        (successful_parses as f64 / total_messages as f64) * 100.0
    } else {
        0.0
    };

    // Item stats
    let total_items = all_items.len();
    let offers_count = all_items
        .iter()
        .filter(|i| i.item_type == Intent::Offer)
        .count();
    let requests_count = all_items
        .iter()
        .filter(|i| i.item_type == Intent::Request)
        .count();
    let avg_items_per_message = if successful_parses > 0 {
        total_items as f64 / successful_parses as f64
    } else {
        0.0
    };

    // Field completeness
    let medication_has = all_items
        .iter()
        .filter(|i| !i.medication.is_empty())
        .count();
    let quantity_has = all_items.iter().filter(|i| i.quantity > 0.0).count();
    let price_has = all_items
        .iter()
        .filter(|i| i.price > 0.0 || i.max_price > 0.0)
        .count();
    let unit_has = all_items.iter().filter(|i| i.unit.is_some()).count();

    let medication_completeness = FieldStats {
        total: total_items,
        has_value: medication_has,
        percentage: if total_items > 0 {
            (medication_has as f64 / total_items as f64) * 100.0
        } else {
            0.0
        },
    };
    let quantity_completeness = FieldStats {
        total: total_items,
        has_value: quantity_has,
        percentage: if total_items > 0 {
            (quantity_has as f64 / total_items as f64) * 100.0
        } else {
            0.0
        },
    };
    let price_completeness = FieldStats {
        total: total_items,
        has_value: price_has,
        percentage: if total_items > 0 {
            (price_has as f64 / total_items as f64) * 100.0
        } else {
            0.0
        },
    };
    let unit_completeness = FieldStats {
        total: total_items,
        has_value: unit_has,
        percentage: if total_items > 0 {
            (unit_has as f64 / total_items as f64) * 100.0
        } else {
            0.0
        },
    };

    // Confidence analysis
    let confidences: Vec<f64> = all_items.iter().map(|i| i.ai_confidence).collect();
    let mut sorted_conf = confidences.clone();
    sorted_conf.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let conf_min = sorted_conf.first().copied().unwrap_or(0.0);
    let conf_max = sorted_conf.last().copied().unwrap_or(0.0);
    let conf_avg = if !confidences.is_empty() {
        confidences.iter().sum::<f64>() / confidences.len() as f64
    } else {
        0.0
    };
    let conf_median = if !sorted_conf.is_empty() {
        sorted_conf[sorted_conf.len() / 2]
    } else {
        0.0
    };
    let conf_std = std_deviation(&confidences, conf_avg);

    let mut distribution: HashMap<String, usize> = HashMap::new();
    for &c in &confidences {
        *distribution.entry(confidence_bucket(c)).or_insert(0) += 1;
    }

    let confidence_stats = ConfidenceStats {
        min: conf_min,
        max: conf_max,
        avg: conf_avg,
        median: conf_median,
        std_dev: conf_std,
        distribution,
    };

    // Latency stats
    let mut latencies: Vec<u128> = results.iter().map(|r| r.latency_ms).collect();
    latencies.sort();

    let avg_latency_ms = if !latencies.is_empty() {
        latencies.iter().sum::<u128>() / latencies.len() as u128
    } else {
        0
    };
    let min_latency_ms = latencies.first().copied().unwrap_or(0);
    let max_latency_ms = latencies.last().copied().unwrap_or(0);
    let p50_latency_ms = percentile(&latencies, 0.50);
    let p95_latency_ms = percentile(&latencies, 0.95);
    let p99_latency_ms = percentile(&latencies, 0.99);

    // Quality score calculation (weighted)
    let quality_score = calculate_quality_score(
        success_rate,
        medication_completeness.percentage,
        quantity_completeness.percentage,
        price_completeness.percentage,
        conf_avg,
        avg_latency_ms,
    );

    QualityMetrics {
        model_name: model_name.to_string(),
        total_messages,
        successful_parses,
        failed_parses,
        success_rate,
        total_items,
        offers_count,
        requests_count,
        avg_items_per_message,
        medication_completeness,
        quantity_completeness,
        price_completeness,
        unit_completeness,
        confidence_stats,
        avg_latency_ms,
        min_latency_ms,
        max_latency_ms,
        p50_latency_ms,
        p95_latency_ms,
        p99_latency_ms,
        quality_score,
    }
}

fn calculate_quality_score(
    success_rate: f64,
    med_pct: f64,
    qty_pct: f64,
    price_pct: f64,
    avg_confidence: f64,
    avg_latency: u128,
) -> f64 {
    // Weights for different factors
    let success_weight = 0.25;
    let medication_weight = 0.25;
    let quantity_weight = 0.15;
    let price_weight = 0.15;
    let confidence_weight = 0.15;
    let latency_weight = 0.05;

    // Normalize latency (lower is better, cap at 10s)
    let latency_score = ((10000.0 - avg_latency as f64).max(0.0) / 10000.0) * 100.0;

    success_rate * success_weight
        + med_pct * medication_weight
        + qty_pct * quantity_weight
        + price_pct * price_weight
        + (avg_confidence * 100.0) * confidence_weight
        + latency_score * latency_weight
}

// ============================================================================
// Display Functions
// ============================================================================

fn display_model_metrics(metrics: &QualityMetrics) {
    print_header(&format!("📊 {} Analysis", metrics.model_name));

    // Success rate section
    println!(
        "\n  {} {}",
        "🎯 SUCCESS RATE".white().bold(),
        "─".repeat(60).dimmed()
    );
    println!(
        "     Total Messages:    {}",
        metrics.total_messages.to_string().cyan()
    );
    println!(
        "     Successful:        {} {}",
        metrics.successful_parses.to_string().green(),
        format!("({:.1}%)", metrics.success_rate).dimmed()
    );
    println!(
        "     Failed:            {}",
        metrics.failed_parses.to_string().red()
    );
    println!(
        "     Success Bar:       {} {:.1}%",
        quality_indicator(metrics.success_rate),
        metrics.success_rate
    );

    // Items section
    println!(
        "\n  {} {}",
        "📦 ITEMS EXTRACTED".white().bold(),
        "─".repeat(56).dimmed()
    );
    println!(
        "     Total Items:       {}",
        metrics.total_items.to_string().cyan()
    );
    println!(
        "     Offers:            {} {}",
        metrics.offers_count.to_string().green(),
        format!(
            "({:.1}%)",
            if metrics.total_items > 0 {
                metrics.offers_count as f64 / metrics.total_items as f64 * 100.0
            } else {
                0.0
            }
        )
        .dimmed()
    );
    println!(
        "     Requests:          {} {}",
        metrics.requests_count.to_string().yellow(),
        format!(
            "({:.1}%)",
            if metrics.total_items > 0 {
                metrics.requests_count as f64 / metrics.total_items as f64 * 100.0
            } else {
                0.0
            }
        )
        .dimmed()
    );
    println!(
        "     Avg per Message:   {:.2}",
        metrics.avg_items_per_message
    );

    // Field completeness section
    println!(
        "\n  {} {}",
        "✅ FIELD COMPLETENESS".white().bold(),
        "─".repeat(53).dimmed()
    );

    let fields = [
        ("Medication", &metrics.medication_completeness),
        ("Quantity", &metrics.quantity_completeness),
        ("Price", &metrics.price_completeness),
        ("Unit", &metrics.unit_completeness),
    ];

    for (name, stats) in &fields {
        println!(
            "     {:15} {} {:>5}/{:<5} {:.1}%",
            name,
            quality_indicator(stats.percentage),
            stats.has_value,
            stats.total,
            stats.percentage
        );
    }

    // Confidence section
    println!(
        "\n  {} {}",
        "🎲 CONFIDENCE ANALYSIS".white().bold(),
        "─".repeat(52).dimmed()
    );
    println!(
        "     Average:           {:.3}",
        metrics.confidence_stats.avg
    );
    println!(
        "     Median:            {:.3}",
        metrics.confidence_stats.median
    );
    println!(
        "     Std Dev:           {:.3}",
        metrics.confidence_stats.std_dev
    );
    println!(
        "     Range:             {:.3} - {:.3}",
        metrics.confidence_stats.min, metrics.confidence_stats.max
    );

    // Confidence distribution
    println!("\n     Distribution:");
    let buckets = ["0.0-0.2", "0.2-0.4", "0.4-0.6", "0.6-0.8", "0.8-1.0"];
    for bucket in buckets {
        let count = metrics
            .confidence_stats
            .distribution
            .get(bucket)
            .copied()
            .unwrap_or(0);
        let pct = if metrics.total_items > 0 {
            count as f64 / metrics.total_items as f64 * 100.0
        } else {
            0.0
        };
        let bar_len = (pct / 5.0).round() as usize;
        let bar = "█".repeat(bar_len.min(20));
        println!("       {} {:>4} {} {:.1}%", bucket, count, bar.cyan(), pct);
    }

    // Latency section
    println!(
        "\n  {} {}",
        "⚡ LATENCY (ms)".white().bold(),
        "─".repeat(59).dimmed()
    );
    println!("     Average:           {}ms", metrics.avg_latency_ms);
    println!("     Min:               {}ms", metrics.min_latency_ms);
    println!("     Max:               {}ms", metrics.max_latency_ms);
    println!("     P50:               {}ms", metrics.p50_latency_ms);
    println!("     P95:               {}ms", metrics.p95_latency_ms);
    println!("     P99:               {}ms", metrics.p99_latency_ms);

    // Quality score
    println!(
        "\n  {} {}",
        "🏆 QUALITY SCORE".white().bold(),
        "─".repeat(58).dimmed()
    );
    let score_color = match metrics.quality_score {
        s if s >= 80.0 => metrics.quality_score.to_string().green(),
        s if s >= 60.0 => metrics.quality_score.to_string().yellow(),
        _ => metrics.quality_score.to_string().red(),
    };
    println!("     Overall Score:     {}/100", score_color);
    println!(
        "     Quality Bar:       {}",
        quality_indicator(metrics.quality_score)
    );
}

fn display_comparison_table(all_metrics: &[QualityMetrics]) {
    print_header("📈 MODEL COMPARISON");

    let mut table = Table::new();
    table.set_format(*FORMAT_BOX_CHARS);
    table.add_row(row![
        "Model", "Success%", "Items", "Med%", "Qty%", "Price%", "Conf", "Latency", "Score"
    ]);

    for m in all_metrics {
        let score_str = format!("{:.1}", m.quality_score);
        let score_colored = match m.quality_score {
            s if s >= 80.0 => score_str.green(),
            s if s >= 60.0 => score_str.yellow(),
            _ => score_str.red(),
        };

        table.add_row(row![
            m.model_name,
            format!("{:.1}%", m.success_rate),
            m.total_items,
            format!("{:.1}%", m.medication_completeness.percentage),
            format!("{:.1}%", m.quantity_completeness.percentage),
            format!("{:.1}%", m.price_completeness.percentage),
            format!("{:.2}", m.confidence_stats.avg),
            format!("{}ms", m.avg_latency_ms),
            score_colored
        ]);
    }

    println!();
    table.printstd();
}

fn display_winners(all_metrics: &[QualityMetrics]) {
    print_header("🏆 WINNERS");

    if let Some(best) = all_metrics
        .iter()
        .max_by(|a, b| a.quality_score.partial_cmp(&b.quality_score).unwrap())
    {
        println!(
            "\n  🥇 {} {} (Score: {:.1}/100)",
            "Best Overall:".white().bold(),
            best.model_name.green().bold(),
            best.quality_score
        );
    }

    if let Some(fastest) = all_metrics
        .iter()
        .filter(|m| m.successful_parses > 0)
        .min_by_key(|m| m.avg_latency_ms)
    {
        println!(
            "  ⚡ {} {} ({}ms avg)",
            "Fastest:".white().bold(),
            fastest.model_name.cyan().bold(),
            fastest.avg_latency_ms
        );
    }

    if let Some(most_items) = all_metrics.iter().max_by_key(|m| m.total_items) {
        println!(
            "  📦 {} {} ({} items)",
            "Most Items:".white().bold(),
            most_items.model_name.yellow().bold(),
            most_items.total_items
        );
    }

    if let Some(highest_conf) = all_metrics.iter().max_by(|a, b| {
        a.confidence_stats
            .avg
            .partial_cmp(&b.confidence_stats.avg)
            .unwrap()
    }) {
        println!(
            "  🎯 {} {} ({:.2} avg)",
            "Highest Confidence:".white().bold(),
            highest_conf.model_name.magenta().bold(),
            highest_conf.confidence_stats.avg
        );
    }

    if let Some(best_med) = all_metrics.iter().max_by(|a, b| {
        a.medication_completeness
            .percentage
            .partial_cmp(&b.medication_completeness.percentage)
            .unwrap()
    }) {
        println!(
            "  💊 {} {} ({:.1}%)",
            "Best Medication Extraction:".white().bold(),
            best_med.model_name.blue().bold(),
            best_med.medication_completeness.percentage
        );
    }
}

fn generate_recommendations(all_metrics: &[QualityMetrics]) -> Vec<String> {
    let mut recommendations = Vec::new();

    for m in all_metrics {
        if m.success_rate < 90.0 {
            recommendations.push(format!("⚠️  {} has {:.1}% success rate - consider increasing timeout or checking model availability", m.model_name, m.success_rate));
        }
        if m.medication_completeness.percentage < 80.0 {
            recommendations.push(format!(
                "💊 {} extracts medication in only {:.1}% of items - may need prompt tuning",
                m.model_name, m.medication_completeness.percentage
            ));
        }
        if m.confidence_stats.avg < 0.7 {
            recommendations.push(format!(
                "🎲 {} has low average confidence ({:.2}) - outputs may be unreliable",
                m.model_name, m.confidence_stats.avg
            ));
        }
        if m.avg_latency_ms > 5000 {
            recommendations.push(format!(
                "⏱️  {} is slow ({}ms avg) - consider using a faster model for production",
                m.model_name, m.avg_latency_ms
            ));
        }
    }

    if recommendations.is_empty() {
        recommendations.push("✅ All models performing well!".to_string());
    }

    recommendations
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = get_config_interactive();
    let models = get_model_configs();

    // Filter models
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

    // Show model config
    let mut config_table = Table::new();
    config_table.set_format(*FORMAT_BOX_CHARS);
    config_table.add_row(row!["Model", "Endpoint", "Model ID"]);
    for m in &filtered_models {
        config_table.add_row(row![m.name, m.base_url, m.model]);
    }
    println!("📋 Model Configuration:");
    config_table.printstd();

    // Connect to database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    // Fetch test messages
    println!(
        "\n📥 Loading {} test messages from database...",
        config.limit
    );
    let messages: Vec<TestMessage> = sqlx::query_as!(
        TestMessage,
        r#"SELECT id, content, sender_name, group_name FROM raw_messages LIMIT $1"#,
        config.limit
    )
    .fetch_all(&pool)
    .await?;

    if messages.is_empty() {
        println!("❌ No messages found in database.");
        return Ok(());
    }

    println!("   ✅ Loaded {} messages\n", messages.len());

    // Storage for all results
    let mut all_results: HashMap<String, Vec<ModelResult>> = HashMap::new();

    // Process each model
    for (model_idx, model) in filtered_models.iter().enumerate() {
        println!(
            "\n🚀 Analyzing Model {}/{} [{}]",
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

        let semaphore = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
        let mut handles = Vec::new();

        for msg in messages.iter() {
            let permit = Arc::clone(&semaphore).acquire_owned();
            let parser = Arc::clone(&parser);
            let content = msg.content.clone();
            let sender = msg.sender_name.clone();
            let group = msg.group_name.clone();
            let model_name = model.name.clone();
            let msg_id = msg.id.clone();
            let msg_content = msg.content.clone();
            let timeout_secs = config.timeout_secs;

            let handle = tokio::spawn(async move {
                let _permit = permit.await.unwrap();
                let start = Instant::now();
                let parse_fut =
                    parser.parse(&content, sender.as_deref(), group.as_deref(), None, None);
                let result =
                    tokio::time::timeout(Duration::from_secs(timeout_secs), parse_fut).await;
                let latency = start.elapsed().as_millis();

                match result {
                    Ok(Ok(items)) => ModelResult {
                        model_name,
                        message_id: msg_id,
                        message_content: truncate(&msg_content, 200),
                        items_count: items.len(),
                        latency_ms: latency,
                        success: true,
                        error: None,
                        items,
                    },
                    Ok(Err(e)) => ModelResult {
                        model_name,
                        message_id: msg_id,
                        message_content: truncate(&msg_content, 200),
                        items_count: 0,
                        latency_ms: latency,
                        success: false,
                        error: Some(e.to_string()),
                        items: vec![],
                    },
                    Err(_) => ModelResult {
                        model_name,
                        message_id: msg_id,
                        message_content: truncate(&msg_content, 200),
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

        // Collect results with progress
        let mut model_results = Vec::new();
        for (idx, handle) in handles.into_iter().enumerate() {
            let res = handle.await?;
            model_results.push(res);

            if (idx + 1) % 10 == 0 || idx + 1 == messages.len() {
                print!("\r   Processing: {}/{} ", idx + 1, messages.len());
                std::io::stdout().flush().ok();
            }
        }
        println!("{}", "✅".green());

        all_results.insert(model.name.clone(), model_results);
    }

    // Calculate metrics for each model
    let mut all_metrics: Vec<QualityMetrics> = Vec::new();
    for model in &filtered_models {
        if let Some(results) = all_results.get(&model.name) {
            let metrics = calculate_quality_metrics(&model.name, results);
            all_metrics.push(metrics);
        }
    }

    // Display detailed metrics for each model
    for metrics in &all_metrics {
        display_model_metrics(metrics);
    }

    // Display comparison table
    if all_metrics.len() > 1 {
        display_comparison_table(&all_metrics);
    }

    // Display winners
    display_winners(&all_metrics);

    // Generate recommendations
    let recommendations = generate_recommendations(&all_metrics);
    print_header("💡 RECOMMENDATIONS");
    for rec in &recommendations {
        println!("  {}", rec);
    }

    // Export to JSON if requested
    if config.export_json {
        let winner = all_metrics
            .iter()
            .max_by(|a, b| a.quality_score.partial_cmp(&b.quality_score).unwrap())
            .map(|m| m.model_name.clone());

        let detailed_results: Vec<ModelResult> = all_results.values().flatten().cloned().collect();

        let report = AnalysisReport {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_messages: messages.len(),
            models_analyzed: filtered_models.iter().map(|m| m.name.clone()).collect(),
            metrics: all_metrics.clone(),
            detailed_results,
            winner,
            recommendations,
        };

        let json_path = "analysis_report.json";
        let mut file = File::create(json_path)?;
        file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
        println!("\n💾 Detailed report saved to: {}", json_path.cyan());
    }

    println!("\n{}", "✅ Analysis complete!".green().bold());

    Ok(())
}
