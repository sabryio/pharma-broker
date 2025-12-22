//! # Hybrid RAG Filtering Benchmark
//!
//! Benchmark tool for measuring hybrid RAG filtering performance impact.
//! Ported from legacy/cmd/playground/benchmark/main.go
//!
//! ## Usage
//! ```bash
//! cargo run --bin bench-hybrid-rag
//! ```

use std::env;
use std::time::{Duration, Instant};

use colored::Colorize;
use pharma_core::ai::{PharmaParser, PharmaParserConfig};
use pharma_core::domain::{MedicationMapping, RawMessage};
use pharma_core::repository::{
    MedicationMappingRepository, SeaOrmMedicationMappingRepo, create_connection,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Configuration
// ============================================================================

const ITERATIONS: usize = 3;

/// Test messages (same as Go benchmark)
fn get_test_messages() -> Vec<RawMessage> {
    vec![
        RawMessage {
            id: "bench-1".to_string(),
            content: "*عندي*\n*زولادكس 3.6*\n*سكسندا*\n*اوزمبك*".to_string(),
            ..Default::default()
        },
        RawMessage {
            id: "bench-2".to_string(),
            content: "*محتاج*\n*ديكابيبتيل*\n*فوستيمون*".to_string(),
            ..Default::default()
        },
        RawMessage {
            id: "bench-3".to_string(),
            content: "*متوفر*\n*مريوفيرت*\n*سيتروتايد*\n*اوفتريل*".to_string(),
            ..Default::default()
        },
    ]
}

// ============================================================================
// Benchmark Runner
// ============================================================================

struct BenchmarkResult {
    avg_duration: Duration,
    avg_items: f64,
    estimated_tokens: usize,
}

async fn run_benchmark(
    parser: &PharmaParser,
    messages: &[RawMessage],
    mappings: Option<&[String]>,
    use_hybrid: bool,
) -> BenchmarkResult {
    let mut total_duration = Duration::ZERO;
    let mut total_items = 0usize;
    let mut successful_iterations = 0usize;

    for i in 0..ITERATIONS {
        let start = Instant::now();
        let mut item_count = 0;
        let mut error: Option<String> = None;

        for msg in messages {
            match parser.parse(&msg.content, None, "", None, mappings).await {
                Ok(items) => {
                    item_count += items.len();
                }
                Err(e) => {
                    error = Some(format!("{}", e));
                    break;
                }
            }
        }

        let duration = start.elapsed();

        if let Some(err) = error {
            println!("  Iteration {}: {} - {}", i + 1, "ERROR".red(), err);
        } else {
            println!(
                "  Iteration {}: {:?} ({} items parsed)",
                i + 1,
                duration,
                item_count
            );
            total_duration += duration;
            total_items += item_count;
            successful_iterations += 1;
        }
    }

    let avg_duration = if successful_iterations > 0 {
        total_duration / successful_iterations as u32
    } else {
        Duration::ZERO
    };
    let avg_items = if successful_iterations > 0 {
        total_items as f64 / successful_iterations as f64
    } else {
        0.0
    };

    let estimated_tokens = estimate_prompt_tokens(messages, mappings, use_hybrid);

    BenchmarkResult {
        avg_duration,
        avg_items,
        estimated_tokens,
    }
}

/// Estimate prompt tokens (matching Go implementation)
/// Rough estimate: ~4 chars per token
fn estimate_prompt_tokens(
    messages: &[RawMessage],
    mappings: Option<&[String]>,
    use_hybrid: bool,
) -> usize {
    let mut text_len = 0;

    for msg in messages {
        text_len += msg.content.len();
    }

    if use_hybrid {
        // Filtered mappings would be much smaller
        // Estimate: ~10 mappings per message on average
        let mapping_count = mappings.map(|m| m.len()).unwrap_or(0);
        let filtered_count = mapping_count.min(messages.len() * 10);
        text_len += filtered_count * 40; // Average Arabic+English per mapping
    } else {
        // Full mappings
        let mapping_count = mappings.map(|m| m.len()).unwrap_or(0);
        text_len += mapping_count * 40;
    }

    text_len / 4
}

/// Filter mappings relevant to the messages (hybrid RAG simulation)
fn filter_relevant_mappings(
    messages: &[RawMessage],
    all_mappings: &[MedicationMapping],
) -> Vec<String> {
    // Simple keyword-based filtering (similar to Go's hybrid filtering)
    // In production, this would use embeddings/semantic search
    let mut relevant = Vec::new();
    let message_text: String = messages.iter().map(|m| m.content.as_str()).collect();

    for mapping in all_mappings {
        // Check if Arabic name appears in any message
        if message_text.contains(&mapping.arabic_name) {
            relevant.push(mapping.to_prompt_context());
        }
    }

    // If no exact matches, include top N by similarity (fallback)
    if relevant.is_empty() {
        relevant = all_mappings
            .iter()
            .take(messages.len() * 10)
            .map(|m| m.to_prompt_context())
            .collect();
    }

    relevant
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

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              🧪 HYBRID RAG FILTERING BENCHMARK                               ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "║              Measuring performance impact of filtered vs full mappings       ║".green()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".green()
    );
    println!();

    // Load environment
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!("🗄️  Connecting to database...");
    let db = create_connection(&database_url).await?;

    // Load all medication mappings
    let repo = SeaOrmMedicationMappingRepo::new(db);
    let count = repo.count().await?;
    println!(
        "📊 Total mappings in database: {}",
        count.to_string().yellow()
    );

    // Fetch all mappings (paginated)
    let mut all_mappings: Vec<MedicationMapping> = Vec::new();
    let page_size = 1000i64;
    let mut offset = 0i64;
    loop {
        let page = repo.get_all(page_size, offset).await?;
        if page.is_empty() {
            break;
        }
        all_mappings.extend(page);
        offset += page_size;
    }

    // Create full mappings list for prompts
    let full_mappings: Vec<String> = all_mappings.iter().map(|m| m.to_prompt_context()).collect();

    println!("   Loaded {} mappings", all_mappings.len());

    // Create AI parser
    let ai_url = env::var("AI_BASE_URL")
        .or_else(|_| env::var("LLM_URL"))
        .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string());

    println!("🔗 AI Backend: {}", ai_url.cyan());

    let mut parser_config = PharmaParserConfig::from_env();
    parser_config.client.base_url = ai_url;
    let parser = PharmaParser::new(parser_config);

    // Test messages
    let test_messages = get_test_messages();
    println!("📝 Test messages: {}", test_messages.len());
    println!();

    // =========================================================================
    // Test 1: WITHOUT Hybrid Filtering (full mappings)
    // =========================================================================
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "{}",
        "--- Test 1: WITHOUT Hybrid Filtering ---".yellow().bold()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    let result_without = run_benchmark(&parser, &test_messages, Some(&full_mappings), false).await;

    println!();
    println!(
        "  Average: {:?} ({:.1} items per run)",
        result_without.avg_duration, result_without.avg_items
    );
    println!(
        "  Estimated prompt tokens: ~{}",
        result_without.estimated_tokens
    );

    // =========================================================================
    // Test 2: WITH Hybrid Filtering (filtered mappings)
    // =========================================================================
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!("{}", "--- Test 2: WITH Hybrid Filtering ---".green().bold());
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    // Filter mappings relevant to test messages
    let filtered_mappings = filter_relevant_mappings(&test_messages, &all_mappings);
    println!(
        "  Filtered to {} relevant mappings",
        filtered_mappings.len().to_string().green()
    );

    let result_with = run_benchmark(&parser, &test_messages, Some(&filtered_mappings), true).await;

    println!();
    println!(
        "  Average: {:?} ({:.1} items per run)",
        result_with.avg_duration, result_with.avg_items
    );
    println!(
        "  Estimated prompt tokens: ~{}",
        result_with.estimated_tokens
    );

    // =========================================================================
    // Summary
    // =========================================================================
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║                              📊 SUMMARY                                      ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );

    let speedup = if result_with.avg_duration.as_millis() > 0 {
        result_without.avg_duration.as_millis() as f64 / result_with.avg_duration.as_millis() as f64
    } else {
        0.0
    };

    let token_reduction = if result_without.estimated_tokens > 0 {
        100.0
            - (result_with.estimated_tokens as f64 / result_without.estimated_tokens as f64 * 100.0)
    } else {
        0.0
    };

    println!();
    println!(
        "  Without Hybrid: {:?} avg, ~{} tokens",
        result_without.avg_duration, result_without.estimated_tokens
    );
    println!(
        "  With Hybrid:    {:?} avg, ~{} tokens",
        result_with.avg_duration, result_with.estimated_tokens
    );
    println!();
    println!("  Speedup:        {:.2}x", speedup);
    println!("  Token Reduction: {:.1}%", token_reduction);
    println!();

    // Circuit breaker state
    println!("🔌 Circuit Breaker: {:?}", parser.circuit_state());
    println!();
    println!("✅ Benchmark complete.");

    Ok(())
}
