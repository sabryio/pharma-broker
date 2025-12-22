package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
)

// LeaderboardHandler handles leaderboard operations
type LeaderboardHandler struct {
	repo repository.LeaderboardRepository
	log  zerolog.Logger
}

// NewLeaderboardHandler creates a new LeaderboardHandler
func NewLeaderboardHandler(repo repository.LeaderboardRepository, log zerolog.Logger) *LeaderboardHandler {
	return &LeaderboardHandler{
		repo: repo,
		log:  log.With().Str("component", "LeaderboardHandler").Logger(),
	}
}

// GetDemandLeaderboardGin returns top medications by demand ratio
func (h *LeaderboardHandler) GetDemandLeaderboardGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Leaderboard service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, _ := GetPaginationGin(c)

	stats, err := h.repo.GetTopDemand(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get demand leaderboard")
		DatabaseErrorGin(c, "Failed to get leaderboard")
		return
	}

	SuccessGin(c, stats)
}

// GetMedicationDemandGin returns demand stats for a specific medication
func (h *LeaderboardHandler) GetMedicationDemandGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Leaderboard service not configured")
		return
	}

	medication, ok := GetPathIDGin(c, "medication")
	if !ok {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.repo.GetDemandForMedication(ctx, medication)
	if err != nil {
		h.log.Error().Err(err).Str("medication", medication).Msg("Failed to get medication demand")
		DatabaseErrorGin(c, "Failed to get demand")
		return
	}

	SuccessGin(c, stats)
}

// RefreshLeaderboardGin triggers a leaderboard refresh
func (h *LeaderboardHandler) RefreshLeaderboardGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Leaderboard service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 10*time.Second)
	defer cancel()

	if err := h.repo.RefreshLeaderboard(ctx); err != nil {
		h.log.Error().Err(err).Msg("Failed to refresh leaderboard")
		DatabaseErrorGin(c, "Failed to refresh leaderboard")
		return
	}

	SuccessGin(c, map[string]interface{}{
		"success":      true,
		"refreshed_at": time.Now(),
	})
}
