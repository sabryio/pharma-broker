// Package middleware provides HTTP middleware components.
package middleware

import (
	"context"
	"net/http"
	"pharmabroker/api/handlers"
	"runtime/debug"
	"strings"
	"sync"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"github.com/rs/zerolog"
	"golang.org/x/time/rate"
)

// rateLimiterEntry holds a rate limiter with its last access time for LRU eviction
type rateLimiterEntry struct {
	limiter    *rate.Limiter
	lastAccess time.Time
}

// RateLimiter manages per-IP rate limiting with TTL-based eviction
type RateLimiter struct {
	limiters map[string]*rateLimiterEntry
	mu       sync.RWMutex
	rps      float64
	burst    int
	ttl      time.Duration
	cancel   context.CancelFunc
}

// NewRateLimiter creates a new rate limiter with configurable limits
func NewRateLimiter(rps float64, burst int) *RateLimiter {
	return NewRateLimiterWithTTL(rps, burst, 10*time.Minute)
}

// NewRateLimiterWithTTL creates a new rate limiter with configurable limits and TTL
func NewRateLimiterWithTTL(rps float64, burst int, ttl time.Duration) *RateLimiter {
	if rps <= 0 {
		rps = 10.0
	}
	if burst <= 0 {
		burst = 20
	}
	if ttl <= 0 {
		ttl = 10 * time.Minute
	}
	return &RateLimiter{
		limiters: make(map[string]*rateLimiterEntry),
		rps:      rps,
		burst:    burst,
		ttl:      ttl,
	}
}

// getLimiter returns the rate limiter for a given IP
func (rl *RateLimiter) getLimiter(ip string) *rate.Limiter {
	now := time.Now()

	rl.mu.RLock()
	entry, exists := rl.limiters[ip]
	rl.mu.RUnlock()

	if exists {
		rl.mu.Lock()
		entry.lastAccess = now
		rl.mu.Unlock()
		return entry.limiter
	}

	rl.mu.Lock()
	defer rl.mu.Unlock()

	// Double-check after acquiring write lock
	if entry, exists = rl.limiters[ip]; exists {
		entry.lastAccess = now
		return entry.limiter
	}

	limiter := rate.NewLimiter(rate.Limit(rl.rps), rl.burst)
	rl.limiters[ip] = &rateLimiterEntry{
		limiter:    limiter,
		lastAccess: now,
	}

	return limiter
}

// StartCleanup periodically cleans up stale rate limiters based on TTL
func (rl *RateLimiter) StartCleanup(ctx context.Context, interval time.Duration) {
	ctx, rl.cancel = context.WithCancel(ctx)

	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				rl.cleanup()
			}
		}
	}()
}

// cleanup removes stale entries based on TTL
func (rl *RateLimiter) cleanup() {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	now := time.Now()
	for ip, entry := range rl.limiters {
		if now.Sub(entry.lastAccess) > rl.ttl {
			delete(rl.limiters, ip)
		}
	}
}

// Stop cancels the cleanup goroutine
func (rl *RateLimiter) Stop() {
	if rl.cancel != nil {
		rl.cancel()
	}
}

// SpaHandler serves static files and falls back to index.html for SPA routing
func SpaHandler(fsys http.FileSystem) http.Handler {
	fileServer := http.FileServer(fsys)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		path := r.URL.Path

		// Try to open the file
		f, err := fsys.Open(path)
		if err == nil {
			_ = f.Close()
		} else {
			// If file doesn't exist and it's not an API/static asset, serve index.html
			if !strings.HasPrefix(path, "/api") && !strings.Contains(path, ".") {
				r.URL.Path = "/"
			}
		}

		fileServer.ServeHTTP(w, r)
	})
}

// CorsConfig holds CORS configuration
type CorsConfig struct {
	AllowedOrigins   []string
	AllowedMethods   []string
	AllowedHeaders   []string
	AllowCredentials bool
}

// DefaultCorsConfig returns a permissive CORS config (use only for development)
func DefaultCorsConfig() CorsConfig {
	return CorsConfig{
		AllowedOrigins:   []string{"*"},
		AllowedMethods:   []string{"GET", "POST", "PATCH", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"Content-Type", "Authorization"},
		AllowCredentials: false,
	}
}

// CorsMiddleware handles CORS preflight and simple requests with configurable origins
func CorsMiddleware(cfg CorsConfig) func(http.Handler) http.Handler {
	allowAll := len(cfg.AllowedOrigins) == 1 && cfg.AllowedOrigins[0] == "*"
	originSet := make(map[string]struct{}, len(cfg.AllowedOrigins))
	for _, o := range cfg.AllowedOrigins {
		originSet[o] = struct{}{}
	}

	methods := strings.Join(cfg.AllowedMethods, ", ")
	headers := strings.Join(cfg.AllowedHeaders, ", ")

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			origin := r.Header.Get("Origin")

			// Check if origin is allowed
			allowed := false
			if allowAll {
				w.Header().Set("Access-Control-Allow-Origin", "*")
				allowed = true
			} else if origin != "" {
				if _, ok := originSet[origin]; ok {
					w.Header().Set("Access-Control-Allow-Origin", origin)
					w.Header().Set("Vary", "Origin")
					allowed = true
				}
			}

			if allowed {
				w.Header().Set("Access-Control-Allow-Methods", methods)
				w.Header().Set("Access-Control-Allow-Headers", headers)
				if cfg.AllowCredentials {
					w.Header().Set("Access-Control-Allow-Credentials", "true")
				}
			}

			if r.Method == "OPTIONS" {
				w.WriteHeader(http.StatusNoContent)
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// RecoveryMiddleware recovers from panics in HTTP handlers
func RecoveryMiddleware(next http.Handler, log zerolog.Logger) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		defer func() {
			if rcv := recover(); rcv != nil {
				log.Error().
					Interface("panic", rcv).
					Str("url", r.URL.String()).
					Str("stack", string(debug.Stack())).
					Msg("Panic in HTTP handler")
				http.Error(w, "Internal Server Error", http.StatusInternalServerError)
			}
		}()
		next.ServeHTTP(w, r)
	})
}

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

// GinRateLimitWithLimiter applies per-IP rate limiting with a provided limiter
// This allows external control over the limiter lifecycle (cleanup, stop)
func GinRateLimitWithLimiter(rl *RateLimiter) gin.HandlerFunc {
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

// GinRecovery handles panic recovery with structured logging and stack traces
func GinRecovery(log zerolog.Logger) gin.HandlerFunc {
	return func(c *gin.Context) {
		defer func() {
			if err := recover(); err != nil {
				traceID, _ := c.Get("trace_id")
				log.Error().
					Interface("panic", err).
					Interface("trace_id", traceID).
					Str("path", c.Request.URL.Path).
					Str("stack", string(debug.Stack())).
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

// TimeoutConfig holds timeout middleware configuration
type TimeoutConfig struct {
	Timeout   time.Duration
	SkipPaths []string
}

// GinTimeout adds request timeout middleware with configurable skip paths
func GinTimeout(cfg TimeoutConfig) gin.HandlerFunc {
	skipSet := make(map[string]struct{}, len(cfg.SkipPaths))
	for _, p := range cfg.SkipPaths {
		skipSet[p] = struct{}{}
	}

	return func(c *gin.Context) {
		// Skip timeout for configured paths (e.g., SSE endpoints)
		if _, skip := skipSet[c.Request.URL.Path]; skip {
			c.Next()
			return
		}

		ctx, cancel := context.WithTimeout(c.Request.Context(), cfg.Timeout)
		defer cancel()

		c.Request = c.Request.WithContext(ctx)

		finished := make(chan struct{})

		go func() {
			c.Next()
			close(finished)
		}()

		select {
		case <-finished:
			return
		case <-ctx.Done():
			c.AbortWithStatusJSON(http.StatusGatewayTimeout, handlers.Response{
				Success: false,
				Error:   handlers.ErrInternal("Request timeout"),
			})
		}
	}
}
