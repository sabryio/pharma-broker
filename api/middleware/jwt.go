// Package middleware provides HTTP middleware components.
package middleware

import (
	"crypto/subtle"
	"errors"
	"net/http"
	"strings"
	"time"

	"pharmabroker/api/handlers"

	"github.com/gin-gonic/gin"
	"github.com/golang-jwt/jwt/v5"
)

// JWTConfig holds JWT authentication configuration
type JWTConfig struct {
	// Secret is the key used to sign and verify JWT tokens.
	// REQUIRED - must be at least 32 characters for security.
	Secret string

	// Issuer is the expected "iss" claim in tokens.
	// Default: "pharmabroker"
	Issuer string

	// Audience is the expected "aud" claim in tokens.
	// Default: "pharmabroker-api"
	Audience string

	// TokenExpiry is the duration tokens are valid for.
	// Default: 24h
	TokenExpiry time.Duration

	// RefreshExpiry is the duration refresh tokens are valid for.
	// Default: 7 days
	RefreshExpiry time.Duration

	// SkipPaths are paths that don't require authentication.
	// Health checks and metrics are typically skipped.
	SkipPaths []string
}

// DefaultJWTConfig returns a JWTConfig with sensible defaults
func DefaultJWTConfig() JWTConfig {
	return JWTConfig{
		Issuer:        "pharmabroker",
		Audience:      "pharmabroker-api",
		TokenExpiry:   24 * time.Hour,
		RefreshExpiry: 7 * 24 * time.Hour,
		SkipPaths: []string{
			"/health",
			"/health/live",
			"/health/ready",
			"/metrics",
		},
	}
}

// Claims represents the JWT claims structure
type Claims struct {
	jwt.RegisteredClaims
	UserID   string   `json:"user_id,omitempty"`
	Username string   `json:"username,omitempty"`
	Role     string   `json:"role,omitempty"`
	Scopes   []string `json:"scopes,omitempty"`
}

// JWTAuth provides JWT authentication functionality
type JWTAuth struct {
	config JWTConfig
	skip   map[string]struct{}
}

// NewJWTAuth creates a new JWT authenticator
func NewJWTAuth(cfg JWTConfig) (*JWTAuth, error) {
	if cfg.Secret == "" {
		return nil, errors.New("jwt secret is required")
	}
	if len(cfg.Secret) < 32 {
		return nil, errors.New("jwt secret must be at least 32 characters")
	}

	// Apply defaults
	if cfg.Issuer == "" {
		cfg.Issuer = "pharmabroker"
	}
	if cfg.Audience == "" {
		cfg.Audience = "pharmabroker-api"
	}
	if cfg.TokenExpiry == 0 {
		cfg.TokenExpiry = 24 * time.Hour
	}
	if cfg.RefreshExpiry == 0 {
		cfg.RefreshExpiry = 7 * 24 * time.Hour
	}

	// Build skip path set
	skip := make(map[string]struct{}, len(cfg.SkipPaths))
	for _, p := range cfg.SkipPaths {
		skip[p] = struct{}{}
	}

	return &JWTAuth{
		config: cfg,
		skip:   skip,
	}, nil
}

// GenerateToken creates a new JWT token for a user
func (j *JWTAuth) GenerateToken(userID, username, role string, scopes []string) (string, error) {
	now := time.Now()
	claims := Claims{
		RegisteredClaims: jwt.RegisteredClaims{
			Issuer:    j.config.Issuer,
			Audience:  jwt.ClaimStrings{j.config.Audience},
			Subject:   userID,
			IssuedAt:  jwt.NewNumericDate(now),
			ExpiresAt: jwt.NewNumericDate(now.Add(j.config.TokenExpiry)),
			NotBefore: jwt.NewNumericDate(now),
		},
		UserID:   userID,
		Username: username,
		Role:     role,
		Scopes:   scopes,
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	return token.SignedString([]byte(j.config.Secret))
}

// GenerateRefreshToken creates a refresh token with longer expiry
func (j *JWTAuth) GenerateRefreshToken(userID string) (string, error) {
	now := time.Now()
	claims := jwt.RegisteredClaims{
		Issuer:    j.config.Issuer,
		Audience:  jwt.ClaimStrings{j.config.Audience},
		Subject:   userID,
		IssuedAt:  jwt.NewNumericDate(now),
		ExpiresAt: jwt.NewNumericDate(now.Add(j.config.RefreshExpiry)),
		NotBefore: jwt.NewNumericDate(now),
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	return token.SignedString([]byte(j.config.Secret))
}

// ValidateToken parses and validates a JWT token
func (j *JWTAuth) ValidateToken(tokenString string) (*Claims, error) {
	token, err := jwt.ParseWithClaims(tokenString, &Claims{}, func(token *jwt.Token) (interface{}, error) {
		// Validate signing method
		if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
			return nil, errors.New("unexpected signing method")
		}
		return []byte(j.config.Secret), nil
	})

	if err != nil {
		return nil, err
	}

	claims, ok := token.Claims.(*Claims)
	if !ok || !token.Valid {
		return nil, errors.New("invalid token claims")
	}

	// Validate issuer
	if claims.Issuer != j.config.Issuer {
		return nil, errors.New("invalid token issuer")
	}

	// Validate audience
	validAudience := false
	for _, aud := range claims.Audience {
		if aud == j.config.Audience {
			validAudience = true
			break
		}
	}
	if !validAudience {
		return nil, errors.New("invalid token audience")
	}

	return claims, nil
}

// GinJWT returns a Gin middleware for JWT authentication
func (j *JWTAuth) GinJWT() gin.HandlerFunc {
	return func(c *gin.Context) {
		// Check if path should skip authentication
		if _, skip := j.skip[c.Request.URL.Path]; skip {
			c.Next()
			return
		}

		// Extract token from Authorization header
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("missing authorization header"),
			})
			return
		}

		// Expect "Bearer <token>" format
		parts := strings.SplitN(authHeader, " ", 2)
		if len(parts) != 2 || !strings.EqualFold(parts[0], "Bearer") {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("invalid authorization header format"),
			})
			return
		}

		tokenString := parts[1]

		// Validate token
		claims, err := j.ValidateToken(tokenString)
		if err != nil {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("invalid or expired token"),
			})
			return
		}

		// Store claims in context for handlers to use
		c.Set("user_id", claims.UserID)
		c.Set("username", claims.Username)
		c.Set("role", claims.Role)
		c.Set("scopes", claims.Scopes)
		c.Set("claims", claims)

		c.Next()
	}
}

// RequireRole returns middleware that checks for a specific role
func (j *JWTAuth) RequireRole(roles ...string) gin.HandlerFunc {
	roleSet := make(map[string]struct{}, len(roles))
	for _, r := range roles {
		roleSet[r] = struct{}{}
	}

	return func(c *gin.Context) {
		role, exists := c.Get("role")
		if !exists {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("authentication required"),
			})
			return
		}

		roleStr, ok := role.(string)
		if !ok {
			c.AbortWithStatusJSON(http.StatusInternalServerError, handlers.Response{
				Success: false,
				Error:   handlers.ErrInternal("invalid role type"),
			})
			return
		}

		if _, allowed := roleSet[roleStr]; !allowed {
			c.AbortWithStatusJSON(http.StatusForbidden, handlers.Response{
				Success: false,
				Error:   handlers.ErrForbidden("insufficient permissions"),
			})
			return
		}

		c.Next()
	}
}

// RequireScope returns middleware that checks for specific scopes
func (j *JWTAuth) RequireScope(requiredScopes ...string) gin.HandlerFunc {
	return func(c *gin.Context) {
		scopesVal, exists := c.Get("scopes")
		if !exists {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("authentication required"),
			})
			return
		}

		scopes, ok := scopesVal.([]string)
		if !ok {
			c.AbortWithStatusJSON(http.StatusInternalServerError, handlers.Response{
				Success: false,
				Error:   handlers.ErrInternal("invalid scopes type"),
			})
			return
		}

		// Check if user has all required scopes
		scopeSet := make(map[string]struct{}, len(scopes))
		for _, s := range scopes {
			scopeSet[s] = struct{}{}
		}

		for _, required := range requiredScopes {
			if _, has := scopeSet[required]; !has {
				c.AbortWithStatusJSON(http.StatusForbidden, handlers.Response{
					Success: false,
					Error:   handlers.ErrForbidden("missing required scope: " + required),
				})
				return
			}
		}

		c.Next()
	}
}

// APIKeyAuth provides simple API key authentication as an alternative to JWT
type APIKeyAuth struct {
	keys map[string]string // key -> description/owner
}

// NewAPIKeyAuth creates a new API key authenticator
func NewAPIKeyAuth(keys map[string]string) *APIKeyAuth {
	return &APIKeyAuth{keys: keys}
}

// GinAPIKey returns a Gin middleware for API key authentication
func (a *APIKeyAuth) GinAPIKey() gin.HandlerFunc {
	return func(c *gin.Context) {
		// Check X-API-Key header first
		apiKey := c.GetHeader("X-API-Key")
		if apiKey == "" {
			// Fall back to query parameter
			apiKey = c.Query("api_key")
		}

		if apiKey == "" {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("missing API key"),
			})
			return
		}

		// Constant-time comparison to prevent timing attacks
		valid := false
		for key := range a.keys {
			if subtle.ConstantTimeCompare([]byte(apiKey), []byte(key)) == 1 {
				valid = true
				break
			}
		}

		if !valid {
			c.AbortWithStatusJSON(http.StatusUnauthorized, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("invalid API key"),
			})
			return
		}

		c.Next()
	}
}

// GetUserID extracts user ID from Gin context (set by JWT middleware)
func GetUserID(c *gin.Context) string {
	if id, exists := c.Get("user_id"); exists {
		if str, ok := id.(string); ok {
			return str
		}
	}
	return ""
}

// GetUsername extracts username from Gin context (set by JWT middleware)
func GetUsername(c *gin.Context) string {
	if name, exists := c.Get("username"); exists {
		if str, ok := name.(string); ok {
			return str
		}
	}
	return ""
}

// GetRole extracts role from Gin context (set by JWT middleware)
func GetRole(c *gin.Context) string {
	if role, exists := c.Get("role"); exists {
		if str, ok := role.(string); ok {
			return str
		}
	}
	return ""
}

// GetClaims extracts full claims from Gin context (set by JWT middleware)
func GetClaims(c *gin.Context) *Claims {
	if claims, exists := c.Get("claims"); exists {
		if c, ok := claims.(*Claims); ok {
			return c
		}
	}
	return nil
}
