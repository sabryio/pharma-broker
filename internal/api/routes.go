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

// NewRouter creates the HTTP router with middleware (backwards compatible)
func NewRouter(handlers *Handlers, cfg *config.APIConfig, log zerolog.Logger) http.Handler {
	return NewRouterWithLearning(handlers, nil, cfg, log)
}

// NewRouterWithLearning creates the HTTP router with adaptive learning support
func NewRouterWithLearning(handlers *Handlers, learningHandlers *LearningHandlers, cfg *config.APIConfig, log zerolog.Logger) http.Handler {
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

	// Feedback endpoints (learning loop)
	mux.HandleFunc("POST /api/matches/{id}/feedback", handlers.RecordFeedback)
	mux.HandleFunc("GET /api/feedback/analysis", handlers.GetFeedbackAnalysis)
	mux.HandleFunc("GET /api/feedback/recent", handlers.GetRecentFeedback)

	// Demand leaderboard endpoints
	mux.HandleFunc("GET /api/leaderboard", handlers.GetDemandLeaderboard)
	mux.HandleFunc("GET /api/leaderboard/{medication}", handlers.GetMedicationDemand)
	mux.HandleFunc("POST /api/leaderboard/refresh", handlers.RefreshLeaderboard)

	// Audit log endpoints
	mux.HandleFunc("GET /api/audit", handlers.GetAuditLogs)

	// Admin Learning endpoints (adaptive weight learning)
	// Only registered if learning handlers are provided
	if learningHandlers != nil {
		mux.HandleFunc("GET /api/admin/learning/status", learningHandlers.GetLearningStatus)
		mux.HandleFunc("POST /api/admin/learning/trigger", learningHandlers.TriggerLearning)
		mux.HandleFunc("POST /api/admin/learning/apply", learningHandlers.ApplyPendingWeights)
		mux.HandleFunc("POST /api/admin/learning/reject", learningHandlers.RejectPendingWeights)
		mux.HandleFunc("POST /api/admin/learning/rollback", learningHandlers.RollbackWeights)
		mux.HandleFunc("GET /api/admin/learning/history", learningHandlers.GetWeightHistory)
		mux.HandleFunc("GET /api/admin/learning/weights", learningHandlers.GetCurrentWeights)
		mux.HandleFunc("PUT /api/admin/learning/weights", learningHandlers.UpdateWeightsManually)
		mux.HandleFunc("GET /api/admin/learning/feedback-stats", learningHandlers.GetFeedbackStats)
	}

	// SSE endpoint
	mux.HandleFunc("GET /api/events", handlers.sseHub.ServeHTTP)

	// Health check endpoints
	healthChecker := NewHealthChecker()
	mux.HandleFunc("GET /health", healthChecker.FullHealthHandler)
	mux.HandleFunc("GET /health/live", healthChecker.LiveHandler)
	mux.HandleFunc("GET /health/ready", healthChecker.ReadyHandler)

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

	// Apply middleware stack: CORS -> Recovery -> Rate Limit -> Tracing -> Handler
	handler := CorsMiddleware(
		RecoveryMiddleware(
			RateLimitMiddleware(rateLimiter)(
				TracingMiddleware(mux, log),
			),
			log,
		),
	)

	return handler
}
