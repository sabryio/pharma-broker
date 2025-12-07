package api

import (
	"context"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
	"golang.org/x/time/rate"

	"pharmabroker/internal/config"
)

// contextKey is a custom type for context keys to avoid collisions
type contextKey string

const (
	// TraceIDKey is the context key for trace ID
	TraceIDKey contextKey = "trace_id"
)

// GetTraceID extracts the trace ID from context
func GetTraceID(ctx context.Context) string {
	if id, ok := ctx.Value(TraceIDKey).(string); ok {
		return id
	}
	return ""
}

// TracingMiddleware injects/propagates trace IDs for request tracing
func TracingMiddleware(next http.Handler, log zerolog.Logger) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Get or generate trace ID
		traceID := r.Header.Get("X-Trace-ID")
		if traceID == "" {
			traceID = uuid.New().String()[:8] // Short trace ID for readability
		}

		// Add to context
		ctx := context.WithValue(r.Context(), TraceIDKey, traceID)

		// Add to response header
		w.Header().Set("X-Trace-ID", traceID)

		// Log with trace ID
		log.Debug().
			Str("trace_id", traceID).
			Str("method", r.Method).
			Str("path", r.URL.Path).
			Msg("Request")

		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// RateLimiter manages per-IP rate limiting
type RateLimiter struct {
	limiters map[string]*rate.Limiter
	mu       sync.RWMutex
	rps      float64
	burst    int
}

// NewRateLimiter creates a new rate limiter with configurable limits
func NewRateLimiter(cfg *config.APIConfig) *RateLimiter {
	rps := cfg.RateLimitRPS
	if rps <= 0 {
		rps = 10.0
	}
	burst := cfg.RateLimitBurst
	if burst <= 0 {
		burst = 20
	}
	return &RateLimiter{
		limiters: make(map[string]*rate.Limiter),
		rps:      rps,
		burst:    burst,
	}
}

// getLimiter returns the rate limiter for a given IP
func (rl *RateLimiter) getLimiter(ip string) *rate.Limiter {
	rl.mu.RLock()
	limiter, exists := rl.limiters[ip]
	rl.mu.RUnlock()

	if exists {
		return limiter
	}

	rl.mu.Lock()
	defer rl.mu.Unlock()

	// Double-check after acquiring write lock
	if limiter, exists = rl.limiters[ip]; exists {
		return limiter
	}

	limiter = rate.NewLimiter(rate.Limit(rl.rps), rl.burst)
	rl.limiters[ip] = limiter

	// Cleanup old limiters periodically (simple approach: cap size)
	if len(rl.limiters) > 10000 {
		// Remove some old entries (simple eviction)
		count := 0
		for k := range rl.limiters {
			delete(rl.limiters, k)
			count++
			if count >= 1000 {
				break
			}
		}
	}

	return limiter
}

// RateLimitMiddleware applies rate limiting per IP
func RateLimitMiddleware(rl *RateLimiter) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Get client IP
			ip := r.RemoteAddr
			if forwarded := r.Header.Get("X-Forwarded-For"); forwarded != "" {
				ip = forwarded
			}

			limiter := rl.getLimiter(ip)
			if !limiter.Allow() {
				http.Error(w, "Too Many Requests", http.StatusTooManyRequests)
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// PaginationLimitMiddleware enforces maximum page size
func EnforcePaginationLimit(limit, maxLimit int) int {
	if limit <= 0 {
		return 20 // Default
	}
	if limit > maxLimit {
		return maxLimit
	}
	return limit
}

// CleanupRoutine periodically cleans up stale rate limiters
func (rl *RateLimiter) StartCleanup(ctx context.Context, interval time.Duration) {
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()

		for {
			select {
			case <-ctx.Done():
				return
			case <-ticker.C:
				rl.mu.Lock()
				// Keep only last 5000 entries if over 10000
				if len(rl.limiters) > 10000 {
					newMap := make(map[string]*rate.Limiter, 5000)
					count := 0
					for k, v := range rl.limiters {
						if count >= 5000 {
							break
						}
						newMap[k] = v
						count++
					}
					rl.limiters = newMap
				}
				rl.mu.Unlock()
			}
		}
	}()
}

// SpaHandler serves static files and falls back to index.html for SPA routing
func SpaHandler(fsys http.FileSystem) http.Handler {
	fileServer := http.FileServer(fsys)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		path := r.URL.Path

		// Try to open the file
		f, err := fsys.Open(path)
		if err != nil {
			// If file doesn't exist and it's not an API/static asset, serve index.html
			if !strings.HasPrefix(path, "/api") && !strings.Contains(path, ".") {
				r.URL.Path = "/"
			}
		} else {
			f.Close()
		}

		fileServer.ServeHTTP(w, r)
	})
}

// CorsMiddleware handles CORS preflight and simple requests
func CorsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusNoContent)
			return
		}

		next.ServeHTTP(w, r)
	})
}

// RecoveryMiddleware recovers from panics in HTTP handlers
func RecoveryMiddleware(next http.Handler, log zerolog.Logger) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		defer func() {
			if rcv := recover(); rcv != nil {
				log.Error().
					Interface("panic", rcv).
					Str("url", r.URL.String()).
					Msg("Panic in HTTP handler")
				http.Error(w, "Internal Server Error", http.StatusInternalServerError)
			}
		}()
		next.ServeHTTP(w, r)
	})
}
