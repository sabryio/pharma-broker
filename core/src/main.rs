//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use pharma_core::ai::PharmaParser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::api::handlers::init_start_time;
use pharma_core::api::{create_router, routes::AppState};
use pharma_core::grpc::{GrpcDependencies, GrpcRepositories, PharmaCoreService, start_grpc_server};
use pharma_core::matching::{MatchingEngine, MatchingEngineConfig};
use pharma_core::metrics::init_metrics;
use pharma_core::repository::{
    SeaOrmAuditLogRepo, SeaOrmFeedbackRepo, SeaOrmGroupRepo, SeaOrmMatchQueueRepo, SeaOrmMatchRepo,
    SeaOrmMedicationMappingRepo, SeaOrmOfferRepo, SeaOrmRawMessageRepo, SeaOrmRequestRepo,
    SeaOrmReviewQueueRepo, create_connection, pharma_db,
};
use pharma_core::worker::match_processor::MatchProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

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
    pharma_db::migration::run_migrations(&db).await?;
    tracing::info!("✅ Migrations complete");

    // Create repositories (SeaORM)
    let offer_repo = Arc::new(SeaOrmOfferRepo::new(db.clone()));
    let request_repo = Arc::new(SeaOrmRequestRepo::new(db.clone()));
    let match_repo = Arc::new(SeaOrmMatchRepo::new(db.clone()));
    let raw_message_repo = Arc::new(SeaOrmRawMessageRepo::new(db.clone()));
    let group_repo = Arc::new(SeaOrmGroupRepo::new(db.clone()));
    let feedback_repo = Arc::new(SeaOrmFeedbackRepo::new(db.clone()));
    let review_queue_repo = Arc::new(SeaOrmReviewQueueRepo::new(db.clone()));

    let audit_log_repo = Arc::new(SeaOrmAuditLogRepo::new(db.clone()));
    let match_queue_repo = Arc::new(SeaOrmMatchQueueRepo::new(db.clone()));

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

    let matching_engine = Arc::new(MatchingEngine::new(engine_config));
    tracing::info!("⚖️ Matching engine initialized");

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

    let processor = MatchProcessor::new(
        match_queue_repo.clone(),
        offer_repo.clone(),
        request_repo.clone(),
        match_repo.clone(),
        audit_log_repo.clone(),
        matching_engine.clone(),
        ws_tx.clone(),
    );
    let worker_handle = tokio::spawn(async move {
        processor.run(worker_shutdown_rx).await;
    });

    // Create application state for HTTP (with matching engine)
    let state = AppState {
        offer_repo: offer_repo.clone(),
        request_repo: request_repo.clone(),
        match_repo: match_repo.clone(),
        group_repo: group_repo.clone(),
        matching_engine: Some(matching_engine.clone()),
        ws_tx: ws_tx.clone(),
        metrics_handle: Some(metrics_handle),
        feedback_repo: feedback_repo.clone(),
        review_queue_repo: review_queue_repo.clone(),
        audit_log_repo: audit_log_repo.clone(),
        medication_mapping_repo: medication_mapping_repo.clone(),
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
        feedback: feedback_repo,
        review_queue: review_queue_repo,
        audit_log: audit_log_repo,
        match_queue: match_queue_repo.clone(),
        medication_mapping: medication_mapping_repo,
        match_repo: match_repo.clone(),
    };
    let grpc_deps = GrpcDependencies {
        ai_client,
        ws_tx: ws_tx.clone(),
        matching_engine: matching_engine.clone(),
    };
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
            match tokio::time::timeout(drain_timeout, worker_handle).await {
                Ok(Ok(())) => tracing::info!("✅ Worker stopped gracefully"),
                Ok(Err(e)) => tracing::warn!("⚠️ Worker task panicked: {}", e),
                Err(_) => tracing::warn!("⚠️ Worker drain timed out after {:?}", drain_timeout),
            }

            // Phase 5: Final drain for servers
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        },
    }

    tracing::info!("👋 PharmaBroker Core Engine stopped cleanly");
    Ok(())
}
