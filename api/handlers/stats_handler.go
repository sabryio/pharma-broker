package handlers

import (
	"context"
	"net/http"
	"time"

	"pharmabroker/domain/repository"

	"github.com/rs/zerolog"
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

// GetStats returns dashboard statistics
func (h *StatsHandler) GetStats(w http.ResponseWriter, r *http.Request) {
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.repo.GetStats(ctx)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get stats")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to fetch stats"))
		return
	}

	success(w, stats)
}
