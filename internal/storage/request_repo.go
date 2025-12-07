package storage

import (
	"context"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormRequestRepo implements domain.RequestRepository using GORM
type GormRequestRepo struct {
	db *GormDB
}

// NewGormRequestRepo creates a new GORM-based request repository
func NewGormRequestRepo(db *GormDB) *GormRequestRepo {
	return &GormRequestRepo{db: db}
}

// Save creates or updates a request (upsert)
func (r *GormRequestRepo) Save(ctx context.Context, req *domain.Request) error {
	model := ToRequestModel(req)

	// GORM upsert with conflict handling
	return r.db.DB.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"medication", "quantity", "max_price", "status", "updated_at",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves a request by its ID
func (r *GormRequestRepo) GetByID(ctx context.Context, id string) (*domain.Request, error) {
	var model models.Request
	err := r.db.DB.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil // Return nil, nil for not found (consistent with original)
		}
		return nil, err
	}
	return ToRequestDomain(&model), nil
}

// GetActive retrieves active requests with pagination (urgent first)
func (r *GormRequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Request, error) {
	var requests []models.Request
	err := r.db.DB.WithContext(ctx).
		Where("status = ?", "ACTIVE").
		Order("urgent DESC, created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&requests).Error
	if err != nil {
		return nil, err
	}
	return ToRequestsDomain(requests), nil
}

// Search performs FTS search on requests (raw SQL for FTS5)
func (r *GormRequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Request, error) {
	var requests []models.Request

	// FTS queries require raw SQL - GORM doesn't support virtual tables
	sanitizedQuery := SanitizeFTSQuery(query)
	err := r.db.DB.WithContext(ctx).
		Raw(`
			SELECT r.id, r.raw_message_id, r.source_phone, r.source_name, r.source_group, r.group_name,
				r.medication, r.medication_raw, r.quantity, r.unit, r.max_price, r.currency, r.urgent,
				r.notes, r.raw_message, r.status, r.created_at, r.updated_at
			FROM requests r
			JOIN requests_fts f ON r.rowid = f.rowid
			WHERE requests_fts MATCH ? AND r.status = 'ACTIVE'
			ORDER BY r.urgent DESC, rank
			LIMIT ? OFFSET ?
		`, sanitizedQuery, limit, offset).
		Scan(&requests).Error

	if err != nil {
		return nil, err
	}
	return ToRequestsDomain(requests), nil
}

// UpdateStatus updates the status of a request
func (r *GormRequestRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	return r.db.DB.WithContext(ctx).
		Model(&models.Request{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":     string(status),
			"updated_at": time.Now(),
		}).Error
}

// CountActive returns the count of active requests
func (r *GormRequestRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.Request{}).
		Where("status = ?", "ACTIVE").
		Count(&count).Error
	return count, err
}
