package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/repository"
)

// StatsHandler handles statistics-related operations
type StatsHandler struct {
	repo repository.StatsRepository
	log  zerolog.Logger
}

// NewStatsHandler creates a new StatsHandler
func NewStatsHandler(repo repository.StatsRepository, log zerolog.Logger) *StatsHandler {
	return &StatsHandler{
		repo: repo,
		log:  log.With().Str("component", "StatsHandler").Logger(),
	}
}

// ============================================================================
// Gin Handlers
// ============================================================================

// GetStatsGin returns dashboard statistics (Gin)
func (h *StatsHandler) GetStatsGin(c *gin.Context) {
	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.repo.GetStats(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get stats")
		DatabaseErrorGin(c, "Failed to fetch stats")
		return
	}

	SuccessGin(c, stats)
}
