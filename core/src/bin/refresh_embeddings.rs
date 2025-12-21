//! # Medication Embedding Refresh Tool
//!
//! Tool to seed medications from JSON and refresh/regenerate embeddings.
//! Ported from legacy/cmd/tools/refresh_embeddings/main.go
//!
//! ## Usage
//! ```bash
//! cargo run --bin refresh-embeddings
//! cargo run --bin refresh-embeddings -- --seed   # Seed from medications.json
//! cargo run --bin refresh-embeddings -- --force  # Force regenerate all embeddings
//! ```

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use colored::Colorize;
use pharma_core::ai::{PharmaParser, PharmaParserConfig};
use pharma_core::domain::MedicationMapping;
use pharma_core::repository::MedicationMappingRepository;
use pharma_core::repository::postgres::PostgresMedicationMappingRepo;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Configuration
// ============================================================================

const BATCH_SIZE: usize = 10;
const CONCURRENCY: usize = 4; // Number of concurrent batch requests
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
// Seed from JSON
// ============================================================================

async fn seed_from_json(repo: &PostgresMedicationMappingRepo) -> anyhow::Result<usize> {
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
        "║              Seed from JSON and regenerate embeddings                        ║".green()
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
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let repo = PostgresMedicationMappingRepo::new(pool);

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

        seed_from_json(&repo).await?;
    }

    // Reload all medication mappings
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

    println!();
    println!(
        "📊 Found {} medication mappings",
        all_mappings.len().to_string().yellow()
    );

    // Create AI parser for embeddings
    let ai_url = env::var("AI_BASE_URL")
        .or_else(|_| env::var("LLM_URL"))
        .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string());

    println!("🔗 AI Backend: {}", ai_url.cyan());

    let mut parser_config = PharmaParserConfig::from_env();
    parser_config.client.base_url = ai_url;
    let parser = PharmaParser::new(parser_config);

    // Count existing embeddings
    let mut needs_embedding = 0usize;
    let mut has_embedding = 0usize;

    for m in &all_mappings {
        if force_refresh || m.get_embedding().is_none() {
            needs_embedding += 1;
        } else {
            has_embedding += 1;
        }
    }

    if force_refresh {
        println!(
            "  {} Force refresh enabled - regenerating ALL embeddings",
            "⚡".yellow()
        );
    }
    println!(
        "  Existing embeddings: {}",
        has_embedding.to_string().green()
    );
    println!("  Need embedding: {}", needs_embedding.to_string().yellow());
    println!();

    if needs_embedding == 0 {
        println!("{} All mappings already have embeddings!", "✓".green());
        return Ok(());
    }

    // Collect mappings that need embeddings
    let indices_needing_embedding: Vec<usize> = all_mappings
        .iter()
        .enumerate()
        .filter(|(_, m)| force_refresh || m.get_embedding().is_none())
        .map(|(i, _)| i)
        .collect();

    let arabic_names: Vec<String> = indices_needing_embedding
        .iter()
        .map(|&i| all_mappings[i].arabic_name.clone())
        .collect();

    println!(
        "Generating embeddings for {} medications ({} concurrent batches)...",
        arabic_names.len().to_string().yellow(),
        CONCURRENCY.to_string().cyan()
    );

    let start = Instant::now();
    let generated_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let all_mappings = Arc::new(Mutex::new(all_mappings));
    let parser = Arc::new(parser);
    let repo = Arc::new(repo);

    // Create batches
    let batches: Vec<(usize, Vec<String>, Vec<usize>)> = (0..arabic_names.len())
        .step_by(BATCH_SIZE)
        .map(|batch_start| {
            let batch_end = (batch_start + BATCH_SIZE).min(arabic_names.len());
            let batch: Vec<String> = arabic_names[batch_start..batch_end].to_vec();
            let indices: Vec<usize> = indices_needing_embedding[batch_start..batch_end].to_vec();
            (batch_start, batch, indices)
        })
        .collect();

    let total_batches = batches.len();
    let completed_batches = Arc::new(AtomicUsize::new(0));

    // Process batches concurrently
    let semaphore = Arc::new(tokio::sync::Semaphore::new(CONCURRENCY));
    let mut handles = Vec::new();

    for (batch_start, batch, indices) in batches {
        let parser = Arc::clone(&parser);
        let repo = Arc::clone(&repo);
        let all_mappings = Arc::clone(&all_mappings);
        let generated_count = Arc::clone(&generated_count);
        let error_count = Arc::clone(&error_count);
        let completed_batches = Arc::clone(&completed_batches);
        let semaphore = Arc::clone(&semaphore);
        let batch_end = batch_start + batch.len();

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            match parser.embed_batch(&batch).await {
                Ok(embeddings) => {
                    // Collect updates first, then save (minimize lock time)
                    let updates: Vec<(usize, Vec<f32>)> = indices
                        .iter()
                        .zip(embeddings.into_iter())
                        .map(|(&idx, emb)| (idx, emb))
                        .collect();

                    for (idx, emb) in updates {
                        // Update mapping
                        {
                            let mut mappings = all_mappings.lock().await;
                            mappings[idx].set_embedding(emb);
                        }

                        // Save outside lock
                        let mapping = {
                            let mappings = all_mappings.lock().await;
                            mappings[idx].clone()
                        };

                        if let Err(e) = repo.save(&mapping).await {
                            eprintln!(
                                "    {} Failed to save embedding for {}: {}",
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
                Err(e) => {
                    eprintln!(
                        "    {} Batch {}-{} failed: {}",
                        "✗".red(),
                        batch_start + 1,
                        batch_end,
                        e
                    );
                    error_count.fetch_add(batch.len(), Ordering::Relaxed);
                }
            }

            let completed = completed_batches.fetch_add(1, Ordering::Relaxed) + 1;
            println!(
                "  {} Batch {}-{} complete ({}/{})",
                "✓".green(),
                batch_start + 1,
                batch_end,
                completed,
                total_batches
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

    println!();
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
    }

    println!();
    println!("🔌 Circuit Breaker: {:?}", parser.circuit_state());
    println!();
    println!("✅ Embedding refresh complete.");

    Ok(())
}
