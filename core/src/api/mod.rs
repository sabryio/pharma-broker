//! API module - REST endpoints using axum
//!
//! Ported from legacy/api/handlers/*.go

pub mod ai_health;
pub mod analytics;
pub mod audit_records;
pub mod audit_trail;
pub mod calibration;
pub mod confidence;
pub mod curation;
pub mod diagnostics;
pub mod embedding_cache;
pub mod groups;
pub mod handlers;
pub mod match_filter;
pub mod match_reviews;
pub mod matching;
pub mod messaging;
pub mod middleware;
pub mod participants;
pub mod pipeline_visualization;
pub mod priority_medications;
pub mod rate_limit;
pub mod raw_messages;
pub mod reclassify;
pub mod reparse;
pub mod review_queue;
pub mod routes;
pub mod supervision;
pub mod uncertainty;
pub mod weights;

pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use routes::create_router;

// Re-export pipeline visualization types for testing
pub use pipeline_visualization::{
    AiReviewVisualization, CalibrationVisualization, CandidateVisualization,
    ConsensusVisualization, ContrastiveVisualization, HierarchicalStageVisualization,
    ModelResultVisualization, PerformanceMetricsVisualization, PipelineStageVisualization,
    PipelineVisualizationResponse, ResolutionStageVisualization, ResolutionVisualization,
    ScoreBreakdownVisualization, ScoreComponentVisualization, TokenUsageVisualization,
    transform_to_visualization,
};
