package sse

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"fmt"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"
)

// =============================================================================
// Authentication Errors
// =============================================================================

var (
	ErrMissingToken     = errors.New("missing authentication token")
	ErrInvalidToken     = errors.New("invalid authentication token")
	ErrExpiredToken     = errors.New("token has expired")
	ErrInvalidSignature = errors.New("invalid token signature")
	ErrUserNotFound     = errors.New("user not found")
)

// =============================================================================
// Token Claims
// =============================================================================

// TokenClaims represents the claims in an SSE authentication token.
type TokenClaims struct {
	UserID    string
	Scopes    []string
	ExpiresAt time.Time
	IssuedAt  time.Time
}

// HasScope checks if the token has a specific scope.
func (c *TokenClaims) HasScope(scope string) bool {
	for _, s := range c.Scopes {
		if s == scope || s == "*" {
			return true
		}
	}
	return false
}

// IsExpired checks if the token has expired.
func (c *TokenClaims) IsExpired() bool {
	return time.Now().After(c.ExpiresAt)
}

// =============================================================================
// Token Validator Interface
// =============================================================================

// TokenValidator validates SSE authentication tokens.
type TokenValidator interface {
	ValidateToken(token string) (*TokenClaims, error)
}

// =============================================================================
// HMAC Token Validator
// =============================================================================

// HMACTokenValidator validates tokens using HMAC-SHA256.
type HMACTokenValidator struct {
	secret []byte
	log    zerolog.Logger
}

// NewHMACTokenValidator creates a new HMAC token validator.
func NewHMACTokenValidator(secret string, log zerolog.Logger) *HMACTokenValidator {
	return &HMACTokenValidator{
		secret: []byte(secret),
		log:    log.With().Str("component", "sse-auth").Logger(),
	}
}

// GenerateToken generates a new authentication token.
func (v *HMACTokenValidator) GenerateToken(userID string, scopes []string, ttl time.Duration) string {
	now := time.Now()
	expiresAt := now.Add(ttl)

	// Format: userID|scopes|expiresAt|issuedAt
	payload := fmt.Sprintf("%s|%s|%d|%d",
		userID,
		strings.Join(scopes, ","),
		expiresAt.Unix(),
		now.Unix(),
	)

	// Sign the payload
	signature := v.sign(payload)

	// Encode: base64(payload).base64(signature)
	encodedPayload := base64.URLEncoding.EncodeToString([]byte(payload))
	encodedSignature := base64.URLEncoding.EncodeToString(signature)

	return encodedPayload + "." + encodedSignature
}

// ValidateToken validates a token and returns the claims.
func (v *HMACTokenValidator) ValidateToken(token string) (*TokenClaims, error) {
	parts := strings.Split(token, ".")
	if len(parts) != 2 {
		return nil, ErrInvalidToken
	}

	// Decode payload
	payloadBytes, err := base64.URLEncoding.DecodeString(parts[0])
	if err != nil {
		return nil, ErrInvalidToken
	}
	payload := string(payloadBytes)

	// Decode signature
	signature, err := base64.URLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, ErrInvalidToken
	}

	// Verify signature
	expectedSig := v.sign(payload)
	if !hmac.Equal(signature, expectedSig) {
		return nil, ErrInvalidSignature
	}

	// Parse payload
	parts = strings.Split(payload, "|")
	if len(parts) != 4 {
		return nil, ErrInvalidToken
	}

	var expiresAt, issuedAt int64
	if _, err := fmt.Sscanf(parts[2], "%d", &expiresAt); err != nil {
		return nil, ErrInvalidToken
	}
	if _, err := fmt.Sscanf(parts[3], "%d", &issuedAt); err != nil {
		return nil, ErrInvalidToken
	}

	claims := &TokenClaims{
		UserID:    parts[0],
		Scopes:    strings.Split(parts[1], ","),
		ExpiresAt: time.Unix(expiresAt, 0),
		IssuedAt:  time.Unix(issuedAt, 0),
	}

	// Check expiration
	if claims.IsExpired() {
		return nil, ErrExpiredToken
	}

	return claims, nil
}

// sign creates an HMAC-SHA256 signature.
func (v *HMACTokenValidator) sign(payload string) []byte {
	h := hmac.New(sha256.New, v.secret)
	h.Write([]byte(payload))
	return h.Sum(nil)
}

// =============================================================================
// API Key Validator
// =============================================================================

// APIKeyValidator validates static API keys.
type APIKeyValidator struct {
	keys map[string]*TokenClaims // key -> claims
	log  zerolog.Logger
	mu   sync.RWMutex
}

// NewAPIKeyValidator creates a new API key validator.
func NewAPIKeyValidator(log zerolog.Logger) *APIKeyValidator {
	return &APIKeyValidator{
		keys: make(map[string]*TokenClaims),
		log:  log.With().Str("component", "sse-apikey").Logger(),
	}
}

// AddKey adds an API key with associated claims.
func (v *APIKeyValidator) AddKey(key string, userID string, scopes []string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	v.keys[key] = &TokenClaims{
		UserID:    userID,
		Scopes:    scopes,
		ExpiresAt: time.Now().Add(100 * 365 * 24 * time.Hour), // Never expires
		IssuedAt:  time.Now(),
	}
}

// RemoveKey removes an API key.
func (v *APIKeyValidator) RemoveKey(key string) {
	v.mu.Lock()
	defer v.mu.Unlock()
	delete(v.keys, key)
}

// ValidateToken validates an API key.
func (v *APIKeyValidator) ValidateToken(token string) (*TokenClaims, error) {
	v.mu.RLock()
	defer v.mu.RUnlock()

	claims, ok := v.keys[token]
	if !ok {
		return nil, ErrInvalidToken
	}

	return claims, nil
}

// =============================================================================
// Authenticated SSE Hub
// =============================================================================

// AuthConfig configures SSE authentication.
type AuthConfig struct {
	Enabled       bool
	RequireScopes []string // Required scopes for SSE access
	TokenParam    string   // Query parameter name for token (default: "token")
	HeaderName    string   // Header name for token (default: "Authorization")
}

// DefaultAuthConfig returns sensible defaults.
func DefaultAuthConfig() AuthConfig {
	return AuthConfig{
		Enabled:       true,
		RequireScopes: []string{"sse:read"},
		TokenParam:    "token",
		HeaderName:    "Authorization",
	}
}

// AuthenticatedSSEHub wraps SSEHub with authentication.
type AuthenticatedSSEHub struct {
	*SSEHub
	validator TokenValidator
	config    AuthConfig
	log       zerolog.Logger
}

// NewAuthenticatedSSEHub creates a new authenticated SSE hub.
func NewAuthenticatedSSEHub(hub *SSEHub, validator TokenValidator, cfg AuthConfig, log zerolog.Logger) *AuthenticatedSSEHub {
	if cfg.TokenParam == "" {
		cfg.TokenParam = "token"
	}
	if cfg.HeaderName == "" {
		cfg.HeaderName = "Authorization"
	}

	return &AuthenticatedSSEHub{
		SSEHub:    hub,
		validator: validator,
		config:    cfg,
		log:       log.With().Str("component", "sse-auth-hub").Logger(),
	}
}

// extractToken extracts the token from request.
func (h *AuthenticatedSSEHub) extractToken(r *http.Request) string {
	// Try query parameter first (SSE can't easily use headers)
	if token := r.URL.Query().Get(h.config.TokenParam); token != "" {
		return token
	}

	// Try Authorization header
	auth := r.Header.Get(h.config.HeaderName)
	if strings.HasPrefix(auth, "Bearer ") {
		return strings.TrimPrefix(auth, "Bearer ")
	}

	return ""
}

// validateRequest validates the request and returns claims.
func (h *AuthenticatedSSEHub) validateRequest(r *http.Request) (*TokenClaims, error) {
	if !h.config.Enabled {
		return &TokenClaims{UserID: "anonymous", Scopes: []string{"*"}}, nil
	}

	token := h.extractToken(r)
	if token == "" {
		return nil, ErrMissingToken
	}

	claims, err := h.validator.ValidateToken(token)
	if err != nil {
		return nil, err
	}

	// Check required scopes
	for _, required := range h.config.RequireScopes {
		if !claims.HasScope(required) {
			return nil, fmt.Errorf("missing required scope: %s", required)
		}
	}

	return claims, nil
}

// ServeHTTP handles authenticated SSE connections.
func (h *AuthenticatedSSEHub) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	claims, err := h.validateRequest(r)
	if err != nil {
		h.log.Warn().Err(err).Str("remote", r.RemoteAddr).Msg("SSE authentication failed")
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}

	h.log.Debug().
		Str("user_id", claims.UserID).
		Strs("scopes", claims.Scopes).
		Msg("SSE client authenticated")

	// Store claims in request context
	ctx := context.WithValue(r.Context(), sseClaimsKey, claims)
	h.SSEHub.ServeHTTP(w, r.WithContext(ctx))
}

// GinHandler returns an authenticated Gin handler.
func (h *AuthenticatedSSEHub) GinHandler() gin.HandlerFunc {
	return func(c *gin.Context) {
		claims, err := h.validateRequest(c.Request)
		if err != nil {
			h.log.Warn().Err(err).Str("remote", c.ClientIP()).Msg("SSE authentication failed")
			c.JSON(http.StatusUnauthorized, gin.H{"error": err.Error()})
			return
		}

		h.log.Debug().
			Str("user_id", claims.UserID).
			Strs("scopes", claims.Scopes).
			Msg("SSE client authenticated")

		// Store claims in Gin context
		c.Set("sse_claims", claims)
		c.Set("user_id", claims.UserID)

		// Call the underlying handler
		h.SSEHub.GinHandler()(c)
	}
}

// Context key for SSE claims
type sseClaimsKeyType struct{}

var sseClaimsKey = sseClaimsKeyType{}

// GetClaimsFromContext retrieves claims from context.
func GetClaimsFromContext(ctx context.Context) *TokenClaims {
	if ctx == nil {
		return nil
	}
	if claims, ok := ctx.Value(sseClaimsKey).(*TokenClaims); ok {
		return claims
	}
	return nil
}

// GetClaimsFromGin retrieves claims from Gin context.
func GetClaimsFromGin(c *gin.Context) *TokenClaims {
	if claims, exists := c.Get("sse_claims"); exists {
		if tc, ok := claims.(*TokenClaims); ok {
			return tc
		}
	}
	return nil
}
