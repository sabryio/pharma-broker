//! # Vector Similarity Search Playground
//!
//! Uses embeddings to find semantically similar medication mappings (RAG).
//! Ported from legacy/cmd/playground/vector_search/main.go
//!
//! ## Usage
//! ```bash
//! cargo run --bin vector-search
//! ```

use std::env;
use std::time::Instant;

use colored::Colorize;
use pharma_core::ai::{PharmaParser, PharmaParserConfig};
use pharma_core::domain::MedicationMapping;
use pharma_core::repository::{
    MedicationMappingRepository, SeaOrmMedicationMappingRepo, create_connection,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Test Content (same as Go version)
// ============================================================================

const TEST_CONTENT: &str = r#"*محتاج جدا*
 
*اوزمبك واحد ونص وربع*
_____________________

*مريوفيرت 150*
*فوستيمون 150*

*جونابيور 150*
*جونابيور 75*

*سيتروتايد ربع*

*أوفتريل 250*
*كوريومون 5000*
*ابيفاسي 5000*

*ديكابيبتايل*
*تريبتوفيم*

*جونال 900*

*ابيجونال 75*

*زولادكس 3.6*

*برولوتكس*
_____________________

*سكسندا*

*ريبلسس 7*

*جوناتستون حقن*

*انفانز*

*بنتازا اقراص*

*بنتازا لبوس*

*زيلودا* *(علبة مستوردة ناقصها شريط ب ٤٧٠٠)*"#;

// ============================================================================
// Similarity Search
// ============================================================================

struct ScoredMapping {
    mapping: MedicationMapping,
    score: f32,
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

fn find_similar(
    mappings: &[MedicationMapping],
    query_embedding: &[f32],
    top_k: usize,
) -> Vec<ScoredMapping> {
    let mut results: Vec<ScoredMapping> = mappings
        .iter()
        .filter_map(|m| {
            m.get_embedding().map(|emb: Vec<f32>| ScoredMapping {
                mapping: m.clone(),
                score: cosine_similarity(query_embedding, &emb),
            })
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results.truncate(top_k);
    results
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

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "║              🔍 VECTOR SIMILARITY SEARCH PLAYGROUND                          ║"
            .green()
            .bold()
    );
    println!(
        "{}",
        "║              RAG-based semantic medication mapping search                    ║".green()
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

    println!(
        "📊 Total mappings in DB: {}",
        all_mappings.len().to_string().yellow()
    );
    println!();

    // Create AI parser for embeddings
    let ai_url = env::var("AI_BASE_URL")
        .or_else(|_| env::var("LLM_URL"))
        .unwrap_or_else(|_| "http://localhost:12434/engines/llama.cpp/v1".to_string());

    println!("🔗 AI Backend: {}", ai_url.cyan());

    let mut parser_config = PharmaParserConfig::from_env();
    parser_config.client.base_url = ai_url;
    let parser = PharmaParser::new(parser_config);

    // =========================================================================
    // Step 1: Check/generate embeddings
    // =========================================================================
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "{}",
        "Step 1: Checking/generating embeddings...".yellow().bold()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    let needs_embedding: Vec<usize> = all_mappings
        .iter()
        .enumerate()
        .filter(|(_, m)| m.get_embedding().is_none())
        .map(|(i, _)| i)
        .collect();

    if !needs_embedding.is_empty() {
        println!(
            "  Generating embeddings for {} mappings...",
            needs_embedding.len().to_string().yellow()
        );

        // Collect Arabic names for batch embedding
        let arabic_names: Vec<String> = needs_embedding
            .iter()
            .map(|&i| all_mappings[i].arabic_name.clone())
            .collect();

        let embeddings = parser.embed_batch(&arabic_names).await?;

        // Store embeddings
        for (j, emb) in embeddings.into_iter().enumerate() {
            let idx = needs_embedding[j];
            all_mappings[idx].set_embedding(emb);
            if let Err(e) = repo.save(&all_mappings[idx]).await {
                eprintln!(
                    "  {} Failed to save embedding for {}: {}",
                    "⚠".yellow(),
                    all_mappings[idx].arabic_name,
                    e
                );
            }
        }
        println!(
            "  {} Generated and stored {} embeddings",
            "✓".green(),
            needs_embedding.len()
        );
    } else {
        println!("  {} All mappings already have embeddings", "✓".green());
    }

    // =========================================================================
    // Step 2: Test semantic search
    // =========================================================================
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "{}",
        "Step 2: Testing semantic similarity search..."
            .yellow()
            .bold()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    println!("Test message:");
    println!("{}", TEST_CONTENT.dimmed());
    println!();

    // Embed the test message
    println!("  Generating message embedding...");
    let message_embedding = parser.embed(TEST_CONTENT).await?;

    // Find top-K similar mappings
    let top_k = 10;
    let similar = find_similar(&all_mappings, &message_embedding, top_k);

    println!(
        "Top {} similar mappings (by cosine similarity):",
        top_k.to_string().green()
    );
    for (i, s) in similar.iter().enumerate() {
        println!(
            "  {:2}. [{:.4}] {} => {}",
            i + 1,
            s.score,
            s.mapping.arabic_name.cyan(),
            s.mapping.english_name
        );
    }

    // =========================================================================
    // Step 3: Compare with keyword matching
    // =========================================================================
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!(
        "{}",
        "Step 3: Comparison with keyword matching..."
            .yellow()
            .bold()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );

    let content_lower = TEST_CONTENT.to_lowercase();
    let keyword_matches: Vec<&MedicationMapping> = all_mappings
        .iter()
        .filter(|m| content_lower.contains(&m.arabic_name.to_lowercase()))
        .collect();

    println!(
        "Keyword matches: {}",
        keyword_matches.len().to_string().green()
    );
    for m in &keyword_matches {
        println!("  {} {} => {}", "✓".green(), m.arabic_name, m.english_name);
    }

    // Find semantic-only matches
    let keyword_names: std::collections::HashSet<&str> = keyword_matches
        .iter()
        .map(|m| m.arabic_name.as_str())
        .collect();

    let semantic_only: Vec<&ScoredMapping> = similar
        .iter()
        .filter(|s| !keyword_names.contains(s.mapping.arabic_name.as_str()))
        .collect();

    println!();
    println!(
        "Semantic-only matches (not found by keywords): {}",
        semantic_only.len().to_string().yellow()
    );
    for s in &semantic_only {
        println!(
            "  {} {} => {}",
            "+".blue(),
            s.mapping.arabic_name,
            s.mapping.english_name
        );
    }

    // =========================================================================
    // Step 4: Live AI test with semantic filtering
    // =========================================================================
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║         🧪 LIVE AI TEST WITH SEMANTIC-FILTERED MAPPINGS                      ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝".cyan()
    );

    // Combine keyword + top 5 semantic matches
    let mut filtered_mappings: Vec<String> = keyword_matches
        .iter()
        .map(|m| m.to_prompt_context())
        .collect();

    for s in similar.iter().take(5) {
        if !keyword_names.contains(s.mapping.arabic_name.as_str()) {
            filtered_mappings.push(s.mapping.to_prompt_context());
        }
    }

    println!(
        "Using {} filtered mappings (keyword + top semantic)",
        filtered_mappings.len().to_string().green()
    );

    let start = Instant::now();
    let results = parser
        .parse(
            TEST_CONTENT,
            Some("Test"),
            "Test",
            None,
            Some(&filtered_mappings),
        )
        .await?;
    let duration = start.elapsed();

    println!(
        "Parsed in {:?} (with {} filtered mappings)",
        duration,
        filtered_mappings.len()
    );
    println!();
    println!("Results:");
    for (i, item) in results.iter().enumerate() {
        let type_icon = if item.item_type == pharma_core::ai::Intent::Offer {
            "🟢"
        } else {
            "🔵"
        };
        println!(
            "  [{}] {} {} (Raw: {}) [{}]",
            i,
            type_icon,
            item.medication.green(),
            item.medication_raw.dimmed(),
            format!("{:?}", item.item_type).cyan()
        );
    }

    // =========================================================================
    // Summary
    // =========================================================================
    println!();
    println!("🔌 Circuit Breaker: {:?}", parser.circuit_state());
    println!();
    println!("✅ Vector search playground complete.");

    Ok(())
}
