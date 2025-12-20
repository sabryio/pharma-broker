//! Rate Limiting Middleware
//!
//! Simple token bucket rate limiter for API protection
//! Implements Task 6.3: API Rate Limiting

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

// ============================================================================
// Configuration
// ============================================================================

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Burst capacity (allows short bursts above limit)
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,               // 100 requests
            window: Duration::from_secs(60), // per minute
            burst: 20,                       // allow burst of 20
        }
    }
}

impl RateLimitConfig {
    /// Create strict rate limit (lower limits)
    pub fn strict() -> Self {
        Self {
            max_requests: 30,
            window: Duration::from_secs(60),
            burst: 5,
        }
    }

    /// Create relaxed rate limit (higher limits)
    pub fn relaxed() -> Self {
        Self {
            max_requests: 500,
            window: Duration::from_secs(60),
            burst: 100,
        }
    }
}

// ============================================================================
// Token Bucket
// ============================================================================

/// Token bucket for a single client
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let max_tokens = (config.max_requests + config.burst) as f64;
        let refill_rate = config.max_requests as f64 / config.window.as_secs_f64();

        Self {
            tokens: max_tokens,
            last_refill: Instant::now(),
            max_tokens,
            refill_rate,
        }
    }

    fn try_consume(&mut self) -> bool {
        // Refill tokens based on time elapsed
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;

        // Try to consume a token
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn tokens_remaining(&self) -> u32 {
        self.tokens as u32
    }
}

// ============================================================================
// Rate Limiter State
// ============================================================================

/// Shared rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Check if request is allowed for the given key (e.g., IP address)
    pub async fn check(&self, key: &str) -> RateLimitResult {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));

        if bucket.try_consume() {
            RateLimitResult::Allowed {
                remaining: bucket.tokens_remaining(),
            }
        } else {
            RateLimitResult::Limited {
                retry_after: Duration::from_secs_f64(1.0 / bucket.refill_rate),
            }
        }
    }

    /// Clean up old entries (call periodically)
    pub async fn cleanup(&self, max_age: Duration) {
        let mut buckets = self.buckets.write().await;
        let cutoff = Instant::now() - max_age;

        buckets.retain(|_, bucket| bucket.last_refill > cutoff);
    }
}

/// Result of rate limit check
#[derive(Debug)]
pub enum RateLimitResult {
    Allowed { remaining: u32 },
    Limited { retry_after: Duration },
}

// ============================================================================
// Middleware
// ============================================================================

/// Rate limiting middleware for axum
///
/// Usage:
/// ```ignore
/// use axum::middleware;
///
/// let rate_limiter = RateLimiter::with_defaults();
///
/// let app = Router::new()
///     .route("/api/endpoint", get(handler))
///     .layer(middleware::from_fn_with_state(
///         rate_limiter.clone(),
///         rate_limit_middleware,
///     ));
/// ```
pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let client_ip = addr.ip().to_string();

    match limiter.check(&client_ip).await {
        RateLimitResult::Allowed { remaining } => {
            let mut response = next.run(request).await;

            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Remaining",
                remaining.to_string().parse().unwrap(),
            );

            response
        }
        RateLimitResult::Limited { retry_after } => {
            tracing::warn!(
                client_ip = %client_ip,
                retry_after_ms = retry_after.as_millis(),
                "Rate limit exceeded"
            );

            crate::metrics::record_rate_limited(&client_ip);

            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", retry_after.as_secs().to_string()),
                    ("X-RateLimit-Remaining", "0".to_string()),
                ],
                "Rate limit exceeded. Please try again later.",
            )
                .into_response()
        }
    }
}

/// Simpler rate limit middleware that extracts IP from request
/// (for when ConnectInfo is not available)
pub async fn rate_limit_by_header(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Try to get client IP from X-Forwarded-For or X-Real-IP headers
    let client_ip = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("X-Real-IP")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    match limiter.check(&client_ip).await {
        RateLimitResult::Allowed { remaining } => {
            let mut response = next.run(request).await;

            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Remaining",
                remaining.to_string().parse().unwrap(),
            );

            response
        }
        RateLimitResult::Limited { retry_after } => {
            tracing::warn!(
                client_ip = %client_ip,
                retry_after_ms = retry_after.as_millis(),
                "Rate limit exceeded"
            );

            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", retry_after.as_secs().to_string()),
                    ("X-RateLimit-Remaining", "0".to_string()),
                ],
                "Rate limit exceeded. Please try again later.",
            )
                .into_response()
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(1),
            burst: 5,
        });

        // Should allow initial requests
        for _ in 0..10 {
            match limiter.check("test-client").await {
                RateLimitResult::Allowed { .. } => {}
                RateLimitResult::Limited { .. } => panic!("Should not be limited"),
            }
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_excess() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            burst: 2,
        });

        // Consume all tokens (5 + 2 burst = 7)
        for _ in 0..7 {
            limiter.check("test-client").await;
        }

        // Next request should be limited
        match limiter.check("test-client").await {
            RateLimitResult::Limited { .. } => {} // Expected
            RateLimitResult::Allowed { .. } => panic!("Should be limited"),
        }
    }

    #[tokio::test]
    async fn test_different_clients_separate_limits() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 2,
            window: Duration::from_secs(60),
            burst: 0,
        });

        // Client 1 uses their limit
        limiter.check("client-1").await;
        limiter.check("client-1").await;

        // Client 2 should still have their own limit
        match limiter.check("client-2").await {
            RateLimitResult::Allowed { .. } => {} // Expected
            RateLimitResult::Limited { .. } => panic!("Client 2 should not be limited"),
        }
    }

    #[tokio::test]
    async fn test_cleanup_removes_old_entries() {
        let limiter = RateLimiter::new(RateLimitConfig::default());

        // Add some entries
        limiter.check("old-client").await;
        limiter.check("new-client").await;

        // Cleanup with very short max age should remove all
        limiter.cleanup(Duration::from_nanos(1)).await;

        // Buckets should be empty (new entry would be created)
        let buckets = limiter.buckets.read().await;
        assert!(buckets.is_empty());
    }
}
