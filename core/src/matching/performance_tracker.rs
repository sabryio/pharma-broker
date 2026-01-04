//! Performance Tracker for Matching Operations
//!
//! Provides utilities for tracking performance metrics during match operations,
//! including memory usage, AI timing, and database query metrics.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::matching::PipelinePerformanceMetrics;

/// Thread-safe performance tracker for a single match operation
#[derive(Debug)]
pub struct PerformanceTracker {
    /// Start time of the operation
    start_time: Instant,
    /// Peak memory usage in bytes
    memory_peak_bytes: AtomicU64,
    /// Time spent waiting in AI queue (milliseconds)
    ai_queue_wait_ms: AtomicU64,
    /// Time spent in AI processing (milliseconds)
    ai_processing_ms: AtomicU64,
    /// Number of database queries executed
    db_query_count: AtomicU64,
    /// Total time spent in database queries (milliseconds)
    db_total_ms: AtomicU64,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            memory_peak_bytes: AtomicU64::new(0),
            ai_queue_wait_ms: AtomicU64::new(0),
            ai_processing_ms: AtomicU64::new(0),
            db_query_count: AtomicU64::new(0),
            db_total_ms: AtomicU64::new(0),
        }
    }

    /// Record memory usage (keeps track of peak)
    pub fn record_memory(&self, bytes: u64) {
        self.memory_peak_bytes.fetch_max(bytes, Ordering::Relaxed);
    }

    /// Record AI queue wait time
    pub fn record_ai_queue_wait(&self, ms: u64) {
        self.ai_queue_wait_ms.fetch_add(ms, Ordering::Relaxed);
    }

    /// Record AI processing time
    pub fn record_ai_processing(&self, ms: u64) {
        self.ai_processing_ms.fetch_add(ms, Ordering::Relaxed);
    }

    /// Record a database query
    pub fn record_db_query(&self, duration_ms: u64) {
        self.db_query_count.fetch_add(1, Ordering::Relaxed);
        self.db_total_ms.fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Get elapsed time since tracker creation
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Get peak memory usage
    pub fn peak_memory_bytes(&self) -> u64 {
        self.memory_peak_bytes.load(Ordering::Relaxed)
    }

    /// Get AI queue wait time
    pub fn ai_queue_wait_ms(&self) -> u64 {
        self.ai_queue_wait_ms.load(Ordering::Relaxed)
    }

    /// Get AI processing time
    pub fn ai_processing_ms(&self) -> u64 {
        self.ai_processing_ms.load(Ordering::Relaxed)
    }

    /// Get total AI time
    pub fn total_ai_time_ms(&self) -> u64 {
        self.ai_queue_wait_ms() + self.ai_processing_ms()
    }

    /// Get database query count
    pub fn db_query_count(&self) -> u64 {
        self.db_query_count.load(Ordering::Relaxed)
    }

    /// Get total database time
    pub fn db_total_ms(&self) -> u64 {
        self.db_total_ms.load(Ordering::Relaxed)
    }

    /// Convert to PipelinePerformanceMetrics
    pub fn to_metrics(&self) -> PipelinePerformanceMetrics {
        let mut metrics = PipelinePerformanceMetrics::new();

        let peak_memory = self.peak_memory_bytes();
        if peak_memory > 0 {
            metrics.memory_peak_bytes = Some(peak_memory);
        }

        let ai_queue = self.ai_queue_wait_ms();
        if ai_queue > 0 {
            metrics.ai_queue_wait_ms = Some(ai_queue);
        }

        let ai_processing = self.ai_processing_ms();
        if ai_processing > 0 {
            metrics.ai_processing_ms = Some(ai_processing);
        }

        metrics.db_query_count = self.db_query_count() as u32;
        metrics.db_total_ms = self.db_total_ms();

        metrics
    }
}

/// Shared performance tracker that can be passed across async boundaries
pub type SharedPerformanceTracker = Arc<PerformanceTracker>;

/// Create a new shared performance tracker
pub fn new_shared_tracker() -> SharedPerformanceTracker {
    Arc::new(PerformanceTracker::new())
}

/// RAII guard for timing a stage
pub struct StageTimingGuard<'a> {
    tracker: &'a PerformanceTracker,
    stage_name: String,
    start: Instant,
    is_ai: bool,
}

impl<'a> StageTimingGuard<'a> {
    /// Create a new timing guard for a regular stage
    pub fn new(tracker: &'a PerformanceTracker, stage_name: impl Into<String>) -> Self {
        Self {
            tracker,
            stage_name: stage_name.into(),
            start: Instant::now(),
            is_ai: false,
        }
    }

    /// Create a new timing guard for an AI stage
    pub fn new_ai(tracker: &'a PerformanceTracker, stage_name: impl Into<String>) -> Self {
        Self {
            tracker,
            stage_name: stage_name.into(),
            start: Instant::now(),
            is_ai: true,
        }
    }

    /// Get the stage name
    pub fn stage_name(&self) -> &str {
        &self.stage_name
    }

    /// Get elapsed time
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

impl Drop for StageTimingGuard<'_> {
    fn drop(&mut self) {
        let elapsed = self.elapsed_ms();
        if self.is_ai {
            self.tracker.record_ai_processing(elapsed);
        }
        tracing::trace!(
            stage = %self.stage_name,
            duration_ms = elapsed,
            is_ai = self.is_ai,
            "Stage completed"
        );
    }
}

/// RAII guard for timing a database query
pub struct DbQueryGuard<'a> {
    tracker: &'a PerformanceTracker,
    start: Instant,
}

impl<'a> DbQueryGuard<'a> {
    /// Create a new database query timing guard
    pub fn new(tracker: &'a PerformanceTracker) -> Self {
        Self {
            tracker,
            start: Instant::now(),
        }
    }

    /// Get elapsed time
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

impl Drop for DbQueryGuard<'_> {
    fn drop(&mut self) {
        let elapsed = self.elapsed_ms();
        self.tracker.record_db_query(elapsed);
    }
}

/// Helper to estimate current memory usage
/// Note: This is a rough estimate and may not be accurate on all platforms
#[cfg(target_os = "linux")]
pub fn estimate_memory_usage() -> Option<u64> {
    use std::fs;

    // Read from /proc/self/statm
    if let Ok(content) = fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 2 {
            // Second field is resident set size in pages
            if let Ok(pages) = parts[1].parse::<u64>() {
                // Assume 4KB pages
                return Some(pages * 4096);
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
pub fn estimate_memory_usage() -> Option<u64> {
    // Memory estimation not available on this platform
    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_tracker_basic() {
        let tracker = PerformanceTracker::new();

        tracker.record_memory(1000);
        tracker.record_memory(2000);
        tracker.record_memory(1500); // Should not update peak

        assert_eq!(tracker.peak_memory_bytes(), 2000);
    }

    #[test]
    fn test_performance_tracker_ai_timing() {
        let tracker = PerformanceTracker::new();

        tracker.record_ai_queue_wait(10);
        tracker.record_ai_queue_wait(5);
        tracker.record_ai_processing(100);
        tracker.record_ai_processing(50);

        assert_eq!(tracker.ai_queue_wait_ms(), 15);
        assert_eq!(tracker.ai_processing_ms(), 150);
        assert_eq!(tracker.total_ai_time_ms(), 165);
    }

    #[test]
    fn test_performance_tracker_db_queries() {
        let tracker = PerformanceTracker::new();

        tracker.record_db_query(5);
        tracker.record_db_query(10);
        tracker.record_db_query(3);

        assert_eq!(tracker.db_query_count(), 3);
        assert_eq!(tracker.db_total_ms(), 18);
    }

    #[test]
    fn test_to_metrics() {
        let tracker = PerformanceTracker::new();

        tracker.record_memory(5000);
        tracker.record_ai_queue_wait(10);
        tracker.record_ai_processing(100);
        tracker.record_db_query(5);
        tracker.record_db_query(10);

        let metrics = tracker.to_metrics();

        assert_eq!(metrics.memory_peak_bytes, Some(5000));
        assert_eq!(metrics.ai_queue_wait_ms, Some(10));
        assert_eq!(metrics.ai_processing_ms, Some(100));
        assert_eq!(metrics.db_query_count, 2);
        assert_eq!(metrics.db_total_ms, 15);
    }

    #[test]
    fn test_shared_tracker() {
        let tracker = new_shared_tracker();
        let tracker2 = Arc::clone(&tracker);

        tracker.record_memory(1000);
        tracker2.record_memory(2000);

        assert_eq!(tracker.peak_memory_bytes(), 2000);
        assert_eq!(tracker2.peak_memory_bytes(), 2000);
    }

    #[test]
    fn test_stage_timing_guard() {
        let tracker = PerformanceTracker::new();

        {
            let _guard = StageTimingGuard::new_ai(&tracker, "test_stage");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // AI processing time should be recorded
        assert!(tracker.ai_processing_ms() >= 10);
    }

    #[test]
    fn test_db_query_guard() {
        let tracker = PerformanceTracker::new();

        {
            let _guard = DbQueryGuard::new(&tracker);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(tracker.db_query_count(), 1);
        assert!(tracker.db_total_ms() >= 5);
    }
}
