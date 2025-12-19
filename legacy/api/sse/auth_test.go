package sse

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// TokenClaims Tests
// =============================================================================

func TestTokenClaims_HasScope(t *testing.T) {
	claims := &TokenClaims{
		UserID: "user-1",
		Scopes: []string{"sse:read", "sse:write"},
	}

	if !claims.HasScope("sse:read") {
		t.Error("Should have sse:read scope")
	}
	if !claims.HasScope("sse:write") {
		t.Error("Should have sse:write scope")
	}
	if claims.HasScope("admin") {
		t.Error("Should not have admin scope")
	}
}

func TestTokenClaims_HasScope_Wildcard(t *testing.T) {
	claims := &TokenClaims{
		UserID: "admin",
		Scopes: []string{"*"},
	}

	if !claims.HasScope("anything") {
		t.Error("Wildcard scope should match anything")
	}
}

func TestTokenClaims_IsExpired(t *testing.T) {
	// Not expired
	claims := &TokenClaims{
		ExpiresAt: time.Now().Add(time.Hour),
	}
	if claims.IsExpired() {
		t.Error("Token should not be expired")
	}

	// Expired
	claims.ExpiresAt = time.Now().Add(-time.Hour)
	if !claims.IsExpired() {
		t.Error("Token should be expired")
	}
}

// =============================================================================
// HMACTokenValidator Tests
// =============================================================================

func TestNewHMACTokenValidator(t *testing.T) {
	log := zerolog.Nop()
	validator := NewHMACTokenValidator("secret-key", log)

	if validator == nil {
		t.Fatal("NewHMACTokenValidator returned nil")
	}
}

func TestHMACTokenValidator_GenerateAndValidate(t *testing.T) {
	log := zerolog.Nop()
	validator := NewHMACTokenValidator("secret-key", log)

	token := validator.GenerateToken("user-123", []string{"sse:read"}, time.Hour)
	if token == "" {
		t.Fatal("GenerateToken returned empty string")
	}

	claims, err := validator.ValidateToken(token)
	if err != nil {
		t.Fatalf("ValidateToken error: %v", err)
	}

	if claims.UserID != "user-123" {
		t.Errorf("UserID = %s, want user-123", claims.UserID)
	}
	if !claims.HasScope("sse:read") {
		t.Error("Should have sse:read scope")
	}
}

func TestHMACTokenValidator_ValidateToken_Invalid(t *testing.T) {
	log := zerolog.Nop()
	validator := NewHMACTokenValidator("secret-key", log)

	_, err := validator.ValidateToken("invalid-token")
	if err == nil {
		t.Error("Should return error for invalid token")
	}
}

func TestHMACTokenValidator_ValidateToken_WrongSignature(t *testing.T) {
	log := zerolog.Nop()
	validator1 := NewHMACTokenValidator("secret-1", log)
	validator2 := NewHMACTokenValidator("secret-2", log)

	token := validator1.GenerateToken("user-123", []string{"sse:read"}, time.Hour)

	_, err := validator2.ValidateToken(token)
	if err != ErrInvalidSignature {
		t.Errorf("Expected ErrInvalidSignature, got %v", err)
	}
}

func TestHMACTokenValidator_ValidateToken_Expired(t *testing.T) {
	log := zerolog.Nop()
	validator := NewHMACTokenValidator("secret-key", log)

	// Generate token that's already expired
	token := validator.GenerateToken("user-123", []string{"sse:read"}, -time.Hour)

	_, err := validator.ValidateToken(token)
	if err != ErrExpiredToken {
		t.Errorf("Expected ErrExpiredToken, got %v", err)
	}
}

// =============================================================================
// APIKeyValidator Tests
// =============================================================================

func TestNewAPIKeyValidator(t *testing.T) {
	log := zerolog.Nop()
	validator := NewAPIKeyValidator(log)

	if validator == nil {
		t.Fatal("NewAPIKeyValidator returned nil")
	}
}

func TestAPIKeyValidator_AddAndValidate(t *testing.T) {
	log := zerolog.Nop()
	validator := NewAPIKeyValidator(log)

	validator.AddKey("api-key-123", "user-1", []string{"sse:read", "sse:write"})

	claims, err := validator.ValidateToken("api-key-123")
	if err != nil {
		t.Fatalf("ValidateToken error: %v", err)
	}

	if claims.UserID != "user-1" {
		t.Errorf("UserID = %s, want user-1", claims.UserID)
	}
}

func TestAPIKeyValidator_ValidateToken_NotFound(t *testing.T) {
	log := zerolog.Nop()
	validator := NewAPIKeyValidator(log)

	_, err := validator.ValidateToken("unknown-key")
	if err != ErrInvalidToken {
		t.Errorf("Expected ErrInvalidToken, got %v", err)
	}
}

func TestAPIKeyValidator_RemoveKey(t *testing.T) {
	log := zerolog.Nop()
	validator := NewAPIKeyValidator(log)

	validator.AddKey("api-key-123", "user-1", []string{"sse:read"})
	validator.RemoveKey("api-key-123")

	_, err := validator.ValidateToken("api-key-123")
	if err != ErrInvalidToken {
		t.Errorf("Expected ErrInvalidToken after removal, got %v", err)
	}
}

// =============================================================================
// AuthConfig Tests
// =============================================================================

func TestDefaultAuthConfig(t *testing.T) {
	cfg := DefaultAuthConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.TokenParam != "token" {
		t.Errorf("TokenParam = %s, want token", cfg.TokenParam)
	}
	if cfg.HeaderName != "Authorization" {
		t.Errorf("HeaderName = %s, want Authorization", cfg.HeaderName)
	}
}

// =============================================================================
// AuthenticatedSSEHub Tests
// =============================================================================

func TestNewAuthenticatedSSEHub(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	authHub := NewAuthenticatedSSEHub(hub, validator, DefaultAuthConfig(), log)

	if authHub == nil {
		t.Fatal("NewAuthenticatedSSEHub returned nil")
	}
}

func TestAuthenticatedSSEHub_extractToken_QueryParam(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	authHub := NewAuthenticatedSSEHub(hub, validator, DefaultAuthConfig(), log)

	req := httptest.NewRequest("GET", "/sse?token=my-token", nil)
	token := authHub.extractToken(req)

	if token != "my-token" {
		t.Errorf("extractToken = %s, want my-token", token)
	}
}

func TestAuthenticatedSSEHub_extractToken_Header(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	authHub := NewAuthenticatedSSEHub(hub, validator, DefaultAuthConfig(), log)

	req := httptest.NewRequest("GET", "/sse", nil)
	req.Header.Set("Authorization", "Bearer my-token")
	token := authHub.extractToken(req)

	if token != "my-token" {
		t.Errorf("extractToken = %s, want my-token", token)
	}
}

func TestAuthenticatedSSEHub_ServeHTTP_Unauthorized(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	authHub := NewAuthenticatedSSEHub(hub, validator, DefaultAuthConfig(), log)

	req := httptest.NewRequest("GET", "/sse", nil)
	w := httptest.NewRecorder()

	authHub.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusUnauthorized)
	}
}

func TestAuthenticatedSSEHub_ServeHTTP_InvalidToken(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	authHub := NewAuthenticatedSSEHub(hub, validator, DefaultAuthConfig(), log)

	req := httptest.NewRequest("GET", "/sse?token=invalid", nil)
	w := httptest.NewRecorder()

	authHub.ServeHTTP(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("Status = %d, want %d", w.Code, http.StatusUnauthorized)
	}
}

func TestAuthenticatedSSEHub_Disabled(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	cfg := DefaultAuthConfig()
	cfg.Enabled = false
	authHub := NewAuthenticatedSSEHub(hub, validator, cfg, log)

	req := httptest.NewRequest("GET", "/sse", nil)
	claims, err := authHub.validateRequest(req)

	if err != nil {
		t.Errorf("validateRequest error with auth disabled: %v", err)
	}
	if claims.UserID != "anonymous" {
		t.Errorf("UserID = %s, want anonymous", claims.UserID)
	}
}

func TestAuthenticatedSSEHub_RequiredScopes(t *testing.T) {
	log := zerolog.Nop()
	hub := NewSSEHub()
	defer hub.Shutdown()

	validator := NewAPIKeyValidator(log)
	validator.AddKey("limited-key", "user-1", []string{"other:scope"})

	cfg := DefaultAuthConfig()
	cfg.RequireScopes = []string{"sse:read"}
	authHub := NewAuthenticatedSSEHub(hub, validator, cfg, log)

	req := httptest.NewRequest("GET", "/sse?token=limited-key", nil)
	_, err := authHub.validateRequest(req)

	if err == nil {
		t.Error("Should return error for missing required scope")
	}
}

// =============================================================================
// Context Helpers Tests
// =============================================================================

func TestGetClaimsFromContext(t *testing.T) {
	// Nil context
	claims := GetClaimsFromContext(context.TODO())
	if claims != nil {
		t.Error("Should return nil for nil context")
	}

	// Empty context (no claims)
	claims = GetClaimsFromContext(context.Background())
	if claims != nil {
		t.Error("Should return nil for context without claims")
	}
}
