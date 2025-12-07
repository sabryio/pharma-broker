package storage

import (
	"context"
	"time"

	"github.com/google/uuid"
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
	EntityID  string      `json:"entity_id,omitempty"`  // ID of affected entity (match, config key, etc.)
	OldValue  string      `json:"old_value,omitempty"`  // Previous value (for config changes)
	NewValue  string      `json:"new_value,omitempty"`  // New value (for config changes)
	Details   string      `json:"details,omitempty"`    // Additional context
	IPAddress string      `json:"ip_address,omitempty"` // Request source
	CreatedAt time.Time   `json:"created_at"`
}

// AuditRepo implements audit logging storage
type AuditRepo struct {
	db *DB
}

// NewAuditRepo creates a new AuditRepo
func NewAuditRepo(db *DB) *AuditRepo {
	return &AuditRepo{db: db}
}

// Log records an audit entry
func (r *AuditRepo) Log(ctx context.Context, action AuditAction, entityID, details string) error {
	entry := &AuditLog{
		ID:        uuid.New().String(),
		Action:    action,
		EntityID:  entityID,
		Details:   details,
		CreatedAt: time.Now(),
	}
	return r.Save(ctx, entry)
}

// LogWithValues records an audit entry with old/new values
func (r *AuditRepo) LogWithValues(ctx context.Context, action AuditAction, entityID, oldVal, newVal, details string) error {
	entry := &AuditLog{
		ID:        uuid.New().String(),
		Action:    action,
		EntityID:  entityID,
		OldValue:  oldVal,
		NewValue:  newVal,
		Details:   details,
		CreatedAt: time.Now(),
	}
	return r.Save(ctx, entry)
}

// Save stores an audit log entry
func (r *AuditRepo) Save(ctx context.Context, entry *AuditLog) error {
	query := `
		INSERT INTO audit_logs (id, action, entity_id, old_value, new_value, details, ip_address, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
	`
	_, err := r.db.Conn().ExecContext(ctx, query,
		entry.ID,
		string(entry.Action),
		entry.EntityID,
		entry.OldValue,
		entry.NewValue,
		entry.Details,
		entry.IPAddress,
		entry.CreatedAt,
	)
	return err
}

// GetRecent returns recent audit logs
func (r *AuditRepo) GetRecent(ctx context.Context, limit int) ([]*AuditLog, error) {
	query := `
		SELECT id, action, entity_id, old_value, new_value, details, ip_address, created_at
		FROM audit_logs
		ORDER BY created_at DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*AuditLog
	for rows.Next() {
		log := &AuditLog{}
		var action string
		if err := rows.Scan(&log.ID, &action, &log.EntityID, &log.OldValue, &log.NewValue, &log.Details, &log.IPAddress, &log.CreatedAt); err != nil {
			return nil, err
		}
		log.Action = AuditAction(action)
		logs = append(logs, log)
	}
	return logs, rows.Err()
}

// GetByAction returns audit logs filtered by action type
func (r *AuditRepo) GetByAction(ctx context.Context, action AuditAction, limit int) ([]*AuditLog, error) {
	query := `
		SELECT id, action, entity_id, old_value, new_value, details, ip_address, created_at
		FROM audit_logs
		WHERE action = ?
		ORDER BY created_at DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, string(action), limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*AuditLog
	for rows.Next() {
		log := &AuditLog{}
		var actionStr string
		if err := rows.Scan(&log.ID, &actionStr, &log.EntityID, &log.OldValue, &log.NewValue, &log.Details, &log.IPAddress, &log.CreatedAt); err != nil {
			return nil, err
		}
		log.Action = AuditAction(actionStr)
		logs = append(logs, log)
	}
	return logs, rows.Err()
}

// GetByEntity returns audit logs for a specific entity
func (r *AuditRepo) GetByEntity(ctx context.Context, entityID string, limit int) ([]*AuditLog, error) {
	query := `
		SELECT id, action, entity_id, old_value, new_value, details, ip_address, created_at
		FROM audit_logs
		WHERE entity_id = ?
		ORDER BY created_at DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, entityID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var logs []*AuditLog
	for rows.Next() {
		log := &AuditLog{}
		var action string
		if err := rows.Scan(&log.ID, &action, &log.EntityID, &log.OldValue, &log.NewValue, &log.Details, &log.IPAddress, &log.CreatedAt); err != nil {
			return nil, err
		}
		log.Action = AuditAction(action)
		logs = append(logs, log)
	}
	return logs, rows.Err()
}

// Count returns total audit log count
func (r *AuditRepo) Count(ctx context.Context) (int, error) {
	var count int
	err := r.db.Reader().QueryRowContext(ctx, "SELECT COUNT(*) FROM audit_logs").Scan(&count)
	return count, err
}
