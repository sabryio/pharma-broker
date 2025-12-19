package parsing

import (
	"context"
	"testing"
	"time"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

// Helper function
func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// =============================================================================
// AuditEntry Tests
// =============================================================================

func TestAuditEntry_ToJSON(t *testing.T) {
	entry := &AuditEntry{
		ID:        "test-123",
		Timestamp: time.Date(2024, 12, 17, 10, 0, 0, 0, time.UTC),
		EventType: AuditEventMatchAutoConfirm,
		MatchID:   "match-1",
		Action:    ActionAutoConfirm,
		Score:     0.95,
		Actor:     "AUTO",
	}

	// Helper function
	var contains = func(s, substr string) bool {
		return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
	}

	json := entry.ToJSON()
	if json == "" {
		t.Error("ToJSON should return non-empty string")
	}
	if !contains(json, "MATCH_AUTO_CONFIRMED") {
		t.Error("JSON should contain event type")
	}
	if !contains(json, "match-1") {
		t.Error("JSON should contain match ID")
	}
}

// =============================================================================
// MemoryAuditLogger Tests
// =============================================================================

func TestNewMemoryAuditLogger(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)

	if logger.maxSize != 100 {
		t.Errorf("maxSize = %d, want 100", logger.maxSize)
	}
	if logger.Count() != 0 {
		t.Errorf("Count() = %d, want 0", logger.Count())
	}
}

func TestNewMemoryAuditLogger_DefaultSize(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(0, log)

	if logger.maxSize != 10000 {
		t.Errorf("maxSize = %d, want 10000 (default)", logger.maxSize)
	}
}

func TestMemoryAuditLogger_Log(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	entry := &AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		EventType: AuditEventMatchCreated,
		MatchID:   "match-1",
	}

	err := logger.Log(ctx, entry)
	if err != nil {
		t.Errorf("Log() error = %v", err)
	}

	if logger.Count() != 1 {
		t.Errorf("Count() = %d, want 1", logger.Count())
	}
}

func TestMemoryAuditLogger_Log_CircularBuffer(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(5, log)
	ctx := context.Background()

	// Log 7 entries (exceeds buffer size of 5)
	for i := range 7 {
		entry := &AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now(),
			EventType: AuditEventMatchCreated,
		}
		_ = logger.Log(ctx, entry)
	}

	// Should only have 5 entries (circular buffer)
	if logger.Count() != 5 {
		t.Errorf("Count() = %d, want 5", logger.Count())
	}
}

func TestMemoryAuditLogger_Query_ByMatchID(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	// Log entries for different matches
	_ = logger.Log(ctx, &AuditEntry{ID: "1", MatchID: "match-A", EventType: AuditEventMatchCreated})
	_ = logger.Log(ctx, &AuditEntry{ID: "2", MatchID: "match-B", EventType: AuditEventMatchCreated})
	_ = logger.Log(ctx, &AuditEntry{ID: "3", MatchID: "match-A", EventType: AuditEventMatchAutoConfirm})

	results, err := logger.Query(ctx, AuditFilter{MatchID: "match-A"})
	if err != nil {
		t.Errorf("Query() error = %v", err)
	}
	if len(results) != 2 {
		t.Errorf("Query() returned %d results, want 2", len(results))
	}
}

func TestMemoryAuditLogger_Query_ByEventType(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	_ = logger.Log(ctx, &AuditEntry{ID: "1", EventType: AuditEventMatchCreated})
	_ = logger.Log(ctx, &AuditEntry{ID: "2", EventType: AuditEventMatchAutoConfirm})
	_ = logger.Log(ctx, &AuditEntry{ID: "3", EventType: AuditEventMatchAutoConfirm})

	results, err := logger.Query(ctx, AuditFilter{EventType: AuditEventMatchAutoConfirm})
	if err != nil {
		t.Errorf("Query() error = %v", err)
	}
	if len(results) != 2 {
		t.Errorf("Query() returned %d results, want 2", len(results))
	}
}

func TestMemoryAuditLogger_Query_WithLimit(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	for i := range 10 {
		_ = logger.Log(ctx, &AuditEntry{ID: string(rune('A' + i)), EventType: AuditEventMatchCreated})
	}

	results, err := logger.Query(ctx, AuditFilter{Limit: 3})
	if err != nil {
		t.Errorf("Query() error = %v", err)
	}
	if len(results) != 3 {
		t.Errorf("Query() returned %d results, want 3", len(results))
	}
}

func TestMemoryAuditLogger_Query_ByTimeRange(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	now := time.Now()
	_ = logger.Log(ctx, &AuditEntry{ID: "1", Timestamp: now.Add(-2 * time.Hour), EventType: AuditEventMatchCreated})
	_ = logger.Log(ctx, &AuditEntry{ID: "2", Timestamp: now.Add(-1 * time.Hour), EventType: AuditEventMatchCreated})
	_ = logger.Log(ctx, &AuditEntry{ID: "3", Timestamp: now, EventType: AuditEventMatchCreated})

	results, err := logger.Query(ctx, AuditFilter{
		StartTime: now.Add(-90 * time.Minute),
		EndTime:   now.Add(time.Minute),
	})
	if err != nil {
		t.Errorf("Query() error = %v", err)
	}
	if len(results) != 2 {
		t.Errorf("Query() returned %d results, want 2", len(results))
	}
}

func TestMemoryAuditLogger_GetAll(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	ctx := context.Background()

	_ = logger.Log(ctx, &AuditEntry{ID: "1"})
	_ = logger.Log(ctx, &AuditEntry{ID: "2"})

	all := logger.GetAll()
	if len(all) != 2 {
		t.Errorf("GetAll() returned %d entries, want 2", len(all))
	}
}

// =============================================================================
// AuditTrail Tests
// =============================================================================

func TestDefaultAuditTrailConfig(t *testing.T) {
	cfg := DefaultAuditTrailConfig()

	if !cfg.Enabled {
		t.Error("Enabled should be true by default")
	}
	if cfg.RetentionDays != 90 {
		t.Errorf("RetentionDays = %d, want 90", cfg.RetentionDays)
	}
}

func TestNewAuditTrail(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)

	if at == nil {
		t.Error("NewAuditTrail should not return nil")
	}
}

func TestAuditTrail_LogMatchAction(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)
	ctx := context.Background()

	match := &entity.Match{
		ID:        "match-1",
		OfferID:   "offer-1",
		RequestID: "request-1",
		Score:     0.95,
		Status:    entity.MatchStatusConfirmed,
		Reasoning: "High confidence match",
	}
	offer := &entity.Offer{ID: "offer-1", Medication: "Aspirin"}
	request := &entity.Request{ID: "request-1", Medication: "Aspirin 100mg"}

	err := at.LogMatchAction(ctx, match, offer, request, ActionAutoConfirm, "Score >= 0.9")
	if err != nil {
		t.Errorf("LogMatchAction() error = %v", err)
	}

	if logger.Count() != 1 {
		t.Errorf("Logger count = %d, want 1", logger.Count())
	}

	entries := logger.GetAll()
	if entries[0].EventType != AuditEventMatchAutoConfirm {
		t.Errorf("EventType = %s, want MATCH_AUTO_CONFIRMED", entries[0].EventType)
	}
	if entries[0].Actor != "AUTO" {
		t.Errorf("Actor = %s, want AUTO", entries[0].Actor)
	}
}

func TestAuditTrail_LogMatchAction_Disabled(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	cfg := DefaultAuditTrailConfig()
	cfg.Enabled = false
	at := NewAuditTrail(cfg, logger, log)
	ctx := context.Background()

	match := &entity.Match{ID: "match-1"}
	offer := &entity.Offer{ID: "offer-1"}
	request := &entity.Request{ID: "request-1"}

	_ = at.LogMatchAction(ctx, match, offer, request, ActionAutoConfirm, "test")

	if logger.Count() != 0 {
		t.Errorf("Logger count = %d, want 0 (disabled)", logger.Count())
	}
}

func TestAuditTrail_LogConfigChange(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)
	ctx := context.Background()

	err := at.LogConfigChange(ctx, "auto_confirm_threshold", 0.9, 0.85, "admin")
	if err != nil {
		t.Errorf("LogConfigChange() error = %v", err)
	}

	entries := logger.GetAll()
	if len(entries) != 1 {
		t.Fatalf("Expected 1 entry, got %d", len(entries))
	}
	if entries[0].EventType != AuditEventConfigChanged {
		t.Errorf("EventType = %s, want CONFIG_CHANGED", entries[0].EventType)
	}
	if entries[0].Actor != "admin" {
		t.Errorf("Actor = %s, want admin", entries[0].Actor)
	}
}

func TestAuditTrail_LogCalibrationReset(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)
	ctx := context.Background()

	err := at.LogCalibrationReset(ctx, "admin", "Manual reset for testing")
	if err != nil {
		t.Errorf("LogCalibrationReset() error = %v", err)
	}

	entries := logger.GetAll()
	if entries[0].EventType != AuditEventCalibrationReset {
		t.Errorf("EventType = %s, want CALIBRATION_RESET", entries[0].EventType)
	}
}

func TestAuditTrail_GetMatchHistory(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)
	ctx := context.Background()

	// Log multiple actions for same match
	match := &entity.Match{ID: "match-1", OfferID: "o1", RequestID: "r1", Score: 0.8}
	offer := &entity.Offer{ID: "o1"}
	request := &entity.Request{ID: "r1"}

	_ = at.LogMatchAction(ctx, match, offer, request, ActionSuggest, "Initial")
	match.Status = entity.MatchStatusConfirmed
	_ = at.LogMatchAction(ctx, match, offer, request, ActionAutoConfirm, "Confirmed")

	history, err := at.GetMatchHistory(ctx, "match-1")
	if err != nil {
		t.Errorf("GetMatchHistory() error = %v", err)
	}
	if len(history) != 2 {
		t.Errorf("GetMatchHistory() returned %d entries, want 2", len(history))
	}
}

func TestAuditTrail_GetRecentActions(t *testing.T) {
	log := zerolog.Nop()
	logger := NewMemoryAuditLogger(100, log)
	at := NewAuditTrail(DefaultAuditTrailConfig(), logger, log)
	ctx := context.Background()

	for i := range 5 {
		match := &entity.Match{ID: string(rune('A' + i)), OfferID: "o", RequestID: "r"}
		_ = at.LogMatchAction(ctx, match, &entity.Offer{}, &entity.Request{}, ActionReview, "test")
	}

	recent, err := at.GetRecentActions(ctx, 3)
	if err != nil {
		t.Errorf("GetRecentActions() error = %v", err)
	}
	if len(recent) != 3 {
		t.Errorf("GetRecentActions() returned %d entries, want 3", len(recent))
	}
}

func TestAuditTrail_Enable(t *testing.T) {
	log := zerolog.Nop()
	at := NewAuditTrail(DefaultAuditTrailConfig(), nil, log)

	at.Enable(false)
	if at.GetConfig().Enabled {
		t.Error("Enabled should be false after Enable(false)")
	}

	at.Enable(true)
	if !at.GetConfig().Enabled {
		t.Error("Enabled should be true after Enable(true)")
	}
}

func TestAuditTrail_SetConfig(t *testing.T) {
	log := zerolog.Nop()
	at := NewAuditTrail(DefaultAuditTrailConfig(), nil, log)

	newCfg := AuditTrailConfig{
		Enabled:       false,
		RetentionDays: 30,
	}
	at.SetConfig(newCfg)

	cfg := at.GetConfig()
	if cfg.Enabled {
		t.Error("Enabled should be false")
	}
	if cfg.RetentionDays != 30 {
		t.Errorf("RetentionDays = %d, want 30", cfg.RetentionDays)
	}
}

func TestAuditTrail_actionToEventType(t *testing.T) {
	log := zerolog.Nop()
	at := NewAuditTrail(DefaultAuditTrailConfig(), nil, log)

	tests := []struct {
		action   ActionType
		expected AuditEventType
	}{
		{ActionAutoConfirm, AuditEventMatchAutoConfirm},
		{ActionSuggest, AuditEventMatchSuggested},
		{ActionReview, AuditEventMatchReviewed},
		{ActionIgnore, AuditEventMatchIgnored},
		{"UNKNOWN", AuditEventMatchCreated},
	}

	for _, tt := range tests {
		result := at.actionToEventType(tt.action)
		if result != tt.expected {
			t.Errorf("actionToEventType(%s) = %s, want %s", tt.action, result, tt.expected)
		}
	}
}
