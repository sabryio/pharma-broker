package storage

import (
	"context"
	"time"

	"github.com/google/uuid"

	"pharmabroker/internal/storage/models"
)

// GormAuditRepo implements audit logging storage using GORM
type GormAuditRepo struct {
	db *GormDB
}

// NewGormAuditRepo creates a new GORM-based AuditRepo
func NewGormAuditRepo(db *GormDB) *GormAuditRepo {
	return &GormAuditRepo{db: db}
}

// Log records an audit entry
func (r *GormAuditRepo) Log(ctx context.Context, action AuditAction, entityID, details string) error {
	entry := &models.AuditLog{
		ID:        uuid.New().String(),
		Action:    string(action),
		EntityID:  nilIfEmpty(entityID),
		Details:   nilIfEmpty(details),
		CreatedAt: time.Now(),
	}
	return r.db.DB.WithContext(ctx).Create(entry).Error
}

// LogWithValues records an audit entry with old/new values
func (r *GormAuditRepo) LogWithValues(ctx context.Context, action AuditAction, entityID, oldVal, newVal, details string) error {
	entry := &models.AuditLog{
		ID:        uuid.New().String(),
		Action:    string(action),
		EntityID:  nilIfEmpty(entityID),
		OldValue:  nilIfEmpty(oldVal),
		NewValue:  nilIfEmpty(newVal),
		Details:   nilIfEmpty(details),
		CreatedAt: time.Now(),
	}
	return r.db.DB.WithContext(ctx).Create(entry).Error
}

// Save stores an audit log entry
func (r *GormAuditRepo) Save(ctx context.Context, entry *AuditLog) error {
	model := &models.AuditLog{
		ID:        entry.ID,
		Action:    string(entry.Action),
		EntityID:  nilIfEmpty(entry.EntityID),
		OldValue:  nilIfEmpty(entry.OldValue),
		NewValue:  nilIfEmpty(entry.NewValue),
		Details:   nilIfEmpty(entry.Details),
		IPAddress: nilIfEmpty(entry.IPAddress),
		CreatedAt: entry.CreatedAt,
	}
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// GetRecent returns recent audit logs
func (r *GormAuditRepo) GetRecent(ctx context.Context, limit int) ([]*AuditLog, error) {
	var logs []models.AuditLog
	err := r.db.DB.WithContext(ctx).
		Order("created_at DESC").
		Limit(limit).
		Find(&logs).Error
	if err != nil {
		return nil, err
	}
	return toAuditLogsDomain(logs), nil
}

// GetByAction returns audit logs filtered by action type
func (r *GormAuditRepo) GetByAction(ctx context.Context, action AuditAction, limit int) ([]*AuditLog, error) {
	var logs []models.AuditLog
	err := r.db.DB.WithContext(ctx).
		Where("action = ?", string(action)).
		Order("created_at DESC").
		Limit(limit).
		Find(&logs).Error
	if err != nil {
		return nil, err
	}
	return toAuditLogsDomain(logs), nil
}

// GetByEntity returns audit logs for a specific entity
func (r *GormAuditRepo) GetByEntity(ctx context.Context, entityID string, limit int) ([]*AuditLog, error) {
	var logs []models.AuditLog
	err := r.db.DB.WithContext(ctx).
		Where("entity_id = ?", entityID).
		Order("created_at DESC").
		Limit(limit).
		Find(&logs).Error
	if err != nil {
		return nil, err
	}
	return toAuditLogsDomain(logs), nil
}

// Count returns total audit log count
func (r *GormAuditRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.AuditLog{}).
		Count(&count).Error
	return int(count), err
}

// toAuditLogsDomain converts GORM models to domain AuditLog slice
func toAuditLogsDomain(logs []models.AuditLog) []*AuditLog {
	result := make([]*AuditLog, len(logs))
	for i, log := range logs {
		result[i] = &AuditLog{
			ID:        log.ID,
			Action:    AuditAction(log.Action),
			EntityID:  deref(log.EntityID),
			OldValue:  deref(log.OldValue),
			NewValue:  deref(log.NewValue),
			Details:   deref(log.Details),
			IPAddress: deref(log.IPAddress),
			CreatedAt: log.CreatedAt,
		}
	}
	return result
}
