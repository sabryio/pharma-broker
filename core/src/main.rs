//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::api::{create_router, routes::AppState};
use pharma_core::grpc::{PharmaCoreService, start_grpc_server};
use pharma_core::matching::Scorer;
use pharma_core::repository::{
    PostgresGroupRepo, PostgresMatchRepo, PostgresOfferRepo, PostgresRawMessageRepo,
    PostgresRequestRepo, create_pool,
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

    // Create scorer
    let scorer = Arc::new(Scorer::default());

    // Create application state for HTTP
    let state = AppState {
        offer_repo: offer_repo.clone(),
        request_repo: request_repo.clone(),
        match_repo,
        scorer,
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

    // Create gRPC service with all repositories
    let grpc_service =
        PharmaCoreService::new(offer_repo, request_repo, raw_message_repo, group_repo);

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
