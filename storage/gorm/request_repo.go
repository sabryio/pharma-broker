// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that RequestRepo implements the interface
var _ repository.RequestRepository = (*RequestRepo)(nil)

// RequestRepo implements repository.RequestRepository using GORM
type RequestRepo struct {
	db *DB
}

// NewRequestRepo creates a new GORM-based request repository
func NewRequestRepo(db *DB) *RequestRepo {
	return &RequestRepo{db: db}
}

// Save creates or updates a request (upsert)
func (r *RequestRepo) Save(ctx context.Context, req *entity.Request) error {
	model := ToRequestModel(req)

	return r.db.Conn.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"medication", "quantity", "max_price", "status", "updated_at",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves a request by its ID
func (r *RequestRepo) GetByID(ctx context.Context, id string) (*entity.Request, error) {
	var model Request
	err := r.db.Conn.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToRequestEntity(&model), nil
}

// GetActive retrieves active requests with pagination (urgent first)
func (r *RequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Request, error) {
	var requests []Request
	err := r.db.Conn.WithContext(ctx).
		Where("status = ?", "ACTIVE").
		Order("urgent DESC, created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&requests).Error
	if err != nil {
		return nil, err
	}
	return ToRequestsEntity(requests), nil
}

// Search performs FTS search on requests
func (r *RequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Request, error) {
	var requests []Request

	// Use medication-optimized search query (Arabic normalization + OR-based)
	sanitizedQuery := BuildMedicationSearchQuery(query)
	err := r.db.Conn.WithContext(ctx).
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
	return ToRequestsEntity(requests), nil
}

// UpdateStatus updates the status of a request
func (r *RequestRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return r.db.Conn.WithContext(ctx).
		Model(&Request{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":     string(status),
			"updated_at": time.Now(),
		}).Error
}

// CountActive returns the count of active requests
func (r *RequestRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&Request{}).
		Where("status = ?", "ACTIVE").
		Count(&count).Error
	return count, err
}
