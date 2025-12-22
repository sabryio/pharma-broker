//! Parsing configuration
//!
//! Ported from legacy/parsing/interface.go

use std::time::Duration;

/// Parse pass identifier for multi-pass parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParsePass {
    /// First pass with strict prompts
    #[default]
    Strict,
    /// Second pass with relaxed prompts (fallback)
    Relaxed,
}

/// Batch processing configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum messages per batch
    pub batch_size: usize,
    /// Maximum time to wait before processing a partial batch
    pub batch_timeout: Duration,
    /// Number of worker tasks
    pub worker_count: usize,
    /// Channel buffer size for incoming messages
    pub channel_buffer: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            batch_timeout: Duration::from_secs(5),
            worker_count: 2,
            channel_buffer: 100,
        }
    }
}

impl BatchConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            batch_size: std::env::var("PARSING_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            batch_timeout: Duration::from_secs(
                std::env::var("PARSING_BATCH_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5),
            ),
            worker_count: std::env::var("PARSING_WORKERS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            channel_buffer: std::env::var("PARSING_CHANNEL_BUFFER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
        }
    }
}

/// Multi-pass parsing configuration
#[derive(Debug, Clone)]
pub struct MultiPassConfig {
    /// Minimum average confidence to accept Pass 1 results
    pub strict_min_confidence: f64,
    /// Minimum confidence to accept Pass 2 results
    pub relaxed_min_confidence: f64,
    /// Enable the relaxed fallback pass
    pub enable_pass2: bool,
    /// Enable queuing low-confidence results for review
    pub enable_review_queue: bool,
}

impl Default for MultiPassConfig {
    fn default() -> Self {
        Self {
            strict_min_confidence: 0.7,
            relaxed_min_confidence: 0.4,
            enable_pass2: true,
            enable_review_queue: true,
        }
    }
}

impl MultiPassConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            strict_min_confidence: std::env::var("PARSING_STRICT_CONFIDENCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.7),
            relaxed_min_confidence: std::env::var("PARSING_RELAXED_CONFIDENCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.4),
            enable_pass2: std::env::var("PARSING_ENABLE_PASS2")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            enable_review_queue: std::env::var("PARSING_ENABLE_REVIEW")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
        }
    }

    /// Check if result needs pass 2 retry based on confidence
    pub fn needs_pass2(&self, avg_confidence: f64) -> bool {
        self.enable_pass2 && avg_confidence < self.strict_min_confidence
    }

    /// Check if result should be queued for review
    pub fn needs_review(&self, avg_confidence: f64) -> bool {
        self.enable_review_queue && avg_confidence < self.relaxed_min_confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_defaults() {
        let config = BatchConfig::default();
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.batch_timeout, Duration::from_secs(5));
        assert_eq!(config.worker_count, 2);
        assert_eq!(config.channel_buffer, 100);
    }

    #[test]
    fn test_multi_pass_config_defaults() {
        let config = MultiPassConfig::default();
        assert!((config.strict_min_confidence - 0.7).abs() < 0.001);
        assert!((config.relaxed_min_confidence - 0.4).abs() < 0.001);
        assert!(config.enable_pass2);
        assert!(config.enable_review_queue);
    }

    #[test]
    fn test_needs_pass2_below_threshold() {
        let config = MultiPassConfig::default();
        // Below 0.7 => needs pass 2
        assert!(config.needs_pass2(0.5));
        assert!(config.needs_pass2(0.6));
        assert!(config.needs_pass2(0.69));
    }

    #[test]
    fn test_needs_pass2_above_threshold() {
        let config = MultiPassConfig::default();
        // At or above 0.7 => no pass 2 needed
        assert!(!config.needs_pass2(0.7));
        assert!(!config.needs_pass2(0.8));
        assert!(!config.needs_pass2(1.0));
    }

    #[test]
    fn test_needs_pass2_disabled() {
        let config = MultiPassConfig {
            enable_pass2: false,
            ..Default::default()
        };
        // Even low confidence doesn't trigger pass 2 when disabled
        assert!(!config.needs_pass2(0.3));
    }

    #[test]
    fn test_needs_review_below_threshold() {
        let config = MultiPassConfig::default();
        // Below 0.4 => needs review
        assert!(config.needs_review(0.2));
        assert!(config.needs_review(0.39));
    }

    #[test]
    fn test_needs_review_above_threshold() {
        let config = MultiPassConfig::default();
        // At or above 0.4 => no review needed
        assert!(!config.needs_review(0.4));
        assert!(!config.needs_review(0.5));
        assert!(!config.needs_review(1.0));
    }

    #[test]
    fn test_needs_review_disabled() {
        let config = MultiPassConfig {
            enable_review_queue: false,
            ..Default::default()
        };
        // Even low confidence doesn't trigger review when disabled
        assert!(!config.needs_review(0.1));
    }

    #[test]
    fn test_parse_pass_default() {
        let pass: ParsePass = Default::default();
        assert_eq!(pass, ParsePass::Strict);
    }

    #[test]
    fn test_parse_pass_equality() {
        assert_eq!(ParsePass::Strict, ParsePass::Strict);
        assert_eq!(ParsePass::Relaxed, ParsePass::Relaxed);
        assert_ne!(ParsePass::Strict, ParsePass::Relaxed);
    }
}
