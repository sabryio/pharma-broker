package api

import (
	"embed"
	"io/fs"
	"net/http"

	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/rs/zerolog"

	"pharmabroker/internal/config"
)

//go:embed static/dist/*
var staticFiles embed.FS

// NewRouter creates the HTTP router with middleware
func NewRouter(handlers *Handlers, cfg *config.APIConfig, log zerolog.Logger) http.Handler {
	mux := http.NewServeMux()

	// API routes
	mux.HandleFunc("GET /api/offers", handlers.GetOffers)
	mux.HandleFunc("GET /api/offers/{id}", handlers.GetOffer)
	mux.HandleFunc("GET /api/requests", handlers.GetRequests)
	mux.HandleFunc("GET /api/requests/{id}", handlers.GetRequest)
	mux.HandleFunc("GET /api/matches", handlers.GetMatches)
	mux.HandleFunc("GET /api/matches/export", handlers.ExportMatchesCSV)
	mux.HandleFunc("POST /api/matches/{id}/confirm", handlers.ConfirmMatch)
	mux.HandleFunc("POST /api/matches/{id}/reject", handlers.RejectMatch)
	mux.HandleFunc("GET /api/stats", handlers.GetStats)
	mux.HandleFunc("GET /api/groups", handlers.GetGroups)
	mux.HandleFunc("POST /api/groups/sync", handlers.SyncGroups)
	mux.HandleFunc("PATCH /api/groups/{jid}", handlers.UpdateGroupMonitoring)

	// AI Analysis and Config
	mux.HandleFunc("POST /api/analyze", handlers.Analyze)
	mux.HandleFunc("GET /api/config", handlers.GetConfig)
	mux.HandleFunc("PATCH /api/config", handlers.UpdateConfig)

	// SSE endpoint
	mux.HandleFunc("GET /api/events", handlers.sseHub.ServeHTTP)

	// Metrics endpoint
	mux.Handle("GET /metrics", promhttp.Handler())

	// Static files (dashboard) - with SPA fallback
	staticFS, err := fs.Sub(staticFiles, "static/dist")
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create static file system")
	}
	mux.Handle("GET /", SpaHandler(http.FS(staticFS)))

	// Create rate limiter
	rateLimiter := NewRateLimiter(cfg)

	// Apply middleware stack: CORS -> Rate Limit -> Tracing -> Handler
	handler := CorsMiddleware(
		RateLimitMiddleware(rateLimiter)(
			TracingMiddleware(mux, log),
		),
	)

	return handler
}
