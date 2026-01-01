//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use pharma_core::ai::PharmaParser;
use pharma_db::migration;
use pharma_db::repo::SeaOrmParticipantRepo;
#[cfg(not(feature = "tokio-console"))]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::api::handlers::init_start_time;
use pharma_core::api::{create_router, routes::AppState};
use pharma_core::grpc::{GrpcDependencies, GrpcRepositories, PharmaCoreService, start_grpc_server};
use pharma_core::matching::{
    MatchingEngine, MatchingEngineConfig, MedicationResolver, MedicationResolverConfig,
};
use pharma_core::metrics::init_metrics;
use pharma_core::repository::{
    SeaOrmAuditLogRepo, SeaOrmFeedbackRepo, SeaOrmGroupRepo, SeaOrmMatchQueueRepo, SeaOrmMatchRepo,
    SeaOrmMedicationAliasRepo, SeaOrmMedicationMappingRepo, SeaOrmMedicationMasterRepo,
    SeaOrmOfferRepo, SeaOrmRawMessageRepo, SeaOrmRequestRepo, SeaOrmReviewQueueRepo,
    create_connection,
};
use pharma_core::worker::match_processor::{MatchProcessor, MatchProcessorRepos};

/// Initialize tracing subscriber with optional tokio-console support
fn init_tracing() {
    #[cfg(feature = "tokio-console")]
    {
        // When tokio-console is enabled, use console_subscriber
        // This enables runtime debugging via `tokio-console` CLI tool
        console_subscriber::init();
        eprintln!("🔍 Tokio Console enabled - connect with `tokio-console` CLI");
    }

    #[cfg(not(feature = "tokio-console"))]
    {
        // Standard tracing setup
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (with optional tokio-console support)
    init_tracing();

    tracing::info!("🦀 PharmaBroker Core Engine v0.1.0 starting...");

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();
    tracing::info!("📊 Prometheus metrics initialized");

    // Initialize uptime tracking for health checks
    init_start_time();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get database URL
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://pharma:pharma@localhost:5432/pharmabroker".into());

    tracing::info!("Connecting to database...");

    // Create database connection pool (SeaORM)
    let db = create_connection(&database_url).await?;
    tracing::info!("✅ Database connected");

    // Run migrations
    tracing::info!("🔄 Running database migrations...");
    migration::run_migrations(&db).await?;
    tracing::info!("✅ Migrations complete");

    // Create repositories (SeaORM)
    let offer_repo = Arc::new(SeaOrmOfferRepo::new(db.clone()));
    let request_repo = Arc::new(SeaOrmRequestRepo::new(db.clone()));
    let match_repo = Arc::new(SeaOrmMatchRepo::new(db.clone()));
    let raw_message_repo = Arc::new(SeaOrmRawMessageRepo::new(db.clone()));
    let group_repo = Arc::new(SeaOrmGroupRepo::new(db.clone()));
    let participant_repo = Arc::new(SeaOrmParticipantRepo::new(db.clone()));
    let feedback_repo = Arc::new(SeaOrmFeedbackRepo::new(db.clone()));
    let review_queue_repo = Arc::new(SeaOrmReviewQueueRepo::new(db.clone()));

    let audit_log_repo = Arc::new(SeaOrmAuditLogRepo::new(db.clone()));
    let match_queue_repo = Arc::new(SeaOrmMatchQueueRepo::new(db.clone()));
    let medication_master_repo = Arc::new(SeaOrmMedicationMasterRepo::new(db.clone()));
    let medication_alias_repo = Arc::new(SeaOrmMedicationAliasRepo::new(db.clone()));

    // Create AI client (reads AI_GATEWAY_URL from env)
    let ai_client = Arc::new(PharmaParser::from_env());
    let gateway_url =
        std::env::var("AI_GATEWAY_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    tracing::info!("🤖 AI Gateway: {}", gateway_url);

    // Create broadcast channel for real-time events
    let medication_mapping_repo = Arc::new(SeaOrmMedicationMappingRepo::new(db.clone()));
    let (ws_tx, _) = tokio::sync::broadcast::channel(100);

    // Track active WebSocket connections
    let active_connections = Arc::new(AtomicUsize::new(0));

    // Create matching engine with scheduler config from environment
    let scheduler_enabled = std::env::var("LEARNING_SCHEDULER_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let scheduler_schedule =
        std::env::var("LEARNING_SCHEDULER_CRON").unwrap_or_else(|_| "0 0 3 * * *".to_string()); // Daily at 3 AM

    let mut engine_config = MatchingEngineConfig::default();
    engine_config.scheduler.enabled = scheduler_enabled;
    engine_config.scheduler.schedule = scheduler_schedule.clone();

    // Create matching engine and set repositories for learning job
    let mut matching_engine = MatchingEngine::new(engine_config);
    matching_engine.set_repositories(feedback_repo.clone(), audit_log_repo.clone());
    let matching_engine = Arc::new(matching_engine);
    tracing::info!("⚖️ Matching engine initialized with feedback repository");

    // Start the learning scheduler if enabled
    if scheduler_enabled {
        if let Err(e) = matching_engine.clone().start_scheduler().await {
            tracing::error!(error = %e, "Failed to start learning scheduler");
        } else {
            tracing::info!(schedule = %scheduler_schedule, "📅 Learning scheduler started");
        }
    } else {
        tracing::info!(
            "📅 Learning scheduler disabled (set LEARNING_SCHEDULER_ENABLED=true to enable)"
        );
    }

    // Initialize and start MatchProcessor (background worker)
    // Create shutdown channel for worker coordination
    let (worker_shutdown_tx, worker_shutdown_rx) = tokio::sync::watch::channel(false);
    let rx_match = worker_shutdown_rx.clone();
    let rx_janitor = worker_shutdown_rx.clone();

    let processor_repos = MatchProcessorRepos {
        match_queue: match_queue_repo.clone(),
        offer: offer_repo.clone(),
        request: request_repo.clone(),
        match_repo: match_repo.clone(),
        audit_log: audit_log_repo.clone(),
        feedback: feedback_repo.clone(),
    };
    let processor = MatchProcessor::new(processor_repos, matching_engine.clone(), ws_tx.clone());
    let worker_handle = tokio::spawn(async move {
        processor.run(rx_match).await;
    });

    // Initialize and start Janitor (cleanup & partitioning worker)
    let janitor_config = pharma_core::worker::janitor::JanitorConfig::from_env();
    let janitor_repos = pharma_core::worker::janitor::JanitorRepositories {
        raw_message: raw_message_repo.clone(),
        offer: offer_repo.clone(),
        request: request_repo.clone(),
        match_repo: match_repo.clone(),
        match_queue: match_queue_repo.clone(),
        audit_log: audit_log_repo.clone(),
    };
    let janitor =
        pharma_core::worker::janitor::Janitor::new(janitor_config, db.clone(), janitor_repos);
    let janitor_handle = tokio::spawn(async move {
        janitor.run(rx_janitor).await;
    });

    // Create application state for HTTP (with matching engine)
    let state = AppState {
        offer_repo: offer_repo.clone(),
        request_repo: request_repo.clone(),
        match_repo: match_repo.clone(),
        group_repo: group_repo.clone(),
        audit_log_repo: audit_log_repo.clone(),
        medication_mapping_repo: medication_mapping_repo.clone(),
        medication_master_repo: medication_master_repo.clone(),
        medication_alias_repo: medication_alias_repo.clone(),
        matching_engine: Some(matching_engine.clone()),
        ai_client: ai_client.clone(),
        ws_tx: ws_tx.clone(),
        metrics_handle: Some(metrics_handle),
        feedback_repo: feedback_repo.clone(),
        review_queue_repo: review_queue_repo.clone(),
        active_connections: active_connections.clone(),
    };

    // Create HTTP router
    let app = create_router(state);

    // Parse ports
    let http_port: u16 = std::env::var("API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let grpc_port: u16 = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50051);

    let http_addr = SocketAddr::from(([0, 0, 0, 0], http_port));
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], grpc_port));

    tracing::info!("🚀 HTTP server listening on http://{}", http_addr);
    tracing::info!("🔌 gRPC server listening on grpc://{}", grpc_addr);

    // Create gRPC service with all repositories and AI client
    let grpc_repos = GrpcRepositories {
        offer: offer_repo,
        request: request_repo,
        raw_message: raw_message_repo,
        group: group_repo,
        participant: participant_repo,
        feedback: feedback_repo,
        review_queue: review_queue_repo,
        audit_log: audit_log_repo,
        match_queue: match_queue_repo.clone(),
        medication_mapping: medication_mapping_repo,
        medication_master: medication_master_repo.clone(),
        medication_alias: medication_alias_repo.clone(),
        match_repo: match_repo.clone(),
    };

    // Create medication resolver for dynamic resolution
    let medication_resolver_config = MedicationResolverConfig {
        auto_approve_threshold: std::env::var("MED_RESOLVER_AUTO_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.92),
        minimum_threshold: std::env::var("MED_RESOLVER_MIN_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.70),
        create_aliases: true,
        auto_create_masters: false,
    };
    let medication_resolver = Arc::new(MedicationResolver::new(
        medication_resolver_config,
        medication_master_repo,
        medication_alias_repo,
    ));
    tracing::info!(
        auto_threshold = %medication_resolver.config().auto_approve_threshold,
        min_threshold = %medication_resolver.config().minimum_threshold,
        "🔗 Medication resolver initialized"
    );

    let grpc_deps = GrpcDependencies::new(ai_client, ws_tx.clone(), matching_engine.clone())
        .with_medication_resolver(medication_resolver);
    let grpc_service = PharmaCoreService::new(grpc_repos, grpc_deps);

    // Shutdown signal for graceful termination (Ctrl+C and SIGTERM)
    let shutdown = async {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => tracing::info!("📛 Ctrl+C received"),
            _ = terminate => tracing::info!("📛 SIGTERM received"),
        }

        tracing::info!("🛑 Starting graceful shutdown...");
    };

    // Shared shutdown signal for both servers
    let (tx, _rx) = tokio::sync::watch::channel(());

    // Clone matching_engine for shutdown
    let matching_engine_shutdown = matching_engine.clone();

    let grpc = {
        let mut rx = tx.subscribe();
        async move {
            let shutdown_future = async move {
                let _ = rx.changed().await;
            };
            if let Err(e) = start_grpc_server(grpc_addr, grpc_service, shutdown_future).await {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    };

    let http = {
        let mut rx = tx.subscribe();
        async move {
            let shutdown_future = async move {
                let _ = rx.changed().await;
            };
            let listener = tokio::net::TcpListener::bind(http_addr)
                .await
                .expect("Failed to bind HTTP listener");
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_future)
                .await
            {
                tracing::error!("HTTP server error: {}", e);
            }
        }
    };

    // Start both servers concurrently and wait for shutdown
    tokio::select! {
        _ = grpc => {},
        _ = http => {},
        _ = shutdown => {
            // Phase 1: Signal servers to stop accepting new connections
            tracing::info!("Phase 1: Stopping servers...");
            let _ = tx.send(());

            // Phase 2: Stop background workers
            tracing::info!("Phase 2: Stopping background workers...");
            let _ = worker_shutdown_tx.send(true);

            // Phase 3: Stop the learning scheduler
            tracing::info!("Phase 3: Stopping learning scheduler...");
            matching_engine_shutdown.stop_scheduler().await;

            // Phase 4: Wait for workers to drain (with timeout)
            tracing::info!("Phase 4: Waiting for workers to drain...");
            let drain_timeout = std::time::Duration::from_secs(10);

            let _ = tokio::join!(
                async {
                    match tokio::time::timeout(drain_timeout, worker_handle).await {
                        Ok(Ok(())) => tracing::info!("✅ MatchProcessor stopped gracefully"),
                        Ok(Err(e)) => tracing::warn!("⚠️ MatchProcessor task panicked: {}", e),
                        Err(_) => tracing::warn!("⚠️ MatchProcessor drain timed out after {:?}", drain_timeout),
                    }
                },
                async {
                    match tokio::time::timeout(drain_timeout, janitor_handle).await {
                        Ok(Ok(())) => tracing::info!("✅ Janitor stopped gracefully"),
                        Ok(Err(e)) => tracing::warn!("⚠️ Janitor task panicked: {}", e),
                        Err(_) => tracing::warn!("⚠️ Janitor drain timed out after {:?}", drain_timeout),
                    }
                }
            );

            // Phase 5: Final drain for servers
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        },
    }

    tracing::info!("👋 PharmaBroker Core Engine stopped cleanly");
    Ok(())
}
