// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"github.com/google/uuid"

	"pharmabroker/domain/entity"
)

// AuditRepo implements audit logging storage
type AuditRepo struct {
	db *DB
}

// NewAuditRepo creates a new audit repository
func NewAuditRepo(db *DB) *AuditRepo {
	return &AuditRepo{db: db}
}

// Log records an audit entry
func (r *AuditRepo) Log(ctx context.Context, action entity.AuditAction, entityID, details string) error {
	model := &AuditLog{
		ID:        uuid.New().String(),
		Action:    string(action),
		EntityID:  nilIfEmpty(entityID),
		Details:   nilIfEmpty(details),
		CreatedAt: time.Now(),
	}
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// LogWithValues records an audit entry with old/new values
func (r *AuditRepo) LogWithValues(ctx context.Context, action entity.AuditAction, entityID, oldVal, newVal, details string) error {
	model := &AuditLog{
		ID:        uuid.New().String(),
		Action:    string(action),
		EntityID:  nilIfEmpty(entityID),
		OldValue:  nilIfEmpty(oldVal),
		NewValue:  nilIfEmpty(newVal),
		Details:   nilIfEmpty(details),
		CreatedAt: time.Now(),
	}
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// Save stores an audit log entry
func (r *AuditRepo) Save(ctx context.Context, entry *entity.AuditLog) error {
	model := ToAuditLogModel(entry)
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// GetRecent returns recent audit logs
func (r *AuditRepo) GetRecent(ctx context.Context, limit int) ([]*entity.AuditLog, error) {
	var models []AuditLog
	err := r.db.Conn.WithContext(ctx).
		Order("created_at DESC").
		Limit(limit).
		Find(&models).Error
	if err != nil {
		return nil, err
	}
	return ToAuditLogsEntity(models), nil
}

// GetByAction returns audit logs filtered by action type
func (r *AuditRepo) GetByAction(ctx context.Context, action entity.AuditAction, limit int) ([]*entity.AuditLog, error) {
	var models []AuditLog
	err := r.db.Conn.WithContext(ctx).
		Where("action = ?", string(action)).
		Order("created_at DESC").
		Limit(limit).
		Find(&models).Error
	if err != nil {
		return nil, err
	}
	return ToAuditLogsEntity(models), nil
}

// GetByEntity returns audit logs for a specific entity
func (r *AuditRepo) GetByEntity(ctx context.Context, entityID string, limit int) ([]*entity.AuditLog, error) {
	var models []AuditLog
	err := r.db.Conn.WithContext(ctx).
		Where("entity_id = ?", entityID).
		Order("created_at DESC").
		Limit(limit).
		Find(&models).Error
	if err != nil {
		return nil, err
	}
	return ToAuditLogsEntity(models), nil
}

// Count returns total audit log count
func (r *AuditRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&AuditLog{}).
		Count(&count).Error
	return int(count), err
}
