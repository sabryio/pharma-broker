//! # Pipeline Simulation CLI
//!
//! Full end-to-end pipeline simulation using actual PharmaBroker implementations.
//! Simulates WhatsApp message flow from ingestion to AI parsing to entity creation.
//!
//! ## Usage
//! ```bash
//! cargo run --bin pipeline-sim -- --limit 8
//! cargo run --bin pipeline-sim -- -l 5
//! ```
//!
//! ## Pipeline Stages
//! 1. Initialize database and repositories
//! 2. Create AI client with token batching
//! 3. Create BatchProcessor
//! 4. Simulate WhatsApp messages
//! 5. Process with AI parsing + entity creation
//! 6. Display results
//!
//! ## Environment Variables
//! - `DATABASE_URL` - PostgreSQL connection string
//! - `AI_BASE_URL` / `AI_MODEL` - AI model configuration
//! - `RUST_LOG` - Logging level (default: info,sqlx=warn)

use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};
use tokio::sync::{broadcast, watch};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::{PharmaParser, PharmaParserConfig, TokenBatchConfig, TokenBatcher};
use pharma_core::domain::{Group, RawMessage};
use pharma_core::parsing::{BatchConfig, BatchProcessor, MultiPassConfig, ParseJob};
use pharma_core::repository::{
    GroupRepository, MatchRepository, OfferRepository, RawMessageRepository, RequestRepository,
    SeaOrmAuditLogRepo, SeaOrmGroupRepo, SeaOrmMatchQueueRepo, SeaOrmMatchRepo,
    SeaOrmMedicationMappingRepo, SeaOrmOfferRepo, SeaOrmRawMessageRepo, SeaOrmRequestRepo,
    SeaOrmReviewQueueRepo, create_connection,
};
use pharma_core::ws::WsEvent;

// ============================================================================
// Interactive Config
// ============================================================================

struct Config {
    limit: usize,
    database_url: String,
}

fn get_config_interactive() -> Config {
    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════════════════╗"
            .magenta()
    );
    println!(
        "{}",
        "║              🔬 PIPELINE SIMULATION                                          ║"
            .magenta()
            .bold()
    );
    println!(
        "{}",
        "║              Full message flow: Ingestion → AI Parsing → Entities            ║"
            .magenta()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════════════════╝"
            .magenta()
    );
    println!();

    let theme = ColorfulTheme::default();

    let limit: usize = Input::with_theme(&theme)
        .with_prompt("📝 Number of mock messages to process")
        .default(8)
        .interact_text()
        .unwrap_or(8);

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
        "  {} {} messages",
        "Config:".magenta().bold(),
        limit.to_string().yellow()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dimmed()
    );
    println!();

    Config {
        limit,
        database_url,
    }
}

// ============================================================================
// Phase Tracking
// ============================================================================

struct Phase {
    name: String,
    status: String,
    start: Instant,
    end: Option<Instant>,
    details: String,
}

struct Workflow {
    phases: Vec<Phase>,
}

impl Workflow {
    fn new() -> Self {
        Self { phases: Vec::new() }
    }

    fn start_phase(&mut self, name: &str) {
        info!("🚀 Starting phase: {}", name);
        self.phases.push(Phase {
            name: name.to_string(),
            status: "running".to_string(),
            start: Instant::now(),
            end: None,
            details: String::new(),
        });
    }

    fn end_phase(&mut self, status: &str, details: &str) {
        if let Some(phase) = self.phases.last_mut() {
            phase.end = Some(Instant::now());
            phase.status = status.to_string();
            phase.details = details.to_string();
            let elapsed = phase.end.unwrap().duration_since(phase.start);
            info!(
                "✅ Phase '{}' completed: {} ({:?})",
                phase.name, status, elapsed
            );
        }
    }

    fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("📊 WORKFLOW SUMMARY");
        println!("{}", "=".repeat(70));

        let mut total = Duration::ZERO;
        for (i, phase) in self.phases.iter().enumerate() {
            let duration = phase
                .end
                .map(|e| e.duration_since(phase.start))
                .unwrap_or_default();
            total += duration;

            let icon = if phase.status == "success" {
                "✅"
            } else {
                "❌"
            };
            println!(
                "{}. {} {:<25} {:<10} {:?}",
                i + 1,
                icon,
                phase.name,
                phase.status,
                duration
            );
            if !phase.details.is_empty() {
                println!("       {}", phase.details);
            }
        }
        println!("{}", "-".repeat(70));
        println!("   Total: {:?}", total);
    }
}

// ============================================================================
// Mock WhatsApp Messages (Realistic Egyptian Pharmacy Messages)
// ============================================================================

fn get_mock_whatsapp_messages() -> Vec<RawMessage> {
    let base_time = Utc::now();

    vec![
        // ========== Group 1: مجموعة صيادلة القاهرة ==========
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363012345678901@g.us".to_string(),
            group_name: "مجموعة صيادلة القاهرة".to_string(),
            sender_jid: "201012345678@s.whatsapp.net".to_string(),
            sender_phone: Some("201012345678".to_string()),
            sender_name: Some("د. أحمد صيدلي".to_string()),
            content: "السلام عليكم\nعندي للبيع:\n*اوجمنتين 1 جم* - 50 علبة بـ 150 جنيه\n*فلاجيل 500* - 30 علبة بـ 45 جنيه\nالتواصل واتس فقط".to_string(),
            timestamp: base_time - chrono::Duration::seconds(5),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363012345678901@g.us".to_string(),
            group_name: "مجموعة صيادلة القاهرة".to_string(),
            sender_jid: "201098765432@s.whatsapp.net".to_string(),
            sender_phone: Some("201098765432".to_string()),
            sender_name: Some("صيدلية النور".to_string()),
            content: "محتاج ضروري جداً:\n*أوجمنتين 1 جرام* 20 علبة\nأي سعر مناسب".to_string(),
            timestamp: base_time - chrono::Duration::seconds(3),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        // Reply to previous message
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363012345678901@g.us".to_string(),
            group_name: "مجموعة صيادلة القاهرة".to_string(),
            sender_jid: "201012345678@s.whatsapp.net".to_string(),
            sender_phone: Some("201012345678".to_string()),
            sender_name: Some("د. أحمد صيدلي".to_string()),
            content: "متوفر عندي، نفس السعر".to_string(),
            timestamp: base_time - chrono::Duration::seconds(1),
            processed_at: None,
            error: None,
            reply_to_id: Some("previous-msg-id".to_string()),
            reply_to_content: Some("محتاج ضروري جداً: أوجمنتين 1 جرام 20 علبة".to_string()),
            reply_to_sender: Some("201098765432@s.whatsapp.net".to_string()),
            created_at: base_time,
        },
        // ========== Group 2: موردين الأدوية ==========
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363098765432109@g.us".to_string(),
            group_name: "موردين الأدوية".to_string(),
            sender_jid: "201155555555@s.whatsapp.net".to_string(),
            sender_phone: Some("201155555555".to_string()),
            sender_name: Some("مورد أدوية".to_string()),
            content: "عرض اليوم 🔥\n*كتافلام 50 مجم* - 200 شريط @ 75 جنيه\n*بروفين 400* - 100 علبة @ 55 جنيه\n*بانادول اكسترا* - 50 علبة @ 40 جنيه\nالكميات محدودة!".to_string(),
            timestamp: base_time - chrono::Duration::seconds(10),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363098765432109@g.us".to_string(),
            group_name: "موردين الأدوية".to_string(),
            sender_jid: "201166666666@s.whatsapp.net".to_string(),
            sender_phone: Some("201166666666".to_string()),
            sender_name: Some("صيدلية الأمل".to_string()),
            content: "مطلوب عاجل:\n- كتافلام 50 - 100 شريط\n- بانادول اكسترا - 30 علبة".to_string(),
            timestamp: base_time - chrono::Duration::seconds(8),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        // ========== Group 3: صيادلة الإسكندرية ==========
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363055555555555@g.us".to_string(),
            group_name: "صيادلة الإسكندرية".to_string(),
            sender_jid: "201177777777@s.whatsapp.net".to_string(),
            sender_phone: Some("201177777777".to_string()),
            sender_name: Some("Pharmacy Alex".to_string()),
            content: "Available for sale:\nConcor 5mg - 40 boxes @ 85 LE\nZoloft 50mg - 25 boxes @ 120 LE".to_string(),
            timestamp: base_time - chrono::Duration::seconds(15),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363055555555555@g.us".to_string(),
            group_name: "صيادلة الإسكندرية".to_string(),
            sender_jid: "201188888888@s.whatsapp.net".to_string(),
            sender_phone: Some("201188888888".to_string()),
            sender_name: Some("Dr. Sara".to_string()),
            content: "محتاجين كونكور 5 - 20 علبة\nالحد الأقصى للسعر 90 جنيه".to_string(),
            timestamp: base_time - chrono::Duration::seconds(12),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
        },
        // ========== Multi-concentration message ==========
        RawMessage {
            id: uuid::Uuid::new_v4().to_string(),
            external_id: Some(uuid::Uuid::new_v4().to_string()),
            group_jid: "120363012345678901@g.us".to_string(),
            group_name: "مجموعة صيادلة القاهرة".to_string(),
            sender_jid: "201199999999@s.whatsapp.net".to_string(),
            sender_phone: Some("201199999999".to_string()),
            sender_name: Some("صيدلية الشفاء".to_string()),
            content: "مطلوب:\n*اوزمبك واحد ونص وربع*\n*زولادكس 3.6*".to_string(),
            timestamp: base_time - chrono::Duration::seconds(2),
            processed_at: None,
            error: None,
            reply_to_id: None,
            reply_to_content: None,
            reply_to_sender: None,
            created_at: base_time,
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
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info,sqlx=warn".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get config via interactive prompts
    let config = get_config_interactive();
    let limit = config.limit;
    let database_url = config.database_url;

    let mut workflow = Workflow::new();

    // ============================================================
    // PHASE 1: Initialize Database & Repositories
    // ============================================================
    workflow.start_phase("Database Init");

    let db = match create_connection(&database_url).await {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            workflow.end_phase("failed", &e.to_string());
            workflow.print_summary();
            return Err(e.into());
        }
    };

    // Create repositories
    let raw_message_repo = Arc::new(SeaOrmRawMessageRepo::new(db.clone()));
    let offer_repo = Arc::new(SeaOrmOfferRepo::new(db.clone()));
    let request_repo = Arc::new(SeaOrmRequestRepo::new(db.clone()));
    let match_repo = Arc::new(SeaOrmMatchRepo::new(db.clone()));
    let group_repo = Arc::new(SeaOrmGroupRepo::new(db.clone()));
    let medication_mapping_repo = Arc::new(SeaOrmMedicationMappingRepo::new(db.clone()));
    let review_queue_repo = Arc::new(SeaOrmReviewQueueRepo::new(db.clone()));
    let audit_log_repo = Arc::new(SeaOrmAuditLogRepo::new(db.clone()));
    let match_queue_repo = Arc::new(SeaOrmMatchQueueRepo::new(db.clone()));

    workflow.end_phase("success", "9 repositories initialized");

    // ============================================================
    // PHASE 2: Setup Monitored Groups
    // ============================================================
    workflow.start_phase("Group Setup");

    let test_groups = vec![
        Group {
            jid: "120363012345678901@g.us".to_string(),
            name: "مجموعة صيادلة القاهرة".to_string(),
            description: None,
            monitored: true,
            added_at: Utc::now(),
            last_message: None,
            message_count: 0,
        },
        Group {
            jid: "120363098765432109@g.us".to_string(),
            name: "موردين الأدوية".to_string(),
            description: None,
            monitored: true,
            added_at: Utc::now(),
            last_message: None,
            message_count: 0,
        },
        Group {
            jid: "120363055555555555@g.us".to_string(),
            name: "صيادلة الإسكندرية".to_string(),
            description: None,
            monitored: true,
            added_at: Utc::now(),
            last_message: None,
            message_count: 0,
        },
    ];

    for group in &test_groups {
        if let Err(e) = group_repo.save(group).await {
            warn!("Failed to save group {}: {}", group.jid, e);
        }
    }

    workflow.end_phase(
        "success",
        &format!("{} groups set as monitored", test_groups.len()),
    );

    // ============================================================
    // PHASE 3: Create AI Client
    // ============================================================
    workflow.start_phase("AI Client Init");

    let parser_config = PharmaParserConfig::from_env();
    let ai_client = Arc::new(PharmaParser::new(parser_config));

    // Create token batcher for context management
    let batcher = TokenBatcher::new(TokenBatchConfig::default());

    workflow.end_phase("success", "Direct AI client initialized");

    // ============================================================
    // PHASE 4: Create Batch Processor
    // ============================================================
    workflow.start_phase("Batch Processor Init");

    let (ws_tx, _ws_rx) = broadcast::channel::<WsEvent>(100);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let batch_config = BatchConfig {
        batch_size: 5,
        batch_timeout: Duration::from_secs(3),
        worker_count: 2,
        channel_buffer: 100,
    };

    let multi_pass_config = MultiPassConfig::default();

    let processor = Arc::new(BatchProcessor::new(
        batch_config.clone(),
        multi_pass_config,
        ai_client.clone(),
        raw_message_repo.clone(),
        offer_repo.clone(),
        request_repo.clone(),
        medication_mapping_repo,
        review_queue_repo,
        audit_log_repo,
        match_queue_repo,
        ws_tx,
    ));

    // Start processor in background
    let processor_handle = {
        let processor = processor.clone();
        tokio::spawn(async move {
            processor.run(shutdown_rx).await;
        })
    };

    workflow.end_phase(
        "success",
        &format!(
            "Batch size: {}, Timeout: {:?}",
            batch_config.batch_size, batch_config.batch_timeout
        ),
    );

    // ============================================================
    // PHASE 5: Simulate WhatsApp Messages
    // ============================================================
    workflow.start_phase("Message Simulation");

    let mock_messages = get_mock_whatsapp_messages();
    let messages_to_process: Vec<_> = mock_messages.into_iter().take(limit).collect();

    println!("\n📱 Simulating incoming WhatsApp messages...");
    println!("{}", "-".repeat(50));

    // Use token batcher to check for context limits
    let batch_messages: Vec<_> = messages_to_process
        .iter()
        .map(|m| {
            pharma_core::ai::BatchMessage::new(&m.id, &m.content)
                .with_sender(m.sender_name.as_deref().unwrap_or(""))
                .with_group(&m.group_name)
        })
        .collect();

    let batches = batcher.split_into_batches(batch_messages);
    println!(
        "📦 Token batcher split {} messages into {} batches",
        messages_to_process.len(),
        batches.len()
    );

    // Submit messages to processor
    let sender = processor.sender();
    for (i, msg) in messages_to_process.iter().enumerate() {
        println!(
            "{}. [{}] {}: {}",
            i + 1,
            msg.group_name,
            msg.sender_name.as_deref().unwrap_or("Unknown"),
            truncate(&msg.content, 50)
        );

        // Save raw message first (like real flow)
        if let Err(e) = raw_message_repo.save(msg).await {
            warn!("Failed to save raw message: {}", e);
        }

        // Submit to batch processor
        let job = ParseJob::new(msg.clone());
        if let Err(e) = sender.send(job).await {
            error!("Failed to submit message: {}", e);
        }

        // Small delay to simulate real message timing
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("{}", "-".repeat(50));

    workflow.end_phase(
        "success",
        &format!(
            "{} messages submitted to BatchProcessor",
            messages_to_process.len()
        ),
    );

    // ============================================================
    // PHASE 6: Wait for AI Processing
    // ============================================================
    workflow.start_phase("AI Processing");

    println!("\n⏳ Waiting for AI parsing...");

    // Wait for processing (batch timeout + some buffer)
    tokio::time::sleep(Duration::from_secs(15)).await;

    // Signal shutdown
    let _ = shutdown_tx.send(true);
    let _ = processor_handle.await;

    // Get stats
    let stats = processor.stats().await;

    workflow.end_phase(
        "success",
        &format!(
            "Received: {}, Batches: {}, Items: {}",
            stats.messages_received, stats.batches_processed, stats.items_extracted
        ),
    );

    // ============================================================
    // PHASE 7: Display Results
    // ============================================================
    workflow.start_phase("Results");

    // Fetch created entities
    let offers = offer_repo.get_active(100, 0).await.unwrap_or_default();
    let requests = request_repo.get_active(100, 0).await.unwrap_or_default();
    let pending_matches = match_repo.get_pending(100, 0).await.unwrap_or_default();

    workflow.end_phase(
        "success",
        &format!(
            "Offers: {}, Requests: {}, Matches: {}",
            offers.len(),
            requests.len(),
            pending_matches.len()
        ),
    );

    // ============================================================
    // SUMMARY
    // ============================================================
    workflow.print_summary();

    println!("\n📊 Database Contents:");
    println!("   - Groups Monitored: {}", test_groups.len());
    println!("   - Offers: {}", offers.len());
    println!("   - Requests: {}", requests.len());
    println!("   - Pending Matches: {}", pending_matches.len());

    // Show extracted medications
    if !offers.is_empty() {
        println!("\n💊 Extracted Offers:");
        for (i, o) in offers.iter().enumerate() {
            if i >= 5 {
                println!("   ... and {} more", offers.len() - 5);
                break;
            }
            println!(
                "   - {} ({:.0} units @ {:.0} EGP)",
                o.medication, o.quantity.unwrap_or_default(), o.price.unwrap_or_default()
            );
        }
    }

    if !requests.is_empty() {
        println!("\n📋 Extracted Requests:");
        for (i, r) in requests.iter().enumerate() {
            if i >= 5 {
                println!("   ... and {} more", requests.len() - 5);
                break;
            }
            let urgent = if r.is_urgent() { " 🔥" } else { "" };
            println!("   - {} ({:.0} units){}", r.medication, r.quantity.unwrap_or_default(), urgent);
        }
    }

    // Token batcher stats
    let batcher_stats = batcher.stats();
    println!("\n📦 Token Batcher Stats:");
    println!("   - Total batches: {}", batcher_stats.total_batches);
    println!(
        "   - Total tokens estimated: {}",
        batcher_stats.total_tokens
    );
    println!("   - Split batches: {}", batcher_stats.split_batches);
    println!(
        "   - Oversized messages: {}",
        batcher_stats.oversized_messages
    );

    if !offers.is_empty() || !requests.is_empty() {
        println!("\n✅ Workflow completed successfully - Real message flow simulated!");
    } else {
        println!("\n⚠️ No offers/requests created - check AI gateway connection");
    }

    Ok(())
}
