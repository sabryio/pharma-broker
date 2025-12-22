// Package http provides the HTTP server using Gin framework.
package http

import (
	"context"
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharma-bridge/domain"
)

// Server wraps the Gin engine and HTTP server.
type Server struct {
	engine *gin.Engine
	server *http.Server
	logger zerolog.Logger
}

// ServerConfig holds configuration for the HTTP server.
type ServerConfig struct {
	Port string
	Mode string // "debug", "release", "test"
}

// NewServer creates a new HTTP server.
func NewServer(cfg ServerConfig, logger zerolog.Logger) *Server {
	if cfg.Mode == "" {
		cfg.Mode = gin.ReleaseMode
	}
	gin.SetMode(cfg.Mode)

	engine := gin.New()
	engine.Use(gin.Recovery())
	engine.Use(zerologMiddleware(logger))

	return &Server{
		engine: engine,
		server: &http.Server{
			Addr:    ":" + cfg.Port,
			Handler: engine,
		},
		logger: logger.With().Str("component", "http").Logger(),
	}
}

// Engine returns the Gin engine for route registration.
func (s *Server) Engine() *gin.Engine {
	return s.engine
}

// Start starts the HTTP server in a goroutine.
func (s *Server) Start() {
	go func() {
		s.logger.Info().Str("addr", s.server.Addr).Msg("🏥 HTTP server starting")
		if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			s.logger.Error().Err(err).Msg("HTTP server failed")
		}
	}()
}

// Shutdown gracefully shuts down the server.
func (s *Server) Shutdown(ctx context.Context) error {
	return s.server.Shutdown(ctx)
}

// zerologMiddleware creates a Gin middleware for zerolog logging.
func zerologMiddleware(logger zerolog.Logger) gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Next()

		// Only log errors
		if len(c.Errors) > 0 {
			logger.Error().
				Str("path", c.Request.URL.Path).
				Int("status", c.Writer.Status()).
				Str("errors", c.Errors.String()).
				Msg("Request error")
		}
	}
}

// HealthResponse represents the health check response.
type HealthResponse struct {
	Status            string           `json:"status"`
	Service           string           `json:"service"`
	Version           string           `json:"version"`
	WhatsAppConnected bool             `json:"whatsapp_connected"`
	MessagesForwarded int64            `json:"messages_forwarded"`
	CircuitBreaker    string           `json:"circuit_breaker"`
	RetryBufferSize   int              `json:"retry_buffer_size,omitempty"`
	DeduplicatorStats any              `json:"deduplicator_stats,omitempty"`
	RateLimiterStats  map[string]int64 `json:"rate_limiter_stats,omitempty"`
}

// NewHealthResponse creates a health response with the current version.
func NewHealthResponse() HealthResponse {
	return HealthResponse{
		Service: "pharma-bridge",
		Version: domain.CurrentVersion.String(),
	}
}
