//! # Medication Embedding Refresh Tool
//!
//! Tool to seed medications from JSON and refresh/regenerate embeddings.
//! Ported from legacy/cmd/tools/refresh_embeddings/main.go
//!
//! ## Features
//! - Delta updates: Only processes records where embedding is NULL
//! - Batching: Efficient batch API calls to reduce latency
//! - Rate limiting: Respects API rate limits with exponential backoff
//! - Token limit handling: Truncates content exceeding model limits
//!
//! ## Usage
//! ```bash
//! cargo run --bin refresh-embeddings              # Delta update (NULL embeddings only)
//! cargo run --bin refresh-embeddings -- --seed    # Seed from medications.json first
//! cargo run --bin refresh-embeddings -- --force   # Force regenerate ALL embeddings
//! cargo run --bin refresh-embeddings -- --dry-run # Show what would be processed
//! ```

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use colored::Colorize;
use pharma_core::ai::{PharmaParser, PharmaParserConfig};
use pharma_core::domain::MedicationMapping;
use pharma_core::repository::{
    MedicationMappingRepository, SeaOrmMedicationMappingRepo, create_connection,
};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Configuration
// ============================================================================

/// Batch size for embedding API calls
const BATCH_SIZE: usize = 10;
/// Number of concurrent batch requests
const CONCURRENCY: usize = 4;
/// Maximum retries for rate-limited requests
const MAX_RETRIES: usize = 3;
/// Initial backoff delay for retries (doubles each retry)
const INITIAL_BACKOFF_MS: u64 = 1000;
/// Maximum token length for embedding input (approximate)
const MAX_TOKEN_LENGTH: usize = 8000;
/// Path to medications JSON file
const MEDICATIONS_JSON: &str = "medications.json";

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
// Error Types
// ============================================================================

#[derive(Debug)]
enum EmbeddingError {
    RateLimited,
    TokenLimitExceeded,
    CircuitOpen,
    Other(String),
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingError::RateLimited => write!(f, "Rate limited"),
            EmbeddingError::TokenLimitExceeded => write!(f, "Token limit exceeded"),
            EmbeddingError::CircuitOpen => write!(f, "Circuit breaker open"),
            EmbeddingError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Truncate text to approximate token limit
fn truncate_to_token_limit(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        // Truncate at word boundary if possible
        let truncated = &text[..max_chars];
        if let Some(last_space) = truncated.rfind(' ') {
            truncated[..last_space].to_string()
        } else {
            truncated.to_string()
        }
    }
}

/// Classify error type from error message
fn classify_error(error: &str) -> EmbeddingError {
    let error_lower = error.to_lowercase();
    if error_lower.contains("rate limit") || error_lower.contains("429") {
        EmbeddingError::RateLimited
    } else if error_lower.contains("token") && error_lower.contains("limit") {
        EmbeddingError::TokenLimitExceeded
    } else if error_lower.contains("circuit") {
        EmbeddingError::CircuitOpen
    } else {
        EmbeddingError::Other(error.to_string())
    }
}

// ============================================================================
// Seed from JSON
// ============================================================================

async fn seed_from_json(repo: &SeaOrmMedicationMappingRepo) -> anyhow::Result<usize> {
    // Try multiple paths for medications.json
    let paths = [
        MEDICATIONS_JSON,
        "core/medications.json",
        "../medications.json",
    ];

    let mut json_path = None;
    for path in &paths {
        if Path::new(path).exists() {
            json_path = Some(*path);
            break;
        }
    }

    let json_path = json_path
        .ok_or_else(|| anyhow::anyhow!("medications.json not found. Tried: {:?}", paths))?;

    println!("  Reading from: {}", json_path.cyan());

    let content = std::fs::read_to_string(json_path)?;
    let medications: HashMap<String, MedicationEntry> = serde_json::from_str(&content)?;

    println!(
        "  Found {} medications in JSON",
        medications.len().to_string().yellow()
    );

    let mut created = 0usize;
    let mut skipped = 0usize;

    for (arabic_name, entry) in medications {
        // Check if already exists by searching
        let existing = repo.find_relevant(&arabic_name, 1).await?;
        let already_exists = existing.iter().any(|m| m.arabic_name == arabic_name);

        if already_exists {
            skipped += 1;
            continue;
        }

        let mut mapping = MedicationMapping::new(&arabic_name, &entry.english);
        if !entry.synonyms.is_empty() {
            mapping.synonyms = Some(entry.synonyms);
        }

        if let Err(e) = repo.save(&mapping).await {
            eprintln!("    {} Failed to save {}: {}", "⚠".yellow(), arabic_name, e);
        } else {
            created += 1;
        }
    }

    println!(
        "  {} Created {} new mappings, skipped {} existing",
        "✓".green(),
        created.to_string().green(),
        skipped
    );

    Ok(created)
}

// ============================================================================
// Embedding Generation with Retry
// ============================================================================

async fn generate_embeddings_with_retry(
    parser: &PharmaParser,
    texts: &[String],
    max_retries: usize,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    let mut attempt = 0;
    let mut backoff = Duration::from_millis(INITIAL_BACKOFF_MS);

    loop {
        match parser.embed_batch(texts).await {
            Ok(embeddings) => return Ok(embeddings),
            Err(e) => {
                let error_str = e.to_string();
                let error_type = classify_error(&error_str);

                match error_type {
                    EmbeddingError::RateLimited => {
                        attempt += 1;
                        if attempt >= max_retries {
                            return Err(EmbeddingError::RateLimited);
                        }
                        eprintln!(
                            "    {} Rate limited, retrying in {:?} (attempt {}/{})",
                            "⏳".yellow(),
                            backoff,
                            attempt,
                            max_retries
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2; // Exponential backoff
                    }
                    EmbeddingError::CircuitOpen => {
                        // Wait for circuit to recover
                        attempt += 1;
                        if attempt >= max_retries {
                            return Err(EmbeddingError::CircuitOpen);
                        }
                        eprintln!(
                            "    {} Circuit open, waiting {:?} (attempt {}/{})",
                            "🔌".yellow(),
                            backoff,
                            attempt,
                            max_retries
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                    }
                    _ => return Err(error_type),
                }
            }
        }
    }
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

    // Check for flags
    let args: Vec<String> = env::args().collect();
    let should_seed = args.iter().any(|a| a == "--seed" || a == "-s");
    let force_refresh = args.iter().any(|a| a == "--force" || a == "-f");
    let dry_run = args.iter().any(|a| a == "--dry-run" || a == "-n");

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              🔄 MEDICATION EMBEDDING REFRESH TOOL                            ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "║              Delta updates with rate limiting & retry                        ║".green()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".green()
    );
    println!();

    if dry_run {
        println!(
            "{}",
            "🔍 DRY RUN MODE - No changes will be made".yellow().bold()
        );
        println!();
    }

    // Load environment
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/pharmabroker".to_string());

    println!("🗄️  Connecting to database...");
    let db = create_connection(&database_url).await?;
    let repo = SeaOrmMedicationMappingRepo::new(db);

    // Seed from JSON if requested or if database is empty
    let initial_count = repo.count().await?;

    if should_seed || initial_count == 0 {
        println!();
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                .dimmed()
        );
        if initial_count == 0 {
            println!(
                "{}",
                "📦 Database empty - seeding from medications.json..."
                    .yellow()
                    .bold()
            );
        } else {
            println!(
                "{}",
                "📦 Seeding from medications.json (--seed flag)..."
                    .yellow()
                    .bold()
            );
        }
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                .dimmed()
        );

        if !dry_run {
            seed_from_json(&repo).await?;
        } else {
            println!("  {} Would seed from medications.json", "→".cyan());
        }
    }

    // Get statistics
    let total_count = repo.count().await?;
    let needs_embedding_count = repo.count_needing_embeddings().await?;
    let has_embedding_count = total_count - needs_embedding_count;

    println!();
    println!(
        "📊 Total medication mappings: {}",
        total_count.to_string().yellow()
    );
    println!(
        "  ✓ With embeddings: {}",
        has_embedding_count.to_string().green()
    );
    println!(
        "  ○ Need embeddings: {}",
        needs_embedding_count.to_string().yellow()
    );

    // Determine what to process
    let mappings_to_process: Vec<MedicationMapping> = if force_refresh {
        println!(
            "  {} Force refresh enabled - regenerating ALL embeddings",
            "⚡".yellow()
        );
        // Load all mappings for force refresh
        let mut all_mappings = Vec::new();
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
        all_mappings
    } else {
        // Delta update: only get mappings without embeddings
        let mut mappings = Vec::new();
        let page_size = 1000i64;
        loop {
            let page = repo.get_needing_embeddings(page_size).await?;
            if page.is_empty() {
                break;
            }
            let page_len = page.len();
            mappings.extend(page);
            if (page_len as i64) < page_size {
                break;
            }
        }
        mappings
    };

    let process_count = mappings_to_process.len();

    if process_count == 0 {
        println!();
        println!("{} All mappings already have embeddings!", "✓".green());
        return Ok(());
    }

    println!();
    println!(
        "🎯 Will process {} mappings",
        process_count.to_string().yellow()
    );

    if dry_run {
        println!();
        println!(
            "{}",
            "Dry run complete. Use without --dry-run to execute.".cyan()
        );
        return Ok(());
    }

    // Create AI parser for embeddings
    let ai_url = env::var("AI_BASE_URL")
        .or_else(|_| env::var("LLM_URL"))
        .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string());

    println!("🔗 AI Backend: {}", ai_url.cyan());
    println!(
        "⚙️  Config: batch_size={}, concurrency={}, max_retries={}",
        BATCH_SIZE.to_string().cyan(),
        CONCURRENCY.to_string().cyan(),
        MAX_RETRIES.to_string().cyan()
    );

    let mut parser_config = PharmaParserConfig::from_env();
    parser_config.client.base_url = ai_url;
    let parser = Arc::new(PharmaParser::new(parser_config));
    let repo = Arc::new(repo);

    // Prepare texts for embedding (with truncation for token limits)
    let texts_and_indices: Vec<(String, usize)> = mappings_to_process
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let text = truncate_to_token_limit(&m.arabic_name, MAX_TOKEN_LENGTH);
            (text, i)
        })
        .collect();

    println!();
    println!(
        "Generating embeddings for {} medications ({} concurrent batches)...",
        process_count.to_string().yellow(),
        CONCURRENCY.to_string().cyan()
    );

    let start = Instant::now();
    let generated_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let rate_limit_count = Arc::new(AtomicUsize::new(0));
    let mappings_to_process = Arc::new(Mutex::new(mappings_to_process));

    // Create batches
    let batches: Vec<(usize, Vec<String>, Vec<usize>)> = texts_and_indices
        .chunks(BATCH_SIZE)
        .enumerate()
        .map(|(batch_idx, chunk)| {
            let texts: Vec<String> = chunk.iter().map(|(t, _)| t.clone()).collect();
            let indices: Vec<usize> = chunk.iter().map(|(_, i)| *i).collect();
            (batch_idx * BATCH_SIZE, texts, indices)
        })
        .collect();

    let total_batches = batches.len();
    let completed_batches = Arc::new(AtomicUsize::new(0));

    // Process batches concurrently with semaphore
    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONCURRENCY));
    let mut handles = Vec::new();

    for (batch_start, batch_texts, indices) in batches {
        let parser = Arc::clone(&parser);
        let repo = Arc::clone(&repo);
        let mappings = Arc::clone(&mappings_to_process);
        let generated_count = Arc::clone(&generated_count);
        let error_count = Arc::clone(&error_count);
        let rate_limit_count = Arc::clone(&rate_limit_count);
        let completed_batches = Arc::clone(&completed_batches);
        let semaphore = Arc::clone(&semaphore);
        let batch_end = batch_start + batch_texts.len();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            match generate_embeddings_with_retry(&parser, &batch_texts, MAX_RETRIES).await {
                Ok(embeddings) => {
                    // Process each embedding
                    for (idx, emb) in indices.iter().zip(embeddings.into_iter()) {
                        // Update mapping with embedding
                        let mut mapping = {
                            let m = mappings.lock().await;
                            m[*idx].clone()
                        };
                        mapping.set_embedding(emb);

                        // Save to database
                        if let Err(e) = repo.save(&mapping).await {
                            eprintln!(
                                "    {} Failed to save {}: {}",
                                "⚠".yellow(),
                                mapping.arabic_name,
                                e
                            );
                            error_count.fetch_add(1, Ordering::Relaxed);
                        } else {
                            generated_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(EmbeddingError::RateLimited) => {
                    eprintln!(
                        "    {} Batch {}-{} failed: Rate limit exceeded after retries",
                        "✗".red(),
                        batch_start + 1,
                        batch_end
                    );
                    rate_limit_count.fetch_add(batch_texts.len(), Ordering::Relaxed);
                    error_count.fetch_add(batch_texts.len(), Ordering::Relaxed);
                }
                Err(EmbeddingError::TokenLimitExceeded) => {
                    eprintln!(
                        "    {} Batch {}-{} failed: Token limit exceeded",
                        "✗".red(),
                        batch_start + 1,
                        batch_end
                    );
                    error_count.fetch_add(batch_texts.len(), Ordering::Relaxed);
                }
                Err(e) => {
                    eprintln!(
                        "    {} Batch {}-{} failed: {}",
                        "✗".red(),
                        batch_start + 1,
                        batch_end,
                        e
                    );
                    error_count.fetch_add(batch_texts.len(), Ordering::Relaxed);
                }
            }

            let completed = completed_batches.fetch_add(1, Ordering::Relaxed) + 1;
            let gen_ = generated_count.load(Ordering::Relaxed);
            let err = error_count.load(Ordering::Relaxed);
            println!(
                "  {} Batch {}/{} complete (generated: {}, errors: {})",
                "✓".green(),
                completed,
                total_batches,
                gen_.to_string().green(),
                if err > 0 {
                    err.to_string().red()
                } else {
                    err.to_string().dimmed()
                }
            );
        });

        handles.push(handle);
    }

    // Wait for all batches to complete
    for handle in handles {
        handle.await?;
    }

    let duration = start.elapsed();
    let final_generated = generated_count.load(Ordering::Relaxed);
    let final_errors = error_count.load(Ordering::Relaxed);
    let final_rate_limits = rate_limit_count.load(Ordering::Relaxed);

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "{} Generated {} embeddings in {:?}",
        "✓".green(),
        final_generated.to_string().green(),
        duration
    );

    if final_errors > 0 {
        println!(
            "{} {} errors occurred",
            "⚠".yellow(),
            final_errors.to_string().red()
        );
        if final_rate_limits > 0 {
            println!(
                "  └─ {} due to rate limiting (consider reducing concurrency)",
                final_rate_limits.to_string().yellow()
            );
        }
    }

    // Final statistics
    let new_needs_embedding = repo.count_needing_embeddings().await?;
    println!();
    println!("📊 Final Status:");
    println!(
        "  ✓ With embeddings: {}",
        (total_count - new_needs_embedding).to_string().green()
    );
    println!(
        "  ○ Still need embeddings: {}",
        new_needs_embedding.to_string().yellow()
    );

    println!();
    println!("🔌 Circuit Breaker: {:?}", parser.circuit_state());
    println!();
    println!("✅ Embedding refresh complete.");

    Ok(())
}
