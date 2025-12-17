package handlers

import (
	"context"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
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

// RecordFeedbackGin records operator feedback on a match
func (h *FeedbackHandler) RecordFeedbackGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Feedback service not configured")
		return
	}

	matchID, ok := ValidateID(c, "id")
	if !ok {
		return
	}

	var req FeedbackRequest
	if !BindAndValidate(c, &req) {
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	match, err := h.matchRepo.GetByID(ctx, matchID)
	if err != nil {
		h.log.Error().Err(err).Str("match_id", matchID).Msg("Failed to get match for feedback")
		NotFoundGin(c, ErrMatchNotFound())
		return
	}

	feedback := &entity.MatchFeedback{
		MatchID:            matchID,
		OperatorID:         req.OperatorID,
		Decision:           entity.FeedbackDecision(req.Decision),
		Reason:             req.Reason,
		OriginalScore:      match.Score,
		OriginalConfidence: match.MatchedBy,
	}

	if err := h.repo.RecordFeedback(ctx, feedback); err != nil {
		h.log.Error().Err(err).Msg("Failed to record feedback")
		DatabaseErrorGin(c, "Failed to record feedback")
		return
	}

	h.log.Info().
		Str("match_id", matchID).
		Str("decision", req.Decision).
		Float64("original_score", match.Score).
		Msg("Feedback recorded")

	SuccessGin(c, map[string]interface{}{
		"success":  true,
		"feedback": feedback,
	})
}

// GetFeedbackAnalysisGin returns aggregated feedback statistics
func (h *FeedbackHandler) GetFeedbackAnalysisGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Feedback service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	days := GetQueryInt(c, "days", 30)

	analysis, err := h.repo.AnalyzeFeedback(ctx, days)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to analyze feedback")
		DatabaseErrorGin(c, "Failed to analyze feedback")
		return
	}

	SuccessGin(c, analysis)
}

// GetRecentFeedbackGin returns recent feedback entries
func (h *FeedbackHandler) GetRecentFeedbackGin(c *gin.Context) {
	if h.repo == nil {
		InternalErrorGin(c, "Feedback service not configured")
		return
	}

	ctx, cancel := context.WithTimeout(c.Request.Context(), 5*time.Second)
	defer cancel()

	limit, _ := GetPaginationGin(c)

	feedback, err := h.repo.GetRecentFeedback(ctx, limit)
	if err != nil {
		h.log.Error().Err(err).Msg("Failed to get recent feedback")
		DatabaseErrorGin(c, "Failed to get feedback")
		return
	}

	SuccessGin(c, feedback)
}
