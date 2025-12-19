package parsing

import (
	"context"
	"pharmabroker/domain/entity"
	"pharmabroker/matching"
	"sync/atomic"
	"time"

	"github.com/rs/zerolog"
)

// =============================================================================
// Auto-Action Configuration
// =============================================================================

// ActionType defines what action to take for a match.
type ActionType string

const (
	ActionAutoConfirm ActionType = "AUTO_CONFIRM" // Automatically confirm the match
	ActionSuggest     ActionType = "SUGGEST"      // Suggest to operator (notify)
	ActionReview      ActionType = "REVIEW"       // Queue for manual review
	ActionIgnore      ActionType = "IGNORE"       // Ignore (don't save)
)

// AutoActionConfig holds configuration for automatic match actions.
type AutoActionConfig struct {
	// Actions for each confidence band
	AutoBandAction    ActionType // Action for AUTO confidence (>= 0.9)
	SuggestBandAction ActionType // Action for SUGGEST confidence (0.7-0.9)
	ReviewBandAction  ActionType // Action for REVIEW confidence (0.5-0.7)

	// Notification settings
	EnableNotifications bool // Send notifications for suggested matches

	// Auto-confirm settings
	AutoConfirmEnabled     bool    // Master switch for auto-confirmation
	MinScoreForAutoConfirm float64 // Minimum score to auto-confirm (default: 0.9)
}

// DefaultAutoActionConfig returns sensible defaults for auto-actions.
func DefaultAutoActionConfig() AutoActionConfig {
	return AutoActionConfig{
		AutoBandAction:         ActionAutoConfirm,
		SuggestBandAction:      ActionSuggest,
		ReviewBandAction:       ActionReview,
		EnableNotifications:    true,
		AutoConfirmEnabled:     true,
		MinScoreForAutoConfirm: DefaultAutoConfirmThreshold,
	}
}

// AutoActionStats tracks auto-action statistics.
type AutoActionStats struct {
	TotalMatches      atomic.Int64 // Total matches processed
	AutoConfirmed     atomic.Int64 // Matches auto-confirmed
	Suggested         atomic.Int64 // Matches suggested to operator
	QueuedForReview   atomic.Int64 // Matches queued for review
	Ignored           atomic.Int64 // Matches ignored
	NotificationsSent atomic.Int64 // Notifications sent
}

// GetStats returns a snapshot of auto-action statistics.
func (s *AutoActionStats) GetStats() map[string]int64 {
	return map[string]int64{
		"total_matches":      s.TotalMatches.Load(),
		"auto_confirmed":     s.AutoConfirmed.Load(),
		"suggested":          s.Suggested.Load(),
		"queued_for_review":  s.QueuedForReview.Load(),
		"ignored":            s.Ignored.Load(),
		"notifications_sent": s.NotificationsSent.Load(),
	}
}

// =============================================================================
// Auto-Action Handler
// =============================================================================

// MatchNotifier interface for sending match notifications.
type MatchNotifier interface {
	NotifyNewMatch(ctx context.Context, match *entity.Match, offer *entity.Offer, request *entity.Request) error
	NotifySuggestedMatch(ctx context.Context, match *entity.Match, offer *entity.Offer, request *entity.Request) error
}

// AutoActionHandler handles automatic actions based on match confidence.
type AutoActionHandler struct {
	config   AutoActionConfig
	stats    AutoActionStats
	notifier MatchNotifier
	log      zerolog.Logger
}

// NewAutoActionHandler creates a new auto-action handler.
func NewAutoActionHandler(cfg AutoActionConfig, notifier MatchNotifier, log zerolog.Logger) *AutoActionHandler {
	if cfg.MinScoreForAutoConfirm <= 0 {
		cfg.MinScoreForAutoConfirm = DefaultAutoConfirmThreshold
	}

	return &AutoActionHandler{
		config:   cfg,
		notifier: notifier,
		log:      log.With().Str("component", "auto-action").Logger(),
	}
}

// MatchActionResult contains the result of processing a match action.
type MatchActionResult struct {
	Action           ActionType
	Status           entity.MatchStatus
	ShouldSave       bool
	ShouldNotify     bool
	NotificationType string // "new_match", "suggested_match", etc.
}

// DetermineAction determines what action to take for a match based on confidence.
func (h *AutoActionHandler) DetermineAction(score *matching.MatchScore) MatchActionResult {
	h.stats.TotalMatches.Add(1)

	var action ActionType
	var status entity.MatchStatus
	var shouldNotify bool
	var notificationType string

	switch score.Confidence {
	case matching.ConfidenceAuto:
		action = h.config.AutoBandAction
		if action == ActionAutoConfirm && h.config.AutoConfirmEnabled && score.Total >= h.config.MinScoreForAutoConfirm {
			status = entity.MatchStatusConfirmed
			h.stats.AutoConfirmed.Add(1)
			shouldNotify = h.config.EnableNotifications
			notificationType = "auto_confirmed"
			h.log.Info().
				Float64("score", score.Total).
				Str("confidence", string(score.Confidence)).
				Msg("🤖 Auto-confirming high-confidence match")
		} else {
			status = entity.MatchStatusPending
			action = ActionSuggest
			h.stats.Suggested.Add(1)
		}

	case matching.ConfidenceSuggest:
		action = h.config.SuggestBandAction
		status = entity.MatchStatusPending
		h.stats.Suggested.Add(1)
		shouldNotify = h.config.EnableNotifications
		notificationType = "suggested_match"
		h.log.Info().
			Float64("score", score.Total).
			Str("confidence", string(score.Confidence)).
			Msg("💡 Suggesting match to operator")

	case matching.ConfidenceReview:
		action = h.config.ReviewBandAction
		status = entity.MatchStatusPending
		h.stats.QueuedForReview.Add(1)
		h.log.Info().
			Float64("score", score.Total).
			Str("confidence", string(score.Confidence)).
			Msg("📋 Queuing match for review")

	default: // ConfidenceNone
		action = ActionIgnore
		h.stats.Ignored.Add(1)
		return MatchActionResult{
			Action:     action,
			ShouldSave: false,
		}
	}

	return MatchActionResult{
		Action:           action,
		Status:           status,
		ShouldSave:       true,
		ShouldNotify:     shouldNotify,
		NotificationType: notificationType,
	}
}

// ProcessMatchAction processes the action for a match.
func (h *AutoActionHandler) ProcessMatchAction(
	ctx context.Context,
	match *entity.Match,
	offer *entity.Offer,
	request *entity.Request,
	result MatchActionResult,
) {
	// Update match status
	match.Status = result.Status

	// Set confirmed timestamp for auto-confirmed matches
	if result.Status == entity.MatchStatusConfirmed {
		now := time.Now()
		match.ConfirmedAt = &now
		match.MatchedBy = "AUTO"
	}

	// Send notification if needed
	if result.ShouldNotify && h.notifier != nil {
		var err error
		switch result.NotificationType {
		case "auto_confirmed":
			err = h.notifier.NotifyNewMatch(ctx, match, offer, request)
		case "suggested_match":
			err = h.notifier.NotifySuggestedMatch(ctx, match, offer, request)
		}

		if err != nil {
			h.log.Error().Err(err).Str("type", result.NotificationType).Msg("Failed to send notification")
		} else {
			h.stats.NotificationsSent.Add(1)
		}
	}
}

// GetStats returns the current auto-action statistics.
func (h *AutoActionHandler) GetStats() map[string]int64 {
	return h.stats.GetStats()
}

// GetConfig returns the current configuration.
func (h *AutoActionHandler) GetConfig() AutoActionConfig {
	return h.config
}

// SetConfig updates the configuration.
func (h *AutoActionHandler) SetConfig(cfg AutoActionConfig) {
	h.config = cfg
	h.log.Info().
		Str("auto_band", string(cfg.AutoBandAction)).
		Str("suggest_band", string(cfg.SuggestBandAction)).
		Str("review_band", string(cfg.ReviewBandAction)).
		Bool("auto_confirm", cfg.AutoConfirmEnabled).
		Msg("Auto-action configuration updated")
}

// EnableAutoConfirm enables or disables auto-confirmation.
func (h *AutoActionHandler) EnableAutoConfirm(enabled bool) {
	h.config.AutoConfirmEnabled = enabled
	h.log.Info().
		Bool("enabled", enabled).
		Msg("Auto-confirm toggled")
}

// SetMinScoreForAutoConfirm sets the minimum score for auto-confirmation.
func (h *AutoActionHandler) SetMinScoreForAutoConfirm(score float64) {
	if score > 0 && score <= 1.0 {
		h.config.MinScoreForAutoConfirm = score
		h.log.Info().
			Float64("min_score", score).
			Msg("Auto-confirm minimum score updated")
	}
}

// EnableNotifications enables or disables notifications.
func (h *AutoActionHandler) EnableNotifications(enabled bool) {
	h.config.EnableNotifications = enabled
	h.log.Info().
		Bool("enabled", enabled).
		Msg("Notifications toggled")
}

// SetNotifier sets the match notifier.
func (h *AutoActionHandler) SetNotifier(notifier MatchNotifier) {
	h.notifier = notifier
}
