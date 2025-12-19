package parsing

import (
	"context"
	"encoding/json"
	"sync"
	"time"

	"pharmabroker/domain/entity"

	"github.com/rs/zerolog"
)

// =============================================================================
// Audit Event Types
// =============================================================================

// AuditEventType defines the type of audit event.
type AuditEventType string

const (
	AuditEventMatchCreated      AuditEventType = "MATCH_CREATED"
	AuditEventMatchAutoConfirm  AuditEventType = "MATCH_AUTO_CONFIRMED"
	AuditEventMatchSuggested    AuditEventType = "MATCH_SUGGESTED"
	AuditEventMatchReviewed     AuditEventType = "MATCH_QUEUED_REVIEW"
	AuditEventMatchIgnored      AuditEventType = "MATCH_IGNORED"
	AuditEventMatchManualAction AuditEventType = "MATCH_MANUAL_ACTION"
	AuditEventConfigChanged     AuditEventType = "CONFIG_CHANGED"
	AuditEventCalibrationReset  AuditEventType = "CALIBRATION_RESET"
)

// =============================================================================
// Audit Entry
// =============================================================================

// AuditEntry represents a single audit log entry.
type AuditEntry struct {
	ID        string                 `json:"id"`
	Timestamp time.Time              `json:"timestamp"`
	EventType AuditEventType         `json:"event_type"`
	MatchID   string                 `json:"match_id,omitempty"`
	OfferID   string                 `json:"offer_id,omitempty"`
	RequestID string                 `json:"request_id,omitempty"`
	Action    ActionType             `json:"action,omitempty"`
	Score     float64                `json:"score,omitempty"`
	Status    entity.MatchStatus     `json:"status,omitempty"`
	Reason    string                 `json:"reason,omitempty"`
	Actor     string                 `json:"actor"` // "SYSTEM", "AUTO", or user ID
	Metadata  map[string]interface{} `json:"metadata,omitempty"`
}

// ToJSON serializes the audit entry to JSON.
func (e *AuditEntry) ToJSON() string {
	data, _ := json.Marshal(e)
	return string(data)
}

// =============================================================================
// Audit Logger Interface
// =============================================================================

// AuditLogger defines the interface for audit logging backends.
type AuditLogger interface {
	Log(ctx context.Context, entry *AuditEntry) error
	Query(ctx context.Context, filter AuditFilter) ([]AuditEntry, error)
}

// AuditFilter defines filters for querying audit logs.
type AuditFilter struct {
	MatchID   string
	EventType AuditEventType
	StartTime time.Time
	EndTime   time.Time
	Limit     int
}

// =============================================================================
// In-Memory Audit Logger (for development/testing)
// =============================================================================

// MemoryAuditLogger stores audit entries in memory with a circular buffer.
type MemoryAuditLogger struct {
	entries []AuditEntry
	maxSize int
	idx     int
	mu      sync.RWMutex
	log     zerolog.Logger
}

// NewMemoryAuditLogger creates a new in-memory audit logger.
func NewMemoryAuditLogger(maxSize int, log zerolog.Logger) *MemoryAuditLogger {
	if maxSize <= 0 {
		maxSize = 10000
	}
	return &MemoryAuditLogger{
		entries: make([]AuditEntry, 0, maxSize),
		maxSize: maxSize,
		log:     log.With().Str("component", "audit-memory").Logger(),
	}
}

// Log stores an audit entry.
func (m *MemoryAuditLogger) Log(ctx context.Context, entry *AuditEntry) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if len(m.entries) < m.maxSize {
		m.entries = append(m.entries, *entry)
	} else {
		m.entries[m.idx] = *entry
		m.idx = (m.idx + 1) % m.maxSize
	}

	return nil
}

// Query retrieves audit entries matching the filter.
func (m *MemoryAuditLogger) Query(ctx context.Context, filter AuditFilter) ([]AuditEntry, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var results []AuditEntry
	limit := filter.Limit
	if limit <= 0 {
		limit = 100
	}

	for _, entry := range m.entries {
		if filter.MatchID != "" && entry.MatchID != filter.MatchID {
			continue
		}
		if filter.EventType != "" && entry.EventType != filter.EventType {
			continue
		}
		if !filter.StartTime.IsZero() && entry.Timestamp.Before(filter.StartTime) {
			continue
		}
		if !filter.EndTime.IsZero() && entry.Timestamp.After(filter.EndTime) {
			continue
		}

		results = append(results, entry)
		if len(results) >= limit {
			break
		}
	}

	return results, nil
}

// GetAll returns all audit entries.
func (m *MemoryAuditLogger) GetAll() []AuditEntry {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return append([]AuditEntry{}, m.entries...)
}

// Count returns the number of audit entries.
func (m *MemoryAuditLogger) Count() int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return len(m.entries)
}

// =============================================================================
// Audit Trail Manager
// =============================================================================

// AuditTrailConfig holds configuration for the audit trail.
type AuditTrailConfig struct {
	Enabled       bool
	LogToFile     bool
	LogToDatabase bool
	RetentionDays int
}

// DefaultAuditTrailConfig returns sensible defaults.
func DefaultAuditTrailConfig() AuditTrailConfig {
	return AuditTrailConfig{
		Enabled:       true,
		LogToFile:     true,
		LogToDatabase: false,
		RetentionDays: 90,
	}
}

// AuditTrail manages audit logging for auto-actions.
type AuditTrail struct {
	config AuditTrailConfig
	logger AuditLogger
	zlog   zerolog.Logger
	idGen  func() string
	mu     sync.RWMutex
}

// NewAuditTrail creates a new audit trail manager.
func NewAuditTrail(cfg AuditTrailConfig, logger AuditLogger, zlog zerolog.Logger) *AuditTrail {
	return &AuditTrail{
		config: cfg,
		logger: logger,
		zlog:   zlog.With().Str("component", "audit-trail").Logger(),
		idGen:  generateAuditID,
	}
}

// generateAuditID generates a unique audit entry ID.
func generateAuditID() string {
	return time.Now().Format("20060102150405.000000")
}

// LogMatchAction logs a match action to the audit trail.
func (at *AuditTrail) LogMatchAction(
	ctx context.Context,
	match *entity.Match,
	offer *entity.Offer,
	request *entity.Request,
	action ActionType,
	reason string,
) error {
	if !at.config.Enabled {
		return nil
	}

	eventType := at.actionToEventType(action)
	actor := "SYSTEM"
	if action == ActionAutoConfirm {
		actor = "AUTO"
	}

	entry := &AuditEntry{
		ID:        at.idGen(),
		Timestamp: time.Now(),
		EventType: eventType,
		MatchID:   match.ID,
		OfferID:   match.OfferID,
		RequestID: match.RequestID,
		Action:    action,
		Score:     match.Score,
		Status:    match.Status,
		Reason:    reason,
		Actor:     actor,
		Metadata: map[string]interface{}{
			"offer_medication":   offer.Medication,
			"request_medication": request.Medication,
			"breakdown":          match.Reasoning,
		},
	}

	// Log to structured logger
	at.zlog.Info().
		Str("audit_id", entry.ID).
		Str("event_type", string(entry.EventType)).
		Str("match_id", entry.MatchID).
		Str("action", string(entry.Action)).
		Float64("score", entry.Score).
		Str("status", string(entry.Status)).
		Str("actor", entry.Actor).
		Str("reason", entry.Reason).
		Msg("📝 Audit: Match action logged")

	// Log to backend
	if at.logger != nil {
		return at.logger.Log(ctx, entry)
	}

	return nil
}

// LogConfigChange logs a configuration change.
func (at *AuditTrail) LogConfigChange(ctx context.Context, configType string, oldValue, newValue interface{}, actor string) error {
	if !at.config.Enabled {
		return nil
	}

	entry := &AuditEntry{
		ID:        at.idGen(),
		Timestamp: time.Now(),
		EventType: AuditEventConfigChanged,
		Actor:     actor,
		Reason:    "Configuration changed: " + configType,
		Metadata: map[string]interface{}{
			"config_type": configType,
			"old_value":   oldValue,
			"new_value":   newValue,
		},
	}

	at.zlog.Info().
		Str("audit_id", entry.ID).
		Str("config_type", configType).
		Interface("old_value", oldValue).
		Interface("new_value", newValue).
		Str("actor", actor).
		Msg("📝 Audit: Configuration changed")

	if at.logger != nil {
		return at.logger.Log(ctx, entry)
	}

	return nil
}

// LogCalibrationReset logs a calibration reset event.
func (at *AuditTrail) LogCalibrationReset(ctx context.Context, actor string, reason string) error {
	if !at.config.Enabled {
		return nil
	}

	entry := &AuditEntry{
		ID:        at.idGen(),
		Timestamp: time.Now(),
		EventType: AuditEventCalibrationReset,
		Actor:     actor,
		Reason:    reason,
	}

	at.zlog.Info().
		Str("audit_id", entry.ID).
		Str("actor", actor).
		Str("reason", reason).
		Msg("📝 Audit: Calibration reset")

	if at.logger != nil {
		return at.logger.Log(ctx, entry)
	}

	return nil
}

// actionToEventType converts an action type to an audit event type.
func (at *AuditTrail) actionToEventType(action ActionType) AuditEventType {
	switch action {
	case ActionAutoConfirm:
		return AuditEventMatchAutoConfirm
	case ActionSuggest:
		return AuditEventMatchSuggested
	case ActionReview:
		return AuditEventMatchReviewed
	case ActionIgnore:
		return AuditEventMatchIgnored
	default:
		return AuditEventMatchCreated
	}
}

// Query retrieves audit entries matching the filter.
func (at *AuditTrail) Query(ctx context.Context, filter AuditFilter) ([]AuditEntry, error) {
	if at.logger == nil {
		return nil, nil
	}
	return at.logger.Query(ctx, filter)
}

// GetMatchHistory retrieves the audit history for a specific match.
func (at *AuditTrail) GetMatchHistory(ctx context.Context, matchID string) ([]AuditEntry, error) {
	return at.Query(ctx, AuditFilter{MatchID: matchID, Limit: 100})
}

// GetRecentActions retrieves recent audit entries.
func (at *AuditTrail) GetRecentActions(ctx context.Context, limit int) ([]AuditEntry, error) {
	return at.Query(ctx, AuditFilter{Limit: limit})
}

// GetConfig returns the current configuration.
func (at *AuditTrail) GetConfig() AuditTrailConfig {
	at.mu.RLock()
	defer at.mu.RUnlock()
	return at.config
}

// SetConfig updates the configuration.
func (at *AuditTrail) SetConfig(cfg AuditTrailConfig) {
	at.mu.Lock()
	at.config = cfg
	at.mu.Unlock()
	at.zlog.Info().
		Bool("enabled", cfg.Enabled).
		Int("retention_days", cfg.RetentionDays).
		Msg("Audit trail configuration updated")
}

// Enable enables or disables the audit trail.
func (at *AuditTrail) Enable(enabled bool) {
	at.mu.Lock()
	at.config.Enabled = enabled
	at.mu.Unlock()
	at.zlog.Info().
		Bool("enabled", enabled).
		Msg("Audit trail toggled")
}
