package parsing

import (
	"context"
	"testing"

	"github.com/rs/zerolog"

	"pharmabroker/domain/entity"
	"pharmabroker/matching"
)

// =============================================================================
// AutoActionConfig Tests
// =============================================================================

func TestDefaultAutoActionConfig(t *testing.T) {
	cfg := DefaultAutoActionConfig()

	if cfg.AutoBandAction != ActionAutoConfirm {
		t.Errorf("AutoBandAction = %s, want %s", cfg.AutoBandAction, ActionAutoConfirm)
	}
	if cfg.SuggestBandAction != ActionSuggest {
		t.Errorf("SuggestBandAction = %s, want %s", cfg.SuggestBandAction, ActionSuggest)
	}
	if cfg.ReviewBandAction != ActionReview {
		t.Errorf("ReviewBandAction = %s, want %s", cfg.ReviewBandAction, ActionReview)
	}
	if !cfg.AutoConfirmEnabled {
		t.Error("AutoConfirmEnabled should be true by default")
	}
	if !cfg.EnableNotifications {
		t.Error("EnableNotifications should be true by default")
	}
	if cfg.MinScoreForAutoConfirm != DefaultAutoConfirmThreshold {
		t.Errorf("MinScoreForAutoConfirm = %f, want %f", cfg.MinScoreForAutoConfirm, DefaultAutoConfirmThreshold)
	}
}

// =============================================================================
// AutoActionStats Tests
// =============================================================================

func TestAutoActionStats_GetStats(t *testing.T) {
	stats := &AutoActionStats{}
	stats.TotalMatches.Store(100)
	stats.AutoConfirmed.Store(30)
	stats.Suggested.Store(40)
	stats.QueuedForReview.Store(20)
	stats.Ignored.Store(10)
	stats.NotificationsSent.Store(50)

	result := stats.GetStats()

	if result["total_matches"] != 100 {
		t.Errorf("total_matches = %d, want 100", result["total_matches"])
	}
	if result["auto_confirmed"] != 30 {
		t.Errorf("auto_confirmed = %d, want 30", result["auto_confirmed"])
	}
	if result["suggested"] != 40 {
		t.Errorf("suggested = %d, want 40", result["suggested"])
	}
}

// =============================================================================
// DetermineAction Tests
// =============================================================================

func TestAutoActionHandler_DetermineAction_AutoConfirm(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	score := &matching.MatchScore{
		Total:      0.95,
		Confidence: matching.ConfidenceAuto,
	}

	result := handler.DetermineAction(score)

	if result.Action != ActionAutoConfirm {
		t.Errorf("Action = %s, want %s", result.Action, ActionAutoConfirm)
	}
	if result.Status != entity.MatchStatusConfirmed {
		t.Errorf("Status = %s, want %s", result.Status, entity.MatchStatusConfirmed)
	}
	if !result.ShouldSave {
		t.Error("ShouldSave should be true")
	}
	if !result.ShouldNotify {
		t.Error("ShouldNotify should be true for auto-confirmed")
	}
}

func TestAutoActionHandler_DetermineAction_Suggest(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	score := &matching.MatchScore{
		Total:      0.8,
		Confidence: matching.ConfidenceSuggest,
	}

	result := handler.DetermineAction(score)

	if result.Action != ActionSuggest {
		t.Errorf("Action = %s, want %s", result.Action, ActionSuggest)
	}
	if result.Status != entity.MatchStatusPending {
		t.Errorf("Status = %s, want %s", result.Status, entity.MatchStatusPending)
	}
	if !result.ShouldSave {
		t.Error("ShouldSave should be true")
	}
}

func TestAutoActionHandler_DetermineAction_Review(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	score := &matching.MatchScore{
		Total:      0.6,
		Confidence: matching.ConfidenceReview,
	}

	result := handler.DetermineAction(score)

	if result.Action != ActionReview {
		t.Errorf("Action = %s, want %s", result.Action, ActionReview)
	}
	if result.Status != entity.MatchStatusPending {
		t.Errorf("Status = %s, want %s", result.Status, entity.MatchStatusPending)
	}
}

func TestAutoActionHandler_DetermineAction_Ignore(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	score := &matching.MatchScore{
		Total:      0.3,
		Confidence: matching.ConfidenceNone,
	}

	result := handler.DetermineAction(score)

	if result.Action != ActionIgnore {
		t.Errorf("Action = %s, want %s", result.Action, ActionIgnore)
	}
	if result.ShouldSave {
		t.Error("ShouldSave should be false for ignored matches")
	}
}

func TestAutoActionHandler_DetermineAction_AutoConfirmDisabled(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultAutoActionConfig()
	cfg.AutoConfirmEnabled = false
	handler := NewAutoActionHandler(cfg, nil, log)

	score := &matching.MatchScore{
		Total:      0.95,
		Confidence: matching.ConfidenceAuto,
	}

	result := handler.DetermineAction(score)

	// Should fall back to suggest when auto-confirm is disabled
	if result.Status != entity.MatchStatusPending {
		t.Errorf("Status = %s, want %s (auto-confirm disabled)", result.Status, entity.MatchStatusPending)
	}
}

func TestAutoActionHandler_DetermineAction_BelowMinScore(t *testing.T) {
	log := zerolog.Nop()
	cfg := DefaultAutoActionConfig()
	cfg.MinScoreForAutoConfirm = 0.95 // High threshold
	handler := NewAutoActionHandler(cfg, nil, log)

	score := &matching.MatchScore{
		Total:      0.91, // Below threshold but in AUTO band
		Confidence: matching.ConfidenceAuto,
	}

	result := handler.DetermineAction(score)

	// Should fall back to suggest when below min score
	if result.Status != entity.MatchStatusPending {
		t.Errorf("Status = %s, want %s (below min score)", result.Status, entity.MatchStatusPending)
	}
}

// =============================================================================
// Mock Notifier for Testing
// =============================================================================

type mockMatchNotifier struct {
	newMatchCalls       int
	suggestedMatchCalls int
	lastMatch           *entity.Match
}

func (m *mockMatchNotifier) NotifyNewMatch(ctx context.Context, match *entity.Match, offer *entity.Offer, request *entity.Request) error {
	m.newMatchCalls++
	m.lastMatch = match
	return nil
}

func (m *mockMatchNotifier) NotifySuggestedMatch(ctx context.Context, match *entity.Match, offer *entity.Offer, request *entity.Request) error {
	m.suggestedMatchCalls++
	m.lastMatch = match
	return nil
}

// =============================================================================
// ProcessMatchAction Tests
// =============================================================================

func TestAutoActionHandler_ProcessMatchAction_AutoConfirm(t *testing.T) {
	log := zerolog.Nop()
	notifier := &mockMatchNotifier{}
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), notifier, log)

	match := &entity.Match{ID: "match-1"}
	offer := &entity.Offer{ID: "offer-1"}
	request := &entity.Request{ID: "request-1"}

	result := MatchActionResult{
		Action:           ActionAutoConfirm,
		Status:           entity.MatchStatusConfirmed,
		ShouldSave:       true,
		ShouldNotify:     true,
		NotificationType: "auto_confirmed",
	}

	handler.ProcessMatchAction(context.Background(), match, offer, request, result)

	if match.Status != entity.MatchStatusConfirmed {
		t.Errorf("Status = %s, want %s", match.Status, entity.MatchStatusConfirmed)
	}
	if match.ConfirmedAt == nil {
		t.Error("ConfirmedAt should be set for confirmed matches")
	}
	if match.MatchedBy != "AUTO" {
		t.Errorf("MatchedBy = %s, want AUTO", match.MatchedBy)
	}
	if notifier.newMatchCalls != 1 {
		t.Errorf("newMatchCalls = %d, want 1", notifier.newMatchCalls)
	}
}

func TestAutoActionHandler_ProcessMatchAction_Suggest(t *testing.T) {
	log := zerolog.Nop()
	notifier := &mockMatchNotifier{}
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), notifier, log)

	match := &entity.Match{ID: "match-1"}
	offer := &entity.Offer{ID: "offer-1"}
	request := &entity.Request{ID: "request-1"}

	result := MatchActionResult{
		Action:           ActionSuggest,
		Status:           entity.MatchStatusPending,
		ShouldSave:       true,
		ShouldNotify:     true,
		NotificationType: "suggested_match",
	}

	handler.ProcessMatchAction(context.Background(), match, offer, request, result)

	if match.Status != entity.MatchStatusPending {
		t.Errorf("Status = %s, want %s", match.Status, entity.MatchStatusPending)
	}
	if notifier.suggestedMatchCalls != 1 {
		t.Errorf("suggestedMatchCalls = %d, want 1", notifier.suggestedMatchCalls)
	}
}

// =============================================================================
// Configuration Methods Tests
// =============================================================================

func TestAutoActionHandler_EnableAutoConfirm(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	handler.EnableAutoConfirm(false)
	if handler.config.AutoConfirmEnabled {
		t.Error("AutoConfirmEnabled should be false")
	}

	handler.EnableAutoConfirm(true)
	if !handler.config.AutoConfirmEnabled {
		t.Error("AutoConfirmEnabled should be true")
	}
}

func TestAutoActionHandler_SetMinScoreForAutoConfirm(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	handler.SetMinScoreForAutoConfirm(0.85)
	if handler.config.MinScoreForAutoConfirm != 0.85 {
		t.Errorf("MinScoreForAutoConfirm = %f, want 0.85", handler.config.MinScoreForAutoConfirm)
	}

	// Invalid values should be ignored
	handler.SetMinScoreForAutoConfirm(0)
	if handler.config.MinScoreForAutoConfirm != 0.85 {
		t.Error("Invalid value should be ignored")
	}

	handler.SetMinScoreForAutoConfirm(1.5)
	if handler.config.MinScoreForAutoConfirm != 0.85 {
		t.Error("Value > 1.0 should be ignored")
	}
}

func TestAutoActionHandler_EnableNotifications(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	handler.EnableNotifications(false)
	if handler.config.EnableNotifications {
		t.Error("EnableNotifications should be false")
	}

	handler.EnableNotifications(true)
	if !handler.config.EnableNotifications {
		t.Error("EnableNotifications should be true")
	}
}

// =============================================================================
// Stats Tracking Tests
// =============================================================================

func TestAutoActionHandler_StatsTracking(t *testing.T) {
	log := zerolog.Nop()
	handler := NewAutoActionHandler(DefaultAutoActionConfig(), nil, log)

	// Process various confidence levels
	handler.DetermineAction(&matching.MatchScore{Total: 0.95, Confidence: matching.ConfidenceAuto})
	handler.DetermineAction(&matching.MatchScore{Total: 0.8, Confidence: matching.ConfidenceSuggest})
	handler.DetermineAction(&matching.MatchScore{Total: 0.6, Confidence: matching.ConfidenceReview})
	handler.DetermineAction(&matching.MatchScore{Total: 0.3, Confidence: matching.ConfidenceNone})

	stats := handler.GetStats()

	if stats["total_matches"] != 4 {
		t.Errorf("total_matches = %d, want 4", stats["total_matches"])
	}
	if stats["auto_confirmed"] != 1 {
		t.Errorf("auto_confirmed = %d, want 1", stats["auto_confirmed"])
	}
	if stats["suggested"] != 1 {
		t.Errorf("suggested = %d, want 1", stats["suggested"])
	}
	if stats["queued_for_review"] != 1 {
		t.Errorf("queued_for_review = %d, want 1", stats["queued_for_review"])
	}
	if stats["ignored"] != 1 {
		t.Errorf("ignored = %d, want 1", stats["ignored"])
	}
}

// =============================================================================
// Auto-Action Constants Tests
// =============================================================================

func TestAutoActionConstants(t *testing.T) {
	if DefaultAutoConfirmThreshold <= 0 || DefaultAutoConfirmThreshold > 1 {
		t.Error("DefaultAutoConfirmThreshold should be between 0 and 1")
	}
	if DefaultAutoConfirmThreshold < 0.8 {
		t.Error("DefaultAutoConfirmThreshold should be at least 0.8 for safety")
	}
}
