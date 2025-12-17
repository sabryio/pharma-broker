package api

import (
	"context"
	"net/http"
	"pharmabroker/api/handlers"
	"pharmabroker/api/middleware"
	"pharmabroker/api/sse"
	"pharmabroker/pkg/config"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/rs/zerolog"
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
	Review      *handlers.ReviewHandler
	Learning    *handlers.LearningHandler
	SSE         *sse.SSEHub
	Health      *handlers.HealthChecker
}

// ServerResources holds resources that need lifecycle management
type ServerResources struct {
	RateLimiter *middleware.RateLimiter
}

// Stop cleans up server resources
func (r *ServerResources) Stop() {
	if r.RateLimiter != nil {
		r.RateLimiter.Stop()
	}
}

// NewGinRouter creates the Gin HTTP router with middleware and routes.
// Returns the router and resources that need cleanup when the server stops.
func NewGinRouter(ctx context.Context, h *Handlers, cfg *config.APIConfig, log zerolog.Logger) (*gin.Engine, *ServerResources) {
	// Gin automatically checks GIN_MODE env var (debug/release/test)
	// For production, set GIN_MODE=release

	r := gin.New()
	resources := &ServerResources{}

	// Create rate limiter with TTL-based cleanup
	rateLimiter := middleware.NewRateLimiterWithTTL(cfg.RateLimitRPS, cfg.RateLimitBurst, 10*time.Minute)
	rateLimiter.StartCleanup(ctx, 5*time.Minute)
	resources.RateLimiter = rateLimiter

	// Global middleware stack
	r.Use(middleware.GinRecovery(log))
	r.Use(middleware.GinTracing(log))
	r.Use(middleware.GinRateLimitWithLimiter(rateLimiter))
	r.Use(middleware.GinTimeout(middleware.TimeoutConfig{
		Timeout:   cfg.RequestTimeout,
		SkipPaths: []string{"/api/events"}, // Skip SSE endpoint
	}))
	r.Use(corsMiddleware(cfg))

	// Register route groups
	api := r.Group("/api")
	{
		registerOfferRoutes(api, h)
		registerRequestRoutes(api, h)
		registerMatchRoutes(api, h)
		registerGroupRoutes(api, h)
		registerStatsRoutes(api, h)
		registerConfigRoutes(api, h)
		registerFeedbackRoutes(api, h)
		registerLeaderboardRoutes(api, h)
		registerAuditRoutes(api, h)
		registerReviewRoutes(api, h)
		registerAnalysisRoutes(api, h)
		registerSSERoutes(api, h)

		// Admin routes
		admin := api.Group("/admin")
		{
			registerLearningRoutes(admin, h)
		}
	}

	// Health check endpoints
	registerHealthRoutes(r, h)

	// Prometheus metrics
	r.GET("/metrics", gin.WrapH(promhttp.Handler()))

	return r, resources
}

// corsMiddleware returns a CORS configuration middleware
func corsMiddleware(cfg *config.APIConfig) gin.HandlerFunc {
	// Use allowed origins from config, default to "*" for development
	allowedOrigins := cfg.CorsAllowedOrigins
	if len(allowedOrigins) == 0 {
		allowedOrigins = []string{"*"}
	}

	return func(c *gin.Context) {
		origin := c.Request.Header.Get("Origin")
		allowAll := len(allowedOrigins) == 1 && allowedOrigins[0] == "*"

		if allowAll {
			c.Header("Access-Control-Allow-Origin", "*")
		} else if origin != "" {
			for _, allowed := range allowedOrigins {
				if origin == allowed {
					c.Header("Access-Control-Allow-Origin", origin)
					c.Header("Vary", "Origin")
					break
				}
			}
		}

		c.Header("Access-Control-Allow-Methods", "GET, POST, PATCH, PUT, DELETE, OPTIONS")
		c.Header("Access-Control-Allow-Headers", "Origin, Content-Type, Authorization, X-Trace-ID")
		c.Header("Access-Control-Expose-Headers", "X-Trace-ID")
		c.Header("Access-Control-Max-Age", "86400")

		if c.Request.Method == "OPTIONS" {
			c.AbortWithStatus(http.StatusNoContent)
			return
		}

		c.Next()
	}
}

// Route registration functions

func registerOfferRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Offer == nil {
		return
	}
	offers := rg.Group("/offers")
	{
		offers.GET("", h.Offer.GetOffersGin)
		offers.GET("/:id", h.Offer.GetOfferGin)
	}
}

func registerRequestRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Request == nil {
		return
	}
	requests := rg.Group("/requests")
	{
		requests.GET("", h.Request.GetRequestsGin)
		requests.GET("/:id", h.Request.GetRequestGin)
	}
}

func registerMatchRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Match == nil {
		return
	}
	matches := rg.Group("/matches")
	{
		matches.GET("", h.Match.GetMatchesGin)
		matches.GET("/export", h.Match.ExportMatchesCSVGin)
		matches.POST("/:id/confirm", h.Match.ConfirmMatchGin)
		matches.POST("/:id/reject", h.Match.RejectMatchGin)
	}
}

func registerGroupRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Group == nil {
		return
	}
	groups := rg.Group("/groups")
	{
		groups.GET("", h.Group.GetGroupsGin)
		groups.POST("/sync", h.Group.SyncGroupsGin)
		groups.PATCH("/:jid", h.Group.UpdateGroupMonitoringGin)
	}
}

func registerStatsRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Stats == nil {
		return
	}
	rg.GET("/stats", h.Stats.GetStatsGin)
}

func registerConfigRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Config == nil {
		return
	}
	rg.GET("/config", h.Config.GetConfigGin)
	rg.PATCH("/config", h.Config.UpdateConfigGin)
}

func registerFeedbackRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Feedback == nil {
		return
	}
	rg.POST("/matches/:id/feedback", h.Feedback.RecordFeedbackGin)

	feedback := rg.Group("/feedback")
	{
		feedback.GET("/analysis", h.Feedback.GetFeedbackAnalysisGin)
		feedback.GET("/recent", h.Feedback.GetRecentFeedbackGin)
	}
}

func registerLeaderboardRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Leaderboard == nil {
		return
	}
	leaderboard := rg.Group("/leaderboard")
	{
		leaderboard.GET("", h.Leaderboard.GetDemandLeaderboardGin)
		leaderboard.GET("/:medication", h.Leaderboard.GetMedicationDemandGin)
		leaderboard.POST("/refresh", h.Leaderboard.RefreshLeaderboardGin)
	}
}

func registerAuditRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Audit == nil {
		return
	}
	rg.GET("/audit", h.Audit.GetAuditLogsGin)
}

func registerReviewRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Review == nil {
		return
	}
	review := rg.Group("/review")
	{
		review.GET("/queue", h.Review.GetPendingReviewsGin)
		review.GET("/count", h.Review.GetReviewCountGin)
		review.GET("/:id", h.Review.GetReviewItemGin)
		review.POST("/:id/approve", h.Review.ApproveReviewGin)
		review.POST("/:id/reject", h.Review.RejectReviewGin)
	}
}

func registerAnalysisRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Analysis == nil {
		return
	}
	rg.POST("/analyze", h.Analysis.AnalyzeGin)
}

func registerSSERoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.SSE == nil {
		return
	}
	rg.GET("/events", h.SSE.GinHandler())
}

func registerLearningRoutes(rg *gin.RouterGroup, h *Handlers) {
	if h.Learning == nil {
		return
	}
	learning := rg.Group("/learning")
	{
		learning.GET("/status", h.Learning.GetLearningStatusGin)
		learning.POST("/trigger", h.Learning.TriggerLearningGin)
		learning.POST("/apply", h.Learning.ApplyPendingWeightsGin)
		learning.POST("/reject", h.Learning.RejectPendingWeightsGin)
		learning.POST("/rollback", h.Learning.RollbackWeightsGin)
		learning.GET("/history", h.Learning.GetWeightHistoryGin)
		learning.GET("/weights", h.Learning.GetCurrentWeightsGin)
		learning.PUT("/weights", h.Learning.UpdateWeightsManuallyGin)
		learning.GET("/feedback-stats", h.Learning.GetFeedbackStatsGin)
	}
}

func registerHealthRoutes(r *gin.Engine, h *Handlers) {
	if h.Health == nil {
		return
	}
	r.GET("/health", h.Health.FullHealthGin)
	r.GET("/health/live", h.Health.LiveGin)
	r.GET("/health/ready", h.Health.ReadyGin)
}

// GinHandlerAdapter wraps an http.Handler to work with Gin
func GinHandlerAdapter(h http.Handler) gin.HandlerFunc {
	return gin.WrapH(h)
}
