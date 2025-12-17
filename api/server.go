package api

import (
	"context"
	"net/http"
	"pharmabroker/api/handlers"
	"pharmabroker/api/middleware"
	"pharmabroker/api/sse"
	"pharmabroker/pkg/config"
	"strconv"
	"strings"
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
	JWTAuth     *middleware.JWTAuth
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

	// JWT Authentication middleware (if enabled)
	var jwtAuth *middleware.JWTAuth
	if cfg.JWT.Enabled && cfg.JWT.Secret != "" {
		var err error
		jwtAuth, err = middleware.NewJWTAuth(middleware.JWTConfig{
			Secret:        cfg.JWT.Secret,
			Issuer:        cfg.JWT.Issuer,
			Audience:      cfg.JWT.Audience,
			TokenExpiry:   time.Duration(cfg.JWT.TokenExpiryHours) * time.Hour,
			RefreshExpiry: time.Duration(cfg.JWT.RefreshExpiryDays) * 24 * time.Hour,
			SkipPaths: []string{
				"/health",
				"/health/live",
				"/health/ready",
				"/metrics",
				"/api/auth/login",
				"/api/auth/refresh",
			},
		})
		if err != nil {
			log.Warn().Err(err).Msg("Failed to initialize JWT auth, running without authentication")
		} else {
			log.Info().Msg("JWT authentication enabled")
			resources.JWTAuth = jwtAuth
		}
	} else {
		log.Warn().Msg("JWT authentication disabled - API endpoints are unprotected")
	}

	// Register route groups
	api := r.Group("/api")
	{
		// Auth routes (always public)
		registerAuthRoutes(api, resources)

		// Apply JWT middleware to protected routes if enabled
		if jwtAuth != nil {
			api.Use(jwtAuth.GinJWT())
		}

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

		// Admin routes (require admin role if JWT enabled)
		admin := api.Group("/admin")
		if jwtAuth != nil {
			admin.Use(jwtAuth.RequireRole("admin"))
		}
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

// corsMiddleware returns a CORS configuration middleware with security best practices
func corsMiddleware(cfg *config.APIConfig) gin.HandlerFunc {
	// Build origin lookup set for O(1) checks
	allowedOrigins := cfg.CorsAllowedOrigins
	if len(allowedOrigins) == 0 {
		allowedOrigins = []string{"*"}
	}
	allowAll := len(allowedOrigins) == 1 && allowedOrigins[0] == "*"
	originSet := make(map[string]struct{}, len(allowedOrigins))
	for _, o := range allowedOrigins {
		originSet[o] = struct{}{}
	}

	// Build method and header strings
	methods := strings.Join(cfg.CorsAllowedMethods, ", ")
	if methods == "" {
		methods = "GET, POST, PATCH, PUT, DELETE, OPTIONS"
	}
	headers := strings.Join(cfg.CorsAllowedHeaders, ", ")
	if headers == "" {
		headers = "Origin, Content-Type, Authorization, X-Trace-ID, X-API-Key"
	}
	exposedHeaders := strings.Join(cfg.CorsExposedHeaders, ", ")
	if exposedHeaders == "" {
		exposedHeaders = "X-Trace-ID"
	}
	maxAge := cfg.CorsMaxAge
	if maxAge <= 0 {
		maxAge = 86400
	}
	maxAgeStr := strconv.Itoa(maxAge)

	return func(c *gin.Context) {
		origin := c.Request.Header.Get("Origin")

		// Determine if origin is allowed
		originAllowed := false
		if allowAll {
			// WARNING: Allow-all should only be used in development
			c.Header("Access-Control-Allow-Origin", "*")
			originAllowed = true
		} else if origin != "" {
			if _, ok := originSet[origin]; ok {
				c.Header("Access-Control-Allow-Origin", origin)
				c.Header("Vary", "Origin")
				originAllowed = true
			}
		}

		// Only set CORS headers if origin is allowed
		if originAllowed {
			c.Header("Access-Control-Allow-Methods", methods)
			c.Header("Access-Control-Allow-Headers", headers)
			c.Header("Access-Control-Expose-Headers", exposedHeaders)
			c.Header("Access-Control-Max-Age", maxAgeStr)

			// Allow credentials only with specific origins (not "*")
			if cfg.CorsAllowCredentials && !allowAll {
				c.Header("Access-Control-Allow-Credentials", "true")
			}
		}

		// Handle preflight requests
		if c.Request.Method == "OPTIONS" {
			if originAllowed {
				c.AbortWithStatus(http.StatusNoContent)
			} else {
				c.AbortWithStatus(http.StatusForbidden)
			}
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

func registerAuthRoutes(rg *gin.RouterGroup, resources *ServerResources) {
	if resources.JWTAuth == nil {
		return
	}
	auth := rg.Group("/auth")
	{
		auth.POST("/login", authLoginHandler(resources.JWTAuth))
		auth.POST("/refresh", authRefreshHandler(resources.JWTAuth))
	}
}

// authLoginHandler handles user login and returns JWT tokens
// This is a placeholder - integrate with your user authentication system
func authLoginHandler(jwtAuth *middleware.JWTAuth) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req struct {
			Username string `json:"username" binding:"required"`
			Password string `json:"password" binding:"required"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(400, handlers.Response{
				Success: false,
				Error:   handlers.ErrBadRequest("invalid request body"),
			})
			return
		}

		// TODO: Implement actual user authentication against your user store
		// This is a placeholder that should be replaced with real authentication
		// For now, reject all logins with a helpful message
		//
		// Example implementation:
		// user, err := userRepo.FindByUsername(req.Username)
		// if err != nil || !user.CheckPassword(req.Password) {
		//     c.JSON(401, handlers.Response{...})
		//     return
		// }
		// token, _ := jwtAuth.GenerateToken(user.ID, user.Username, user.Role, user.Scopes)

		c.JSON(501, handlers.Response{
			Success: false,
			Error:   handlers.NewAPIError("NOT_IMPLEMENTED", "User authentication not configured. Implement user store integration."),
		})
	}
}

// authRefreshHandler handles token refresh
func authRefreshHandler(jwtAuth *middleware.JWTAuth) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req struct {
			RefreshToken string `json:"refresh_token" binding:"required"`
		}

		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(400, handlers.Response{
				Success: false,
				Error:   handlers.ErrBadRequest("invalid request body"),
			})
			return
		}

		// Validate refresh token
		claims, err := jwtAuth.ValidateToken(req.RefreshToken)
		if err != nil {
			c.JSON(401, handlers.Response{
				Success: false,
				Error:   handlers.ErrUnauthorized("invalid or expired refresh token"),
			})
			return
		}

		// Generate new access token
		// Note: In production, you should also verify the refresh token hasn't been revoked
		token, err := jwtAuth.GenerateToken(claims.UserID, claims.Username, claims.Role, claims.Scopes)
		if err != nil {
			c.JSON(500, handlers.Response{
				Success: false,
				Error:   handlers.ErrInternal("failed to generate token"),
			})
			return
		}

		c.JSON(200, handlers.Response{
			Success: true,
			Data: gin.H{
				"access_token": token,
				"token_type":   "Bearer",
				"expires_in":   86400, // 24 hours in seconds
			},
		})
	}
}

// GinHandlerAdapter wraps an http.Handler to work with Gin
func GinHandlerAdapter(h http.Handler) gin.HandlerFunc {
	return gin.WrapH(h)
}
