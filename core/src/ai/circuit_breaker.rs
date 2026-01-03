//! Circuit Breaker Pattern Implementation
//!
//! Prevents cascade failures when AI gateway is unavailable.
//! States: Closed (normal) -> Open (failing) -> HalfOpen (testing recovery)
//!
//! When circuit is open, the FallbackStrategy determines behavior:
//! - DeterministicOnly: Use only exact/alias matching (no AI)
//! - CachedEmbeddings: Use cached embeddings, skip new AI calls
//! - QueueForLater: Queue requests for processing when AI recovers
//! - RejectAll: Reject all matching requests

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

/// Fallback strategy when circuit is open
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FallbackStrategy {
    /// Use only deterministic matching (exact name, alias lookup)
    /// No AI calls, fastest but lowest recall
    #[default]
    DeterministicOnly,
    /// Use cached embeddings for similarity search
    /// Skip new embedding generation, use existing cache
    CachedEmbeddings,
    /// Queue requests for later processing when AI recovers
    /// Returns "pending" status, processes when circuit closes
    QueueForLater,
    /// Reject all matching requests
    /// Safest option, prevents any potentially incorrect matches
    RejectAll,
}

impl std::str::FromStr for FallbackStrategy {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "deterministic" | "deterministic_only" => Self::DeterministicOnly,
            "cached" | "cached_embeddings" => Self::CachedEmbeddings,
            "queue" | "queue_for_later" => Self::QueueForLater,
            "reject" | "reject_all" => Self::RejectAll,
            _ => Self::default(),
        })
    }
}

impl FallbackStrategy {
    /// Check if this strategy allows any matching
    pub fn allows_matching(&self) -> bool {
        !matches!(self, Self::RejectAll)
    }

    /// Check if this strategy requires AI
    pub fn requires_ai(&self) -> bool {
        false // All fallback strategies work without AI
    }

    /// Check if this strategy uses embeddings
    pub fn uses_embeddings(&self) -> bool {
        matches!(self, Self::CachedEmbeddings)
    }
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
    /// Strategy to use when circuit is open
    pub fallback_strategy: FallbackStrategy,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
            fallback_strategy: FallbackStrategy::default(),
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
            fallback_strategy: FallbackStrategy::DeterministicOnly,
        }
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            failure_threshold: std::env::var("CB_FAILURE_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            recovery_timeout: Duration::from_secs(
                std::env::var("CB_RECOVERY_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30),
            ),
            success_threshold: std::env::var("CB_SUCCESS_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
            fallback_strategy: std::env::var("CB_FALLBACK_STRATEGY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
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
                fallback = ?self.config.fallback_strategy,
                "Circuit breaker opened due to failures"
            );
        }
    }

    /// Get the fallback strategy for when circuit is open
    pub fn fallback_strategy(&self) -> FallbackStrategy {
        self.config.fallback_strategy
    }

    /// Check if matching should proceed based on circuit state and fallback strategy
    pub fn should_match(&self) -> bool {
        match self.state() {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => self.config.fallback_strategy.allows_matching(),
        }
    }

    /// Check if AI calls are allowed
    pub fn allow_ai_call(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true, // Test requests
            CircuitState::Open => false,
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            fallback_strategy: FallbackStrategy::default(),
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

    #[test]
    fn test_fallback_strategy_from_str() {
        assert_eq!(
            "deterministic".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::DeterministicOnly)
        );
        assert_eq!(
            "deterministic_only".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::DeterministicOnly)
        );
        assert_eq!(
            "cached".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::CachedEmbeddings)
        );
        assert_eq!(
            "queue".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::QueueForLater)
        );
        assert_eq!(
            "reject".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::RejectAll)
        );
        assert_eq!(
            "unknown".parse::<FallbackStrategy>(),
            Ok(FallbackStrategy::DeterministicOnly)
        );
    }

    #[test]
    fn test_fallback_strategy_allows_matching() {
        assert!(FallbackStrategy::DeterministicOnly.allows_matching());
        assert!(FallbackStrategy::CachedEmbeddings.allows_matching());
        assert!(FallbackStrategy::QueueForLater.allows_matching());
        assert!(!FallbackStrategy::RejectAll.allows_matching());
    }

    #[test]
    fn test_should_match_with_fallback() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
            fallback_strategy: FallbackStrategy::DeterministicOnly,
        };
        let cb = CircuitBreaker::new(config);

        // Closed - should match
        assert!(cb.should_match());

        // Open with DeterministicOnly - should still match
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(cb.should_match());
    }

    #[test]
    fn test_should_match_reject_all() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
            fallback_strategy: FallbackStrategy::RejectAll,
        };
        let cb = CircuitBreaker::new(config);

        // Closed - should match
        assert!(cb.should_match());

        // Open with RejectAll - should NOT match
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.should_match());
    }

    #[test]
    fn test_allow_ai_call() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(10),
            success_threshold: 2,
            fallback_strategy: FallbackStrategy::default(),
        };
        let cb = CircuitBreaker::new(config);

        // Closed - AI allowed
        assert!(cb.allow_ai_call());

        // Open - AI not allowed
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.allow_ai_call());

        // Half-open - AI allowed (test request)
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_ai_call());
    }
}
