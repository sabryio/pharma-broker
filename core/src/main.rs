//! PharmaBroker Core Engine - Entry Point

use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🦀 PharmaBroker Core Engine starting...");

    // Load configuration
    dotenvy::dotenv().ok();

    // TODO: Initialize database connection
    // TODO: Initialize repositories
    // TODO: Initialize matching engine
    // TODO: Start API server

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Listening on {}", addr);

    // Placeholder - will be replaced with actual server
    println!("PharmaBroker Core Engine v0.1.0");
    println!("Server would start on http://{}", addr);

    Ok(())
}
