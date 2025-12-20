//! Circuit Breaker Pattern Implementation
//!
//! Prevents cascade failures when AI gateway is unavailable.
//! States: Closed (normal) -> Open (failing) -> HalfOpen (testing recovery)

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Circuit is open - requests fail fast
    Open,
    /// Testing if service recovered - limited requests allowed
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,
    /// Duration to wait before trying recovery
    pub recovery_timeout: Duration,
    /// Number of successful calls to close circuit from half-open
    pub success_threshold: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

impl CircuitBreakerConfig {
    /// Configuration for AI gateway
    pub fn for_ai_gateway() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

/// Circuit breaker for protecting downstream services
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
        }
    }

    /// Get the current circuit state
    pub fn state(&self) -> CircuitState {
        // Check if we should transition from Open to HalfOpen
        if *self.state.read().unwrap() == CircuitState::Open {
            let last_failure = self.last_failure_time.load(Ordering::Relaxed);

            // Compute elapsed since last failure using system time
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if now_millis.saturating_sub(last_failure)
                >= self.config.recovery_timeout.as_millis() as u64
            {
                let mut state = self.state.write().unwrap();
                if *state == CircuitState::Open {
                    *state = CircuitState::HalfOpen;
                    self.success_count.store(0, Ordering::Relaxed);
                    tracing::info!("Circuit breaker transitioning to HalfOpen");
                }
            }
        }
        *self.state.read().unwrap()
    }

    /// Check if request is allowed
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // Allow test requests
        }
    }

    /// Record a successful call
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);

        let state = self.state();
        if state == CircuitState::HalfOpen {
            let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
            if successes >= self.config.success_threshold {
                let mut state = self.state.write().unwrap();
                *state = CircuitState::Closed;
                self.success_count.store(0, Ordering::Relaxed);
                tracing::info!("Circuit breaker closed after successful recovery");
            }
        }
    }

    /// Record a failed call
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Update last failure time
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.last_failure_time.store(now_millis, Ordering::Relaxed);

        let state = self.state();

        if state == CircuitState::HalfOpen {
            // Any failure in half-open goes back to open
            let mut state = self.state.write().unwrap();
            *state = CircuitState::Open;
            tracing::warn!("Circuit breaker re-opened after failure in HalfOpen state");
        } else if state == CircuitState::Closed && failures >= self.config.failure_threshold {
            let mut state = self.state.write().unwrap();
            *state = CircuitState::Open;
            tracing::warn!(
                failures = failures,
                threshold = self.config.failure_threshold,
                "Circuit breaker opened due to failures"
            );
        }
    }

    /// Reset the circuit breaker (for testing)
    #[cfg(test)]
    pub fn reset(&self) {
        *self.state.write().unwrap() = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.last_failure_time.store(0, Ordering::Relaxed);
    }
}

/// Error when circuit is open
#[derive(Debug, Clone)]
pub struct CircuitOpenError;

impl std::fmt::Display for CircuitOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Circuit breaker is open - request rejected")
    }
}

impl std::error::Error for CircuitOpenError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Record failures
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new(config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // Should reset
        cb.record_failure();
        cb.record_failure();
        // Still at 2 failures, not 4
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50), // Very short for testing
            success_threshold: 1,
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(60));

        // Should transition to half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_half_open_closes_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Open circuit
        cb.record_failure();
        cb.record_failure();

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Successful calls should close it
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_opens_on_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Open circuit
        cb.record_failure();
        cb.record_failure();

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Failure in half-open goes back to open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }
}
