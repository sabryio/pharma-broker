package storage

import (
	"fmt"
	"time"

	"pharmabroker/internal/config"
)

// AuditAction represents the type of audited action
type AuditAction string

const (
	AuditMatchConfirmed  AuditAction = "MATCH_CONFIRMED"
	AuditMatchRejected   AuditAction = "MATCH_REJECTED"
	AuditConfigChanged   AuditAction = "CONFIG_CHANGED"
	AuditGroupEnabled    AuditAction = "GROUP_ENABLED"
	AuditGroupDisabled   AuditAction = "GROUP_DISABLED"
	AuditReportGenerated AuditAction = "REPORT_GENERATED"
)

// AuditLog represents an audit log entry
type AuditLog struct {
	ID        string      `json:"id"`
	Action    AuditAction `json:"action"`
	EntityID  string      `json:"entity_id,omitempty"`
	OldValue  string      `json:"old_value,omitempty"`
	NewValue  string      `json:"new_value,omitempty"`
	Details   string      `json:"details,omitempty"`
	IPAddress string      `json:"ip_address,omitempty"`
	CreatedAt time.Time   `json:"created_at"`
}

// AppConfig holds all application configuration
type AppConfig struct {
	AutoParseEnabled bool   `json:"auto_parse_enabled"`
	SkipOwnMessages  bool   `json:"skip_own_messages"`
	AdminPhone       string `json:"admin_phone"`
}

// DB is a wrapper for legacy compatibility (now uses GormDB internally)
// This type exists to maintain backwards compatibility with code expecting *DB
type DB = GormDB

// New creates a new database connection (alias to NewGormDB for compatibility)
func New(cfg any) (*DB, error) {
	// Type assert to *config.DatabaseConfig
	dbCfg, ok := cfg.(*config.DatabaseConfig)
	if !ok {
		return nil, fmt.Errorf("invalid config type: expected *config.DatabaseConfig, got %T", cfg)
	}
	return NewGormDB(dbCfg)
}

// ====================
// Type Aliases for API Compatibility
// ====================

// Type aliases for backwards compatibility (code expecting *storage.GroupRepo etc.)
type (
	GroupRepo             = GormGroupRepo
	OfferRepo             = GormOfferRepo
	RequestRepo           = GormRequestRepo
	MatchRepo             = GormMatchRepo
	RawMessageRepo        = GormRawMessageRepo
	MatchQueueRepo        = GormMatchQueueRepo
	MedicationMappingRepo = GormMedicationMappingRepo
	ConfigRepo            = GormConfigRepo
	FeedbackRepo          = GormFeedbackRepo
	LeaderboardRepo       = GormLeaderboardRepo
	StatsRepo             = GormStatsRepo
	AuditRepo             = GormAuditRepo
	ReportRepo            = GormReportRepo
)
