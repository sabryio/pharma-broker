package storage

import (
	"context"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormOfferRepo implements domain.OfferRepository using GORM
type GormOfferRepo struct {
	db *GormDB
}

// NewGormOfferRepo creates a new GORM-based offer repository
func NewGormOfferRepo(db *GormDB) *GormOfferRepo {
	return &GormOfferRepo{db: db}
}

// Save creates or updates an offer (upsert)
func (r *GormOfferRepo) Save(ctx context.Context, offer *domain.Offer) error {
	model := ToOfferModel(offer)

	// GORM upsert with conflict handling
	return r.db.DB.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"medication", "quantity", "price", "status", "updated_at",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves an offer by its ID
func (r *GormOfferRepo) GetByID(ctx context.Context, id string) (*domain.Offer, error) {
	var model models.Offer
	err := r.db.DB.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil // Return nil, nil for not found (consistent with original)
		}
		return nil, err
	}
	return ToOfferDomain(&model), nil
}

// GetActive retrieves active offers with pagination
func (r *GormOfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Offer, error) {
	var offers []models.Offer
	err := r.db.DB.WithContext(ctx).
		Where("status = ?", "ACTIVE").
		Order("created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&offers).Error
	if err != nil {
		return nil, err
	}
	return ToOffersDomain(offers), nil
}

// Search performs FTS search on offers (raw SQL for FTS5)
func (r *GormOfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Offer, error) {
	var offers []models.Offer

	// FTS queries require raw SQL - GORM doesn't support virtual tables
	sanitizedQuery := SanitizeFTSQuery(query)
	err := r.db.DB.WithContext(ctx).
		Raw(`
			SELECT o.id, o.raw_message_id, o.source_phone, o.source_name, o.source_group, o.group_name,
				o.medication, o.medication_raw, o.quantity, o.unit, o.price, o.currency, o.expiry_date, o.batch_number,
				o.notes, o.raw_message, o.status, o.created_at, o.updated_at
			FROM offers o
			JOIN offers_fts f ON o.rowid = f.rowid
			WHERE offers_fts MATCH ? AND o.status = 'ACTIVE'
			ORDER BY rank
			LIMIT ? OFFSET ?
		`, sanitizedQuery, limit, offset).
		Scan(&offers).Error

	if err != nil {
		return nil, err
	}
	return ToOffersDomain(offers), nil
}

// UpdateStatus updates the status of an offer
func (r *GormOfferRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	return r.db.DB.WithContext(ctx).
		Model(&models.Offer{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":     string(status),
			"updated_at": time.Now(),
		}).Error
}

// CountActive returns the count of active offers
func (r *GormOfferRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.Offer{}).
		Where("status = ?", "ACTIVE").
		Count(&count).Error
	return count, err
}
