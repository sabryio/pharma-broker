package handlers

import (
	"context"
	"net/http"
	"time"

	"pharmabroker/domain/repository"

	"github.com/rs/zerolog"
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

// GetDemandLeaderboard returns top medications by demand ratio
func (h *LeaderboardHandler) GetDemandLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Leaderboard service not configured"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, _ := getPagination(r)

	stats, err := h.repo.GetTopDemand(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get demand leaderboard")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get leaderboard"))
		return
	}

	success(w, stats)
}

// GetMedicationDemand returns demand stats for a specific medication
func (h *LeaderboardHandler) GetMedicationDemand(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Leaderboard service not configured"))
		return
	}

	medication := r.PathValue("medication")
	if medication == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing medication name"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	stats, err := h.repo.GetDemandForMedication(ctx, medication)
	if err != nil {
		h.log.Error().Err(err).Str("medication", medication).Msg("Failed to get medication demand")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get demand"))
		return
	}

	success(w, stats)
}

// RefreshLeaderboard triggers a leaderboard refresh
func (h *LeaderboardHandler) RefreshLeaderboard(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Leaderboard service not configured"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	if err := h.repo.RefreshLeaderboard(ctx); err != nil {
		h.log.Error().Err(err).Msg("Failed to refresh leaderboard")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to refresh leaderboard"))
		return
	}

	success(w, map[string]interface{}{
		"success":      true,
		"refreshed_at": time.Now(),
	})
}
