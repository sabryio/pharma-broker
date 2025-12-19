//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use pharma_core::api::{create_router, routes::AppState};
use pharma_core::matching::Scorer;
use pharma_core::repository::{
    PostgresMatchRepo, PostgresOfferRepo, PostgresRequestRepo, create_pool,
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

    // Create scorer
    let scorer = Arc::new(Scorer::default());

    // Create application state
    let state = AppState {
        offer_repo,
        request_repo,
        match_repo,
        scorer,
    };

    // Create router
    let app = create_router(state);

    // Start server
    let port: u16 = std::env::var("API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("🚀 Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
