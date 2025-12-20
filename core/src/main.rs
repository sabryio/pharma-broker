//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::ai::AiClient;
use pharma_core::api::handlers::init_start_time;
use pharma_core::api::{create_router, routes::AppState};
use pharma_core::grpc::{PharmaCoreService, start_grpc_server};
use pharma_core::matching::{MatchingEngineConfig, create_matching_engine};
use pharma_core::metrics::init_metrics;
use pharma_core::repository::{
    PostgresFeedbackRepo, PostgresGroupRepo, PostgresMatchRepo, PostgresOfferRepo,
    PostgresRawMessageRepo, PostgresRequestRepo, PostgresReviewQueueRepo, create_pool,
};

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

    // Create AI client (reads AI_GATEWAY_URL from env)
    let ai_client = Arc::new(AiClient::from_env());
    let gateway_url =
        std::env::var("AI_GATEWAY_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    tracing::info!("🤖 AI Gateway: {}", gateway_url);

    // Create broadcast channel for real-time events
    let (ws_tx, _) = tokio::sync::broadcast::channel(100);

    // Create matching engine with scheduler config from environment
    let scheduler_enabled = std::env::var("LEARNING_SCHEDULER_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let scheduler_schedule =
        std::env::var("LEARNING_SCHEDULER_CRON").unwrap_or_else(|_| "0 0 3 * * *".to_string()); // Daily at 3 AM

    let mut engine_config = MatchingEngineConfig::default();
    engine_config.scheduler.enabled = scheduler_enabled;
    engine_config.scheduler.schedule = scheduler_schedule.clone();

    let matching_engine = create_matching_engine(engine_config);
    tracing::info!("⚖️ Matching engine initialized");

    // Start the learning scheduler if enabled
    if scheduler_enabled {
        if let Err(e) = matching_engine.start_scheduler().await {
            tracing::error!(error = %e, "Failed to start learning scheduler");
        } else {
            tracing::info!(schedule = %scheduler_schedule, "📅 Learning scheduler started");
        }
    } else {
        tracing::info!(
            "📅 Learning scheduler disabled (set LEARNING_SCHEDULER_ENABLED=true to enable)"
        );
    }

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
        match_repo.clone(),
        ai_client,
        ws_tx.clone(),
        matching_engine,
    );

    // Start both servers concurrently
    tokio::select! {
        result = async {
            let listener = tokio::net::TcpListener::bind(http_addr).await?;
            axum::serve(listener, app).await
        } => {
            if let Err(e) = result {
                tracing::error!("HTTP server error: {}", e);
            }
        }
        result = start_grpc_server(grpc_addr, grpc_service) => {
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    }

    Ok(())
}
