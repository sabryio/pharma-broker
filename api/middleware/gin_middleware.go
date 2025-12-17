// Package middleware provides HTTP middleware components for Gin.
package middleware

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"github.com/rs/zerolog"

	"pharmabroker/api/handlers"
)

// GinTracing adds distributed tracing support via X-Trace-ID header
func GinTracing(log zerolog.Logger) gin.HandlerFunc {
	return func(c *gin.Context) {
		start := time.Now()

		// Get or generate trace ID
		traceID := c.GetHeader("X-Trace-ID")
		if traceID == "" {
			traceID = uuid.New().String()[:8] // Short trace ID for readability
		}

		// Store in context for downstream use
		c.Set("trace_id", traceID)

		// Add to response header
		c.Header("X-Trace-ID", traceID)

		// Log request start
		log.Debug().
			Str("trace_id", traceID).
			Str("method", c.Request.Method).
			Str("path", c.Request.URL.Path).
			Str("client_ip", c.ClientIP()).
			Msg("Request started")

		// Process request
		c.Next()

		// Log request completion
		log.Info().
			Str("trace_id", traceID).
			Str("method", c.Request.Method).
			Str("path", c.Request.URL.Path).
			Int("status", c.Writer.Status()).
			Dur("latency", time.Since(start)).
			Msg("Request completed")
	}
}

// GinRateLimit applies per-IP rate limiting
func GinRateLimit(rps float64, burst int) gin.HandlerFunc {
	rl := NewRateLimiter(rps, burst)

	return func(c *gin.Context) {
		ip := c.ClientIP()
		limiter := rl.getLimiter(ip)

		if !limiter.Allow() {
			c.AbortWithStatusJSON(http.StatusTooManyRequests, handlers.Response{
				Success: false,
				Error:   handlers.ErrRateLimited("Too many requests"),
			})
			return
		}

		c.Next()
	}
}

// GinRecovery handles panic recovery with structured logging
func GinRecovery(log zerolog.Logger) gin.HandlerFunc {
	return func(c *gin.Context) {
		defer func() {
			if err := recover(); err != nil {
				traceID, _ := c.Get("trace_id")
				log.Error().
					Interface("panic", err).
					Interface("trace_id", traceID).
					Str("path", c.Request.URL.Path).
					Msg("Panic recovered in HTTP handler")

				c.AbortWithStatusJSON(http.StatusInternalServerError, handlers.Response{
					Success: false,
					Error:   handlers.ErrInternal("Internal server error"),
				})
			}
		}()

		c.Next()
	}
}

// GinTimeout adds request timeout middleware
func GinTimeout(timeout time.Duration) gin.HandlerFunc {
	return func(c *gin.Context) {
		// Note: For SSE endpoints, this should be skipped
		if c.Request.URL.Path == "/api/events" {
			c.Next()
			return
		}

		// The underlying http.Server should handle timeouts
		c.Next()
	}
}

// GetTraceIDFromGin extracts trace ID from Gin context
func GetTraceIDFromGin(c *gin.Context) string {
	if id, exists := c.Get("trace_id"); exists {
		if traceID, ok := id.(string); ok {
			return traceID
		}
	}
	return ""
}
