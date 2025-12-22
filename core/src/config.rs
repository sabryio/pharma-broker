//! Configuration module
//!
//! TODO: Port from legacy/pkg/config/config.go

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub api_port: u16,
    pub redis_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgres://pharma:pharma@localhost:5432/pharmabroker".into(),
            api_port: 8080,
            redis_url: None,
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| Self::default().database_url),
            api_port: std::env::var("API_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            redis_url: std::env::var("REDIS_URL").ok(),
        })
    }
}
