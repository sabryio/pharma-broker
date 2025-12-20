//! Retry configuration and logic

use std::time::Duration;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Jitter factor (0.0 to 1.0)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            jitter: 0.1,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for the given attempt number (1-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_ms = self.base_delay.as_millis() as f64;
        let exponential = base_ms * 2_f64.powi(attempt as i32 - 1);
        let capped = exponential.min(self.max_delay.as_millis() as f64);

        // Add jitter
        let jitter_range = capped * self.jitter;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
        let final_ms = (capped + jitter).max(0.0);

        Duration::from_millis(final_ms as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let config = RetryConfig {
            jitter: 0.0, // Disable jitter for predictable testing
            ..Default::default()
        };

        let d1 = config.delay_for_attempt(1);
        let d2 = config.delay_for_attempt(2);
        let d3 = config.delay_for_attempt(3);

        assert_eq!(d1, Duration::from_millis(500));
        assert_eq!(d2, Duration::from_millis(1000));
        assert_eq!(d3, Duration::from_millis(2000));
    }

    #[test]
    fn test_max_delay_cap() {
        let config = RetryConfig {
            max_delay: Duration::from_secs(1),
            jitter: 0.0,
            ..Default::default()
        };

        let d10 = config.delay_for_attempt(10);
        assert_eq!(d10, Duration::from_secs(1));
    }
}
