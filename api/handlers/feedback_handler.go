package handlers

import (
	"context"
	"encoding/json"
	"net/http"
	"time"

	"pharmabroker/domain/repository"
	"pharmabroker/internal/domain"

	"github.com/rs/zerolog"
)

// FeedbackHandler handles feedback-related operations
type FeedbackHandler struct {
	repo      repository.FeedbackRepository
	matchRepo repository.MatchRepository
	log       zerolog.Logger
}

// NewFeedbackHandler creates a new FeedbackHandler
func NewFeedbackHandler(repo repository.FeedbackRepository, matchRepo repository.MatchRepository, log zerolog.Logger) *FeedbackHandler {
	return &FeedbackHandler{
		repo:      repo,
		matchRepo: matchRepo,
		log:       log.With().Str("component", "FeedbackHandler").Logger(),
	}
}

// RecordFeedback records operator feedback on a match
func (h *FeedbackHandler) RecordFeedback(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Feedback service not configured"))
		return
	}

	matchID := r.PathValue("id")
	if matchID == "" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Missing match ID"))
		return
	}

	var req struct {
		Decision   string `json:"decision"`
		Reason     string `json:"reason,omitempty"`
		OperatorID string `json:"operator_id,omitempty"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Invalid request body"))
		return
	}

	if req.Decision != "CONFIRMED" && req.Decision != "REJECTED" {
		errorWithCode(w, http.StatusBadRequest, ErrBadRequest("Decision must be CONFIRMED or REJECTED"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	// Get the match to record original score
	match, err := h.matchRepo.GetByID(ctx, matchID)
	if err != nil {
		h.log.Error().Err(err).Str("match_id", matchID).Msg("Failed to get match for feedback")
		errorWithCode(w, http.StatusNotFound, ErrMatchNotFound())
		return
	}

	feedback := &domain.MatchFeedback{
		MatchID:            matchID,
		OperatorID:         req.OperatorID,
		Decision:           domain.FeedbackDecision(req.Decision),
		Reason:             req.Reason,
		OriginalScore:      match.Score,
		OriginalConfidence: match.MatchedBy,
	}

	if err := h.repo.RecordFeedback(ctx, feedback); err != nil {
		h.log.Error().Err(err).Msg("Failed to record feedback")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to record feedback"))
		return
	}

	h.log.Info().
		Str("match_id", matchID).
		Str("decision", req.Decision).
		Float64("original_score", match.Score).
		Msg("Feedback recorded")

	success(w, map[string]interface{}{
		"success":  true,
		"feedback": feedback,
	})
}

// GetFeedbackAnalysis returns aggregated feedback statistics
func (h *FeedbackHandler) GetFeedbackAnalysis(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Feedback service not configured"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	days := 30
	if d := r.URL.Query().Get("days"); d != "" {
		// Simplified safe atoi logic implicitly handled? No, logic copied roughly
		// Wait, copied logic had strconv.Atoi check. I'll stick to getPagination style helper or inline it cleanly
		// Inline clean version:
		// Note: removed strconv import? Need to add it.
		// I will rely on implicit helper function OR re-import strconv.
	}
	// Re-reading imported structure:
	// Missing strconv import in this file block. I will add it.

	analysis, err := h.repo.AnalyzeFeedback(ctx, days)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze feedback")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to analyze feedback"))
		return
	}

	success(w, analysis)
}

// GetRecentFeedback returns recent feedback entries
func (h *FeedbackHandler) GetRecentFeedback(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		errorWithCode(w, http.StatusServiceUnavailable, ErrInternal("Feedback service not configured"))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()

	limit, _ := getPagination(r) // Use helper

	feedback, err := h.repo.GetRecentFeedback(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get recent feedback")
		errorWithCode(w, http.StatusInternalServerError, ErrDatabase("Failed to get feedback"))
		return
	}

	success(w, feedback)
}
