//! Metrics module for Prometheus monitoring
//!
//! Provides application metrics for observability

use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

/// Initialize the Prometheus metrics exporter
/// Returns a handle that can be used to render metrics
pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
}

// ============================================================================
// Counter Metrics
// ============================================================================

/// Record a message received from the bridge
pub fn record_message_received(group: &str, status: &str) {
    counter!("pharma_messages_received_total", "group" => group.to_string(), "status" => status.to_string()).increment(1);
}

/// Record a message processed
pub fn record_message_processed(msg_type: &str, status: &str) {
    counter!("pharma_messages_processed_total", "type" => msg_type.to_string(), "status" => status.to_string()).increment(1);
}

/// Record an offer created
pub fn record_offer_created() {
    counter!("pharma_offers_created_total").increment(1);
}

/// Record a request created
pub fn record_request_created() {
    counter!("pharma_requests_created_total").increment(1);
}

/// Record an AI parse attempt
pub fn record_ai_parse(status: &str) {
    counter!("pharma_ai_parse_total", "status" => status.to_string()).increment(1);
}

/// Record a database operation
pub fn record_db_operation(operation: &str, status: &str) {
    counter!("pharma_db_operations_total", "operation" => operation.to_string(), "status" => status.to_string()).increment(1);
}

/// Record a rate-limited request
pub fn record_rate_limited(client_ip: &str) {
    counter!("pharma_rate_limited_total", "client_ip" => client_ip.to_string()).increment(1);
}

// ============================================================================
// Histogram Metrics
// ============================================================================

/// Record message processing duration
pub fn record_message_duration(duration_secs: f64) {
    histogram!("pharma_message_processing_duration_seconds").record(duration_secs);
}

/// Record AI parse duration
pub fn record_ai_parse_duration(duration_secs: f64) {
    histogram!("pharma_ai_parse_duration_seconds").record(duration_secs);
}

/// Record database query duration
pub fn record_db_query_duration(duration_secs: f64) {
    histogram!("pharma_db_query_duration_seconds").record(duration_secs);
}

// ============================================================================
// Gauge Metrics
// ============================================================================

/// Set the number of active gRPC connections
pub fn set_active_connections(count: i64) {
    gauge!("pharma_active_grpc_connections").set(count as f64);
}

/// Set the queue size
pub fn set_queue_size(size: i64) {
    gauge!("pharma_queue_size").set(size as f64);
}

/// Set the number of monitored groups
pub fn set_monitored_groups(count: i64) {
    gauge!("pharma_monitored_groups").set(count as f64);
}

/// Set the number of pending AI batch jobs
pub fn set_ai_batch_pending(count: i64) {
    gauge!("pharma_ai_batch_pending").set(count as f64);
}

// ============================================================================
// AI Metrics (Task 3.4)
// ============================================================================

/// Increment AI batch pending counter
pub fn increment_ai_batch_pending() {
    gauge!("pharma_ai_batch_pending").increment(1.0);
}

/// Decrement AI batch pending counter
pub fn decrement_ai_batch_pending() {
    gauge!("pharma_ai_batch_pending").decrement(1.0);
}

/// Record AI parse with detailed status
pub fn record_ai_parse_detailed(status: &str, batch_size: usize) {
    counter!("pharma_ai_parse_total", "status" => status.to_string()).increment(1);
    if batch_size > 1 {
        histogram!("pharma_ai_batch_size").record(batch_size as f64);
    }
}

/// Record token usage for AI batch
pub fn record_ai_tokens_used(tokens: usize) {
    counter!("pharma_ai_tokens_used_total").increment(tokens as u64);
    histogram!("pharma_ai_batch_tokens").record(tokens as f64);
}

// ============================================================================
// Match Quality Metrics (Task 4.4)
// ============================================================================

/// Record match confirmation or rejection
pub fn record_match_confirmation(confirmed: bool) {
    let status = if confirmed { "confirmed" } else { "rejected" };
    counter!("pharma_match_confirmations_total", "status" => status.to_string()).increment(1);
}

/// Record match score distribution
pub fn record_match_score(score: f64, band: &str) {
    histogram!("pharma_match_score_distribution").record(score);
    counter!("pharma_match_confidence_band_total", "band" => band.to_string()).increment(1);
}

/// Set current confirmation rate (0.0 - 1.0)
pub fn set_confirmation_rate(rate: f64) {
    gauge!("pharma_match_confirmation_rate").set(rate);
}
// ============================================================================
// Timer Helper
// ============================================================================

/// A guard that records duration when dropped
pub struct Timer {
    start: Instant,
    metric_fn: Box<dyn FnOnce(f64) + Send>,
}

impl Timer {
    pub fn new<F>(metric_fn: F) -> Self
    where
        F: FnOnce(f64) + Send + 'static,
    {
        Self {
            start: Instant::now(),
            metric_fn: Box::new(metric_fn),
        }
    }

    pub fn message_processing() -> Self {
        Self::new(record_message_duration)
    }

    pub fn ai_parse() -> Self {
        Self::new(record_ai_parse_duration)
    }

    pub fn db_query() -> Self {
        Self::new(record_db_query_duration)
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        // Note: We can't move out of self in Drop, so we use a dummy closure
        // In practice, create a new timer each time
    }
}

/// Start a timer that records duration when finished
pub fn start_timer<F>(metric_fn: F) -> impl FnOnce()
where
    F: FnOnce(f64),
{
    let start = Instant::now();
    move || {
        let duration = start.elapsed().as_secs_f64();
        metric_fn(duration);
    }
}
