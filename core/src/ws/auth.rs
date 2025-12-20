//! WebSocket authentication configuration and token validation

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// WebSocket authentication configuration
#[derive(Debug, Clone)]
pub struct WsAuthConfig {
    /// Whether authentication is enabled
    pub enabled: bool,
    /// Secret key for HMAC token validation
    pub secret: String,
    /// Query parameter name for token (default: "token")
    pub token_param: String,
    /// Maximum concurrent WebSocket connections
    pub max_connections: usize,
    /// Heartbeat ping interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Client inactivity timeout in seconds (disconnect if no pong)
    pub inactivity_timeout_secs: u64,
}

impl Default for WsAuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            secret: std::env::var("WS_SECRET")
                .unwrap_or_else(|_| "default-ws-secret-change-me".to_string()),
            token_param: "token".to_string(),
            max_connections: 100,
            heartbeat_interval_secs: 30,
            inactivity_timeout_secs: 300, // 5 minutes
        }
    }
}

impl WsAuthConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("WS_AUTH_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            secret: std::env::var("WS_SECRET")
                .unwrap_or_else(|_| "default-ws-secret-change-me".to_string()),
            token_param: std::env::var("WS_TOKEN_PARAM").unwrap_or_else(|_| "token".to_string()),
            max_connections: std::env::var("WS_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            heartbeat_interval_secs: std::env::var("WS_HEARTBEAT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            inactivity_timeout_secs: std::env::var("WS_INACTIVITY_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        }
    }

    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.heartbeat_interval_secs)
    }

    /// Get inactivity timeout as Duration
    pub fn inactivity_timeout(&self) -> Duration {
        Duration::from_secs(self.inactivity_timeout_secs)
    }
}

/// Token claims extracted from validated token
#[derive(Debug, Clone)]
pub struct TokenClaims {
    pub user_id: String,
    pub scopes: Vec<String>,
    pub expires_at: u64,
}

impl TokenClaims {
    /// Check if token has a specific scope
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == "*" || s == scope)
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.expires_at > 0 && now > self.expires_at
    }
}

/// Token validation error
#[derive(Debug, Clone)]
pub enum TokenError {
    Missing,
    Invalid,
    Expired,
    InvalidSignature,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Missing => write!(f, "missing token"),
            TokenError::Invalid => write!(f, "invalid token format"),
            TokenError::Expired => write!(f, "token expired"),
            TokenError::InvalidSignature => write!(f, "invalid token signature"),
        }
    }
}

/// Validate a simple token (legacy mode - just compare with WS_TOKEN)
pub fn validate_simple_token(token: &str) -> Result<TokenClaims, TokenError> {
    let expected = std::env::var("WS_TOKEN").unwrap_or_else(|_| "secret-token".to_string());

    if token == expected {
        Ok(TokenClaims {
            user_id: "anonymous".to_string(),
            scopes: vec!["*".to_string()],
            expires_at: 0, // Never expires
        })
    } else {
        Err(TokenError::Invalid)
    }
}

/// Validate an HMAC-signed token
/// Token format: "user_id:scope1,scope2:expires_at:signature"
pub fn validate_hmac_token(token: &str, secret: &str) -> Result<TokenClaims, TokenError> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 4 {
        return Err(TokenError::Invalid);
    }

    let user_id = parts[0];
    let scopes_str = parts[1];
    let expires_str = parts[2];
    let signature = parts[3];

    // Verify signature
    let message = format!("{}:{}:{}", user_id, scopes_str, expires_str);
    let expected_sig = compute_hmac(secret, &message);

    if signature != expected_sig {
        return Err(TokenError::InvalidSignature);
    }

    // Parse expiration
    let expires_at: u64 = expires_str.parse().map_err(|_| TokenError::Invalid)?;

    // Check expiration
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if expires_at > 0 && now > expires_at {
        return Err(TokenError::Expired);
    }

    // Parse scopes
    let scopes: Vec<String> = scopes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(TokenClaims {
        user_id: user_id.to_string(),
        scopes,
        expires_at,
    })
}

/// Compute HMAC-SHA256 signature (hex encoded)
fn compute_hmac(secret: &str, message: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Generate a signed token (for testing/admin use)
pub fn generate_token(secret: &str, user_id: &str, scopes: &[&str], expires_at: u64) -> String {
    let scopes_str = scopes.join(",");
    let message = format!("{}:{}:{}", user_id, scopes_str, expires_at);
    let signature = compute_hmac(secret, &message);
    format!("{}:{}:{}:{}", user_id, scopes_str, expires_at, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_token() {
        let secret = "test-secret";
        let token = generate_token(secret, "user123", &["read", "write"], 0);

        let claims = validate_hmac_token(&token, secret).expect("Should validate");
        assert_eq!(claims.user_id, "user123");
        assert!(claims.has_scope("read"));
        assert!(claims.has_scope("write"));
        assert!(!claims.has_scope("admin"));
    }

    #[test]
    fn test_wildcard_scope() {
        let claims = TokenClaims {
            user_id: "admin".to_string(),
            scopes: vec!["*".to_string()],
            expires_at: 0,
        };

        assert!(claims.has_scope("anything"));
        assert!(claims.has_scope("read"));
        assert!(claims.has_scope("admin"));
    }

    #[test]
    fn test_invalid_signature() {
        let token = "user:read:0:invalid_signature";
        let result = validate_hmac_token(token, "secret");
        assert!(matches!(result, Err(TokenError::InvalidSignature)));
    }

    #[test]
    fn test_expired_token() {
        let secret = "test-secret";
        // Token that expired 1 hour ago
        let expired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 3600;

        let token = generate_token(secret, "user", &["read"], expired_at);
        let result = validate_hmac_token(&token, secret);
        assert!(matches!(result, Err(TokenError::Expired)));
    }
}
