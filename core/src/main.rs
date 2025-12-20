//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::AiClient;
use pharma_core::api::handlers::init_start_time;
use pharma_core::api::{create_router, routes::AppState};
use pharma_core::grpc::{PharmaCoreService, start_grpc_server};
use pharma_core::matching::{MatchingEngine, MatchingEngineConfig};
use pharma_core::metrics::init_metrics;
use pharma_core::repository::postgres::{
    PostgresAuditLogRepo, PostgresFeedbackRepo, PostgresGroupRepo, PostgresMatchQueueRepo,
    PostgresMatchRepo, PostgresMedicationMappingRepo, PostgresOfferRepo, PostgresRawMessageRepo,
    PostgresRequestRepo, PostgresReviewQueueRepo, create_pool,
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

    // Create database connection pool
    let pool = create_pool(&database_url).await?;
    tracing::info!("✅ Database connected");

    // Create repositories
    let offer_repo = Arc::new(PostgresOfferRepo::new(pool.clone()));
    let request_repo = Arc::new(PostgresRequestRepo::new(pool.clone()));
    let match_repo = Arc::new(PostgresMatchRepo::new(pool.clone()));
    let raw_message_repo = Arc::new(PostgresRawMessageRepo::new(pool.clone()));
    let group_repo = Arc::new(PostgresGroupRepo::new(pool.clone()));
    let feedback_repo = Arc::new(PostgresFeedbackRepo::new(pool.clone()));
    let review_queue_repo = Arc::new(PostgresReviewQueueRepo::new(pool.clone()));

    let audit_log_repo = Arc::new(PostgresAuditLogRepo::new(pool.clone()));
    let match_queue_repo = Arc::new(PostgresMatchQueueRepo::new(pool.clone()));

    // Create AI client (reads AI_GATEWAY_URL from env)
    let ai_client = Arc::new(AiClient::from_env());
    let gateway_url =
        std::env::var("AI_GATEWAY_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    tracing::info!("🤖 AI Gateway: {}", gateway_url);

    // Create broadcast channel for real-time events
    let medication_mapping_repo = Arc::new(PostgresMedicationMappingRepo::new(pool.clone()));
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
    let processor = MatchProcessor::new(
        match_queue_repo.clone(),
        offer_repo.clone(),
        request_repo.clone(),
        match_repo.clone(),
        audit_log_repo.clone(),
        matching_engine.clone(),
        ai_client.clone(),
        ws_tx.clone(),
    );
    tokio::spawn(async move {
        processor.run().await;
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
    let grpc_service = PharmaCoreService::new(
        offer_repo,
        request_repo,
        raw_message_repo,
        group_repo,
        feedback_repo,
        review_queue_repo,
        audit_log_repo,
        match_queue_repo.clone(),
        medication_mapping_repo,
        match_repo.clone(),
        ai_client,
        ws_tx.clone(),
        matching_engine,
    );

    // Shutdown signal for graceful termination
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        tracing::info!("Shutdown signal received, starting graceful shutdown...");
    };

    // Shared shutdown signal for both servers
    let (tx, _rx) = tokio::sync::watch::channel(());

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
            let _ = tx.send(());
            // Give servers a moment to drain
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        },
    }

    tracing::info!("👋 PharmaBroker Core Engine stopped cleanly");
    Ok(())
}
