package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"
)

func TestNewJWTAuth(t *testing.T) {
	tests := []struct {
		name    string
		config  JWTConfig
		wantErr bool
	}{
		{
			name: "valid config",
			config: JWTConfig{
				Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
				Issuer:   "test",
				Audience: "test-api",
			},
			wantErr: false,
		},
		{
			name: "empty secret",
			config: JWTConfig{
				Secret: "",
			},
			wantErr: true,
		},
		{
			name: "short secret",
			config: JWTConfig{
				Secret: "short",
			},
			wantErr: true,
		},
		{
			name: "uses defaults",
			config: JWTConfig{
				Secret: "this-is-a-very-long-secret-key-for-testing-purposes",
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			auth, err := NewJWTAuth(tt.config)
			if (err != nil) != tt.wantErr {
				t.Errorf("NewJWTAuth() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if !tt.wantErr && auth == nil {
				t.Error("NewJWTAuth() returned nil auth without error")
			}
		})
	}
}

func TestJWTAuth_GenerateAndValidateToken(t *testing.T) {
	auth, err := NewJWTAuth(JWTConfig{
		Secret:        "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:        "pharmabroker",
		Audience:      "pharmabroker-api",
		TokenExpiry:   time.Hour,
		RefreshExpiry: 24 * time.Hour,
	})
	if err != nil {
		t.Fatalf("Failed to create JWTAuth: %v", err)
	}

	// Generate token
	token, err := auth.GenerateToken("user123", "testuser", "admin", []string{"read", "write"})
	if err != nil {
		t.Fatalf("Failed to generate token: %v", err)
	}

	if token == "" {
		t.Error("Generated token is empty")
	}

	// Validate token
	claims, err := auth.ValidateToken(token)
	if err != nil {
		t.Fatalf("Failed to validate token: %v", err)
	}

	if claims.UserID != "user123" {
		t.Errorf("UserID = %v, want user123", claims.UserID)
	}
	if claims.Username != "testuser" {
		t.Errorf("Username = %v, want testuser", claims.Username)
	}
	if claims.Role != "admin" {
		t.Errorf("Role = %v, want admin", claims.Role)
	}
	if len(claims.Scopes) != 2 {
		t.Errorf("Scopes length = %v, want 2", len(claims.Scopes))
	}
}

func TestJWTAuth_ValidateToken_Invalid(t *testing.T) {
	auth, _ := NewJWTAuth(JWTConfig{
		Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:   "pharmabroker",
		Audience: "pharmabroker-api",
	})

	tests := []struct {
		name  string
		token string
	}{
		{"empty token", ""},
		{"invalid format", "not-a-jwt-token"},
		{"wrong signature", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.wrong-signature"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := auth.ValidateToken(tt.token)
			if err == nil {
				t.Error("ValidateToken() should return error for invalid token")
			}
		})
	}
}

func TestJWTAuth_ValidateToken_WrongIssuer(t *testing.T) {
	auth1, _ := NewJWTAuth(JWTConfig{
		Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:   "issuer1",
		Audience: "api",
	})
	auth2, _ := NewJWTAuth(JWTConfig{
		Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:   "issuer2",
		Audience: "api",
	})

	token, _ := auth1.GenerateToken("user", "user", "user", nil)
	_, err := auth2.ValidateToken(token)
	if err == nil {
		t.Error("ValidateToken() should reject token with wrong issuer")
	}
}

func TestJWTAuth_GinMiddleware(t *testing.T) {
	gin.SetMode(gin.TestMode)

	auth, _ := NewJWTAuth(JWTConfig{
		Secret:    "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:    "pharmabroker",
		Audience:  "pharmabroker-api",
		SkipPaths: []string{"/health"},
	})

	token, _ := auth.GenerateToken("user123", "testuser", "admin", []string{"read"})

	tests := []struct {
		name           string
		path           string
		authHeader     string
		expectedStatus int
	}{
		{
			name:           "valid token",
			path:           "/api/test",
			authHeader:     "Bearer " + token,
			expectedStatus: http.StatusOK,
		},
		{
			name:           "missing header",
			path:           "/api/test",
			authHeader:     "",
			expectedStatus: http.StatusUnauthorized,
		},
		{
			name:           "invalid format",
			path:           "/api/test",
			authHeader:     "InvalidFormat",
			expectedStatus: http.StatusUnauthorized,
		},
		{
			name:           "invalid token",
			path:           "/api/test",
			authHeader:     "Bearer invalid-token",
			expectedStatus: http.StatusUnauthorized,
		},
		{
			name:           "skip path",
			path:           "/health",
			authHeader:     "",
			expectedStatus: http.StatusOK,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := gin.New()
			r.Use(auth.GinJWT())
			r.GET("/api/test", func(c *gin.Context) {
				c.Status(http.StatusOK)
			})
			r.GET("/health", func(c *gin.Context) {
				c.Status(http.StatusOK)
			})

			req := httptest.NewRequest("GET", tt.path, nil)
			if tt.authHeader != "" {
				req.Header.Set("Authorization", tt.authHeader)
			}
			w := httptest.NewRecorder()

			r.ServeHTTP(w, req)

			if w.Code != tt.expectedStatus {
				t.Errorf("Status = %v, want %v", w.Code, tt.expectedStatus)
			}
		})
	}
}

func TestJWTAuth_RequireRole(t *testing.T) {
	gin.SetMode(gin.TestMode)

	auth, _ := NewJWTAuth(JWTConfig{
		Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:   "pharmabroker",
		Audience: "pharmabroker-api",
	})

	adminToken, _ := auth.GenerateToken("admin1", "admin", "admin", nil)
	userToken, _ := auth.GenerateToken("user1", "user", "user", nil)

	tests := []struct {
		name           string
		token          string
		allowedRoles   []string
		expectedStatus int
	}{
		{
			name:           "admin accessing admin route",
			token:          adminToken,
			allowedRoles:   []string{"admin"},
			expectedStatus: http.StatusOK,
		},
		{
			name:           "user accessing admin route",
			token:          userToken,
			allowedRoles:   []string{"admin"},
			expectedStatus: http.StatusForbidden,
		},
		{
			name:           "user accessing user route",
			token:          userToken,
			allowedRoles:   []string{"user", "admin"},
			expectedStatus: http.StatusOK,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			r := gin.New()
			r.Use(auth.GinJWT())
			r.GET("/test", auth.RequireRole(tt.allowedRoles...), func(c *gin.Context) {
				c.Status(http.StatusOK)
			})

			req := httptest.NewRequest("GET", "/test", nil)
			req.Header.Set("Authorization", "Bearer "+tt.token)
			w := httptest.NewRecorder()

			r.ServeHTTP(w, req)

			if w.Code != tt.expectedStatus {
				t.Errorf("Status = %v, want %v", w.Code, tt.expectedStatus)
			}
		})
	}
}

func TestGetUserHelpers(t *testing.T) {
	gin.SetMode(gin.TestMode)

	auth, _ := NewJWTAuth(JWTConfig{
		Secret:   "this-is-a-very-long-secret-key-for-testing-purposes",
		Issuer:   "pharmabroker",
		Audience: "pharmabroker-api",
	})

	token, _ := auth.GenerateToken("user123", "testuser", "admin", []string{"read", "write"})

	r := gin.New()
	r.Use(auth.GinJWT())
	r.GET("/test", func(c *gin.Context) {
		userID := GetUserID(c)
		username := GetUsername(c)
		role := GetRole(c)
		claims := GetClaims(c)

		if userID != "user123" {
			t.Errorf("GetUserID() = %v, want user123", userID)
		}
		if username != "testuser" {
			t.Errorf("GetUsername() = %v, want testuser", username)
		}
		if role != "admin" {
			t.Errorf("GetRole() = %v, want admin", role)
		}
		if claims == nil {
			t.Error("GetClaims() returned nil")
		}

		c.Status(http.StatusOK)
	})

	req := httptest.NewRequest("GET", "/test", nil)
	req.Header.Set("Authorization", "Bearer "+token)
	w := httptest.NewRecorder()

	r.ServeHTTP(w, req)
}
