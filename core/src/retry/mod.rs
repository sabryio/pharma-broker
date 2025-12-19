//! Retry Logic Module
//!
//! Provides retry utilities with exponential backoff and jitter
//! for handling transient failures gracefully.

use std::future::Future;
use std::time::Duration;

use rand::Rng;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff (e.g., 2.0 doubles the delay each time)
    pub multiplier: f64,
    /// Whether to add random jitter to delays
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a config for AI gateway calls
    pub fn for_ai_gateway() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Create a config for database operations
    pub fn for_database() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            multiplier: 1.5,
            jitter: true,
        }
    }

    /// Create a config for gRPC calls
    pub fn for_grpc() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Result of a retry operation
#[derive(Debug)]
pub struct RetryResult<T, E> {
    /// The final result (success or last error)
    pub result: Result<T, E>,
    /// Number of attempts made
    pub attempts: u32,
    /// Total time spent retrying
    pub total_duration: Duration,
}

/// Execute an operation with retry logic
///
/// # Arguments
/// * `config` - Retry configuration
/// * `operation` - Async function to retry
/// * `should_retry` - Function to determine if an error is retryable
///
/// # Example
/// ```ignore
/// let result = with_retry(
///     RetryConfig::default(),
///     || async { fetch_data().await },
///     |e| matches!(e, Error::NetworkError(_)),
/// ).await;
/// ```
pub async fn with_retry<T, E, F, Fut, R>(
    config: RetryConfig,
    mut operation: F,
    should_retry: R,
) -> RetryResult<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
    E: std::fmt::Debug,
{
    let start = std::time::Instant::now();
    let mut attempts = 0;
    let mut current_delay = config.initial_delay;

    loop {
        attempts += 1;

        match operation().await {
            Ok(value) => {
                return RetryResult {
                    result: Ok(value),
                    attempts,
                    total_duration: start.elapsed(),
                };
            }
            Err(e) => {
                // Check if we've exhausted all attempts
                if attempts >= config.max_attempts {
                    tracing::warn!(
                        attempts = attempts,
                        error = ?e,
                        "Retry exhausted, returning last error"
                    );
                    return RetryResult {
                        result: Err(e),
                        attempts,
                        total_duration: start.elapsed(),
                    };
                }

                // Check if this error is retryable
                if !should_retry(&e) {
                    tracing::debug!(
                        error = ?e,
                        "Error is not retryable, returning immediately"
                    );
                    return RetryResult {
                        result: Err(e),
                        attempts,
                        total_duration: start.elapsed(),
                    };
                }

                // Calculate delay with optional jitter
                let delay = if config.jitter {
                    add_jitter(current_delay)
                } else {
                    current_delay
                };

                tracing::info!(
                    attempt = attempts,
                    max_attempts = config.max_attempts,
                    delay_ms = delay.as_millis(),
                    error = ?e,
                    "Retrying after transient failure"
                );

                // Wait before retrying
                tokio::time::sleep(delay).await;

                // Calculate next delay (exponential backoff)
                current_delay = Duration::from_secs_f64(
                    (current_delay.as_secs_f64() * config.multiplier)
                        .min(config.max_delay.as_secs_f64()),
                );
            }
        }
    }
}

/// Add random jitter to a duration (±25%)
fn add_jitter(duration: Duration) -> Duration {
    let mut rng = rand::rng();
    let jitter_factor: f64 = rng.random_range(0.75..1.25);
    Duration::from_secs_f64(duration.as_secs_f64() * jitter_factor)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, Clone, PartialEq)]
    enum TestError {
        Retryable,
        NotRetryable,
    }

    #[tokio::test]
    async fn test_retry_succeeds_first_attempt() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let result = with_retry(config, || async { Ok::<_, TestError>("success") }, |_| true).await;

        assert!(result.result.is_ok());
        assert_eq!(result.attempts, 1);
        assert_eq!(result.result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_third_attempt() {
        let counter = Arc::new(AtomicU32::new(0));

        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let counter_clone = Arc::clone(&counter);
        let result = with_retry(
            config,
            || {
                let c = Arc::clone(&counter_clone);
                async move {
                    let attempt = c.fetch_add(1, Ordering::SeqCst) + 1;
                    if attempt < 3 {
                        Err(TestError::Retryable)
                    } else {
                        Ok("success on third")
                    }
                }
            },
            |e| matches!(e, TestError::Retryable),
        )
        .await;

        assert!(result.result.is_ok());
        assert_eq!(result.attempts, 3);
        assert_eq!(result.result.unwrap(), "success on third");
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));

        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let counter_clone = Arc::clone(&counter);
        let result = with_retry(
            config,
            || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(TestError::Retryable)
                }
            },
            |e| matches!(e, TestError::Retryable),
        )
        .await;

        assert!(result.result.is_err());
        assert_eq!(result.attempts, 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_not_retryable() {
        let counter = Arc::new(AtomicU32::new(0));

        let config = RetryConfig::default();

        let counter_clone = Arc::clone(&counter);
        let result = with_retry(
            config,
            || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(TestError::NotRetryable)
                }
            },
            |e| matches!(e, TestError::Retryable), // Only retry Retryable errors
        )
        .await;

        assert!(result.result.is_err());
        assert_eq!(result.attempts, 1); // Should not retry
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let config = RetryConfig {
            max_attempts: 4,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: false,
        };

        let start = std::time::Instant::now();

        let _result = with_retry(
            config,
            || async { Err::<(), _>(TestError::Retryable) },
            |_| true,
        )
        .await;

        let elapsed = start.elapsed();

        // Should have waited approximately: 100ms + 200ms + 400ms = 700ms
        // Allow some variance for test execution
        assert!(elapsed >= Duration::from_millis(600));
        assert!(elapsed < Duration::from_millis(1000));
    }

    #[test]
    fn test_retry_config_presets() {
        let ai = RetryConfig::for_ai_gateway();
        assert_eq!(ai.max_attempts, 3);
        assert_eq!(ai.initial_delay, Duration::from_millis(500));

        let db = RetryConfig::for_database();
        assert_eq!(db.max_attempts, 5);
        assert_eq!(db.initial_delay, Duration::from_millis(50));

        let grpc = RetryConfig::for_grpc();
        assert_eq!(grpc.max_attempts, 3);
        assert_eq!(grpc.initial_delay, Duration::from_millis(200));
    }
}
