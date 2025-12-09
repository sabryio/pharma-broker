package api

import (
	"net/http"

	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/rs/zerolog"

	"pharmabroker/api/handlers"
	"pharmabroker/api/middleware"
	"pharmabroker/api/sse"
	"pharmabroker/internal/config"
)

// Handlers bundles all API handlers
type Handlers struct {
	Offer       *handlers.OfferHandler
	Request     *handlers.RequestHandler
	Match       *handlers.MatchHandler
	Group       *handlers.GroupHandler
	Stats       *handlers.StatsHandler
	Analysis    *handlers.AnalysisHandler
	Config      *handlers.ConfigHandler
	Feedback    *handlers.FeedbackHandler
	Leaderboard *handlers.LeaderboardHandler
	Audit       *handlers.AuditHandler
	// Learning    *handlers.LearningHandler
	SSE         *sse.SSEHub
	Health      *handlers.HealthChecker
}

// NewRouter creates the HTTP router with middleware
func NewRouter(h *Handlers, cfg *config.APIConfig, log zerolog.Logger) http.Handler {
	mux := http.NewServeMux()

	// API routes
	if h.Offer != nil {
		mux.HandleFunc("GET /api/offers", h.Offer.GetOffers)
		mux.HandleFunc("GET /api/offers/{id}", h.Offer.GetOffer)
	}

	if h.Request != nil {
		mux.HandleFunc("GET /api/requests", h.Request.GetRequests)
		mux.HandleFunc("GET /api/requests/{id}", h.Request.GetRequest)
	}

	if h.Match != nil {
		mux.HandleFunc("GET /api/matches", h.Match.GetMatches)
		mux.HandleFunc("GET /api/matches/export", h.Match.ExportMatchesCSV)
		mux.HandleFunc("POST /api/matches/{id}/confirm", h.Match.ConfirmMatch)
		mux.HandleFunc("POST /api/matches/{id}/reject", h.Match.RejectMatch)
	}

	if h.Stats != nil {
		mux.HandleFunc("GET /api/stats", h.Stats.GetStats)
	}

	if h.Group != nil {
		mux.HandleFunc("GET /api/groups", h.Group.GetGroups)
		mux.HandleFunc("POST /api/groups/sync", h.Group.SyncGroups)
		mux.HandleFunc("PATCH /api/groups/{jid}", h.Group.UpdateGroupMonitoring)
	}

	if h.Analysis != nil {
		mux.HandleFunc("POST /api/analyze", h.Analysis.Analyze)
	}

	if h.Config != nil {
		mux.HandleFunc("GET /api/config", h.Config.GetConfig)
		mux.HandleFunc("PATCH /api/config", h.Config.UpdateConfig)
	}

	if h.Feedback != nil {
		mux.HandleFunc("POST /api/matches/{id}/feedback", h.Feedback.RecordFeedback)
		mux.HandleFunc("GET /api/feedback/analysis", h.Feedback.GetFeedbackAnalysis)
		mux.HandleFunc("GET /api/feedback/recent", h.Feedback.GetRecentFeedback)
	}

	if h.Leaderboard != nil {
		mux.HandleFunc("GET /api/leaderboard", h.Leaderboard.GetDemandLeaderboard)
		mux.HandleFunc("GET /api/leaderboard/{medication}", h.Leaderboard.GetMedicationDemand)
		mux.HandleFunc("POST /api/leaderboard/refresh", h.Leaderboard.RefreshLeaderboard)
	}

	if h.Audit != nil {
		mux.HandleFunc("GET /api/audit", h.Audit.GetAuditLogs)
	}

	// if h.Learning != nil {
	// 	mux.HandleFunc("GET /api/admin/learning/status", h.Learning.GetLearningStatus)
	// 	mux.HandleFunc("POST /api/admin/learning/trigger", h.Learning.TriggerLearning)
	// 	mux.HandleFunc("POST /api/admin/learning/apply", h.Learning.ApplyPendingWeights)
	// 	mux.HandleFunc("POST /api/admin/learning/reject", h.Learning.RejectPendingWeights)
	// 	mux.HandleFunc("POST /api/admin/learning/rollback", h.Learning.RollbackWeights)
	// 	mux.HandleFunc("GET /api/admin/learning/history", h.Learning.GetWeightHistory)
	// 	mux.HandleFunc("GET /api/admin/learning/weights", h.Learning.GetCurrentWeights)
	// 	mux.HandleFunc("PUT /api/admin/learning/weights", h.Learning.UpdateWeightsManually)
	// 	mux.HandleFunc("GET /api/admin/learning/feedback-stats", h.Learning.GetFeedbackStats)
	// }

	if h.SSE != nil {
		mux.HandleFunc("GET /api/events", h.SSE.ServeHTTP)
	}

	// Health check endpoints
	if h.Health != nil {
		mux.HandleFunc("GET /health", h.Health.FullHealthHandler)
		mux.HandleFunc("GET /health/live", h.Health.LiveHandler)
		mux.HandleFunc("GET /health/ready", h.Health.ReadyHandler)
	}

	// Metrics endpoint
	mux.Handle("GET /metrics", promhttp.Handler())

	// Middleware stack
	rateLimiter := middleware.NewRateLimiter(cfg.RateLimitRPS, cfg.RateLimitBurst)

	// Apply middleware: CORS -> Recovery -> Rate Limit -> Tracing -> Handler
	// Using chained middleware helper or explicit nested calls
	handler := middleware.CorsMiddleware(
		middleware.RecoveryMiddleware(
			middleware.RateLimitMiddleware(rateLimiter)(
				middleware.TracingMiddleware(mux, log),
			),
			log,
		),
	)

	return handler
}
