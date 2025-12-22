//! JWT Authentication middleware for Axum
//!
//! Provides JWT token validation, claims extraction, and role/scope-based access control.
//! Ported from legacy/api/middleware/jwt.go

use axum::{
    extract::{FromRequestParts, Request},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for signing/verifying tokens (min 32 chars)
    pub secret: String,
    /// Token issuer claim
    pub issuer: String,
    /// Token audience claim  
    pub audience: String,
    /// Access token expiry duration
    pub token_expiry: Duration,
    /// Refresh token expiry duration
    pub refresh_expiry: Duration,
    /// Paths that skip authentication
    pub skip_paths: Vec<String>,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "change-me-in-production-32-chars!".to_string()),
            issuer: "pharmabroker".to_string(),
            audience: "pharmabroker-api".to_string(),
            token_expiry: Duration::from_secs(24 * 60 * 60), // 24 hours
            refresh_expiry: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            skip_paths: vec![
                "/health".to_string(),
                "/health/live".to_string(),
                "/health/ready".to_string(),
                "/metrics".to_string(),
            ],
        }
    }
}

impl JwtConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "change-me-in-production-32-chars!".to_string()),
            issuer: std::env::var("JWT_ISSUER").unwrap_or_else(|_| "pharmabroker".to_string()),
            audience: std::env::var("JWT_AUDIENCE")
                .unwrap_or_else(|_| "pharmabroker-api".to_string()),
            token_expiry: Duration::from_secs(
                std::env::var("JWT_EXPIRY_HOURS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(24)
                    * 3600,
            ),
            refresh_expiry: Duration::from_secs(
                std::env::var("JWT_REFRESH_DAYS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(7)
                    * 86400,
            ),
            skip_paths: vec![
                "/health".to_string(),
                "/health/live".to_string(),
                "/health/ready".to_string(),
                "/metrics".to_string(),
            ],
        }
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Username
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// User role
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Granted scopes
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
    /// Issued at (Unix timestamp)
    pub iat: u64,
}

impl Claims {
    /// Check if claims have a specific scope
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == "*" || s == scope)
    }

    /// Check if claims have a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.role.as_ref().map(|r| r == role).unwrap_or(false)
    }

    /// Check if claims have any of the specified roles
    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        self.role
            .as_ref()
            .map(|r| roles.iter().any(|role| r == *role))
            .unwrap_or(false)
    }
}

/// JWT authentication error
#[derive(Debug)]
pub enum JwtError {
    Missing,
    Invalid(String),
    Expired,
    WrongIssuer,
    WrongAudience,
    InsufficientRole,
    InsufficientScope,
}

impl IntoResponse for JwtError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            JwtError::Missing => (StatusCode::UNAUTHORIZED, "Missing authorization token"),
            JwtError::Invalid(_) => (StatusCode::UNAUTHORIZED, "Invalid token"),
            JwtError::Expired => (StatusCode::UNAUTHORIZED, "Token expired"),
            JwtError::WrongIssuer => (StatusCode::UNAUTHORIZED, "Invalid token issuer"),
            JwtError::WrongAudience => (StatusCode::UNAUTHORIZED, "Invalid token audience"),
            JwtError::InsufficientRole => (StatusCode::FORBIDDEN, "Insufficient role"),
            JwtError::InsufficientScope => (StatusCode::FORBIDDEN, "Insufficient scope"),
        };
        (status, message).into_response()
    }
}

/// JWT authentication service
#[derive(Clone)]
pub struct JwtAuth {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtAuth {
    /// Create a new JWT authenticator
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }

    /// Generate a new access token
    pub fn generate_token(
        &self,
        user_id: &str,
        username: Option<&str>,
        role: Option<&str>,
        scopes: Vec<String>,
    ) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.map(String::from),
            role: role.map(String::from),
            scopes,
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            exp: now + self.config.token_expiry.as_secs(),
            iat: now,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| JwtError::Invalid(e.to_string()))
    }

    /// Generate a refresh token (longer expiry)
    pub fn generate_refresh_token(&self, user_id: &str) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: user_id.to_string(),
            username: None,
            role: None,
            scopes: vec!["refresh".to_string()],
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            exp: now + self.config.refresh_expiry.as_secs(),
            iat: now,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| JwtError::Invalid(e.to_string()))
    }

    /// Validate a token and extract claims
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            if e.to_string().contains("ExpiredSignature") {
                JwtError::Expired
            } else {
                JwtError::Invalid(e.to_string())
            }
        })?;

        Ok(token_data.claims)
    }

    /// Check if a path should skip authentication
    pub fn should_skip(&self, path: &str) -> bool {
        self.config.skip_paths.iter().any(|p| path.starts_with(p))
    }

    /// Extract token from Authorization header
    pub fn extract_token(auth_header: &str) -> Option<&str> {
        auth_header.strip_prefix("Bearer ")
    }
}

/// Axum extractor for JWT claims
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = JwtError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get JWT auth from extensions (set by middleware)
        let claims = parts
            .extensions
            .get::<Claims>()
            .cloned()
            .ok_or(JwtError::Missing)?;

        Ok(AuthUser(claims))
    }
}

/// JWT authentication middleware
pub async fn jwt_middleware(
    jwt_auth: axum::extract::State<JwtAuth>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip authentication for certain paths
    if jwt_auth.should_skip(path) {
        return next.run(request).await;
    }

    // Extract token from Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) => match JwtAuth::extract_token(header) {
            Some(t) => t,
            None => return JwtError::Missing.into_response(),
        },
        None => return JwtError::Missing.into_response(),
    };

    // Validate token
    let claims = match jwt_auth.validate_token(token) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    // Store claims in request extensions for extractors
    request.extensions_mut().insert(claims);

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JwtConfig {
        JwtConfig {
            secret: "test-secret-key-that-is-32-chars!".to_string(),
            issuer: "test-issuer".to_string(),
            audience: "test-audience".to_string(),
            token_expiry: Duration::from_secs(3600),
            refresh_expiry: Duration::from_secs(86400),
            skip_paths: vec!["/health".to_string()],
        }
    }

    #[test]
    fn test_generate_and_validate_token() {
        let auth = JwtAuth::new(test_config());

        let token = auth
            .generate_token(
                "user-123",
                Some("testuser"),
                Some("admin"),
                vec!["read".to_string()],
            )
            .expect("Should generate token");

        let claims = auth.validate_token(&token).expect("Should validate");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, Some("testuser".to_string()));
        assert_eq!(claims.role, Some("admin".to_string()));
    }

    #[test]
    fn test_claims_has_scope() {
        let claims = Claims {
            sub: "user".to_string(),
            username: None,
            role: None,
            scopes: vec!["read".to_string(), "write".to_string()],
            iss: "test".to_string(),
            aud: "test".to_string(),
            exp: 0,
            iat: 0,
        };

        assert!(claims.has_scope("read"));
        assert!(claims.has_scope("write"));
        assert!(!claims.has_scope("delete"));
    }

    #[test]
    fn test_claims_wildcard_scope() {
        let claims = Claims {
            sub: "admin".to_string(),
            username: None,
            role: None,
            scopes: vec!["*".to_string()],
            iss: "test".to_string(),
            aud: "test".to_string(),
            exp: 0,
            iat: 0,
        };

        assert!(claims.has_scope("anything"));
        assert!(claims.has_scope("admin"));
    }

    #[test]
    fn test_claims_has_role() {
        let claims = Claims {
            sub: "user".to_string(),
            username: None,
            role: Some("admin".to_string()),
            scopes: vec![],
            iss: "test".to_string(),
            aud: "test".to_string(),
            exp: 0,
            iat: 0,
        };

        assert!(claims.has_role("admin"));
        assert!(!claims.has_role("user"));
    }

    #[test]
    fn test_should_skip_path() {
        let auth = JwtAuth::new(test_config());

        assert!(auth.should_skip("/health"));
        assert!(auth.should_skip("/health/live"));
        assert!(!auth.should_skip("/api/offers"));
    }

    #[test]
    fn test_extract_token() {
        assert_eq!(JwtAuth::extract_token("Bearer abc123"), Some("abc123"));
        assert_eq!(JwtAuth::extract_token("Basic xyz"), None);
    }

    #[test]
    fn test_invalid_token() {
        let auth = JwtAuth::new(test_config());
        let result = auth.validate_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret() {
        let auth1 = JwtAuth::new(test_config());
        let token = auth1
            .generate_token("user", None, None, vec![])
            .expect("Should generate");

        let mut config2 = test_config();
        config2.secret = "different-secret-key-32-chars!!".to_string();
        let auth2 = JwtAuth::new(config2);

        let result = auth2.validate_token(&token);
        assert!(result.is_err());
    }
}
