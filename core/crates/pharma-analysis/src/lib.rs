//! # PharmaBroker Data Analysis
//!
//! Rust port of Python analysis scripts for data quality assessment.
//!
//! ## Modules
//! - `health` - Database health & schema discovery
//! - `quality` - Data quality analysis
//! - `integrity` - Referential integrity checks
//! - `business` - Business logic validation
//! - `timeseries` - Time series analysis
//! - `ai_quality` - AI parsing quality
//! - `matching` - Matching engine analysis
//! - `stale` - Stale matches analysis

pub mod ai_quality;
pub mod business;
pub mod health;
pub mod integrity;
pub mod matching;
pub mod quality;
pub mod stale;
pub mod timeseries;

use async_trait::async_trait;
use pharma_db::DatabaseConnection;
use std::path::PathBuf;

/// Common trait for all analysis phases
#[async_trait]
pub trait AnalysisPhase: Send + Sync {
    /// Name of the phase
    fn name(&self) -> &'static str;

    /// Run the analysis
    async fn run(&self, db: &DatabaseConnection) -> anyhow::Result<()>;
}

/// Configuration for analysis
pub struct AnalysisConfig {
    pub reports_dir: PathBuf,
    pub verbose: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            reports_dir: PathBuf::from("reports"),
            verbose: false,
        }
    }
}

/// Helper to print headers
pub fn print_header(text: &str) {
    println!("\n{}", "=".repeat(60));
    println!("🔍 {}", text);
    println!("{}", "=".repeat(60));
}

/// Helper to print sub-headers
pub fn print_subheader(text: &str) {
    println!("\n{}", "-".repeat(40));
    println!("📋 {}", text);
    println!("{}", "-".repeat(40));
}

/// Format number with thousands separator
pub fn fmt_num(n: i64) -> String {
    use num_format::{Locale, ToFormattedString};
    n.to_formatted_string(&Locale::en)
}
