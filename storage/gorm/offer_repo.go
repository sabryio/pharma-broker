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

// Compile-time check that OfferRepo implements the interface
var _ repository.OfferRepository = (*OfferRepo)(nil)

// OfferRepo implements repository.OfferRepository using GORM
type OfferRepo struct {
	db *DB
}

// NewOfferRepo creates a new GORM-based offer repository
func NewOfferRepo(db *DB) *OfferRepo {
	return &OfferRepo{db: db}
}

// Save creates or updates an offer (upsert)
func (r *OfferRepo) Save(ctx context.Context, offer *entity.Offer) error {
	model := ToOfferModel(offer)

	return r.db.Conn.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"medication", "quantity", "price", "status", "updated_at",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves an offer by its ID
func (r *OfferRepo) GetByID(ctx context.Context, id string) (*entity.Offer, error) {
	var model Offer
	err := r.db.Conn.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToOfferEntity(&model), nil
}

// GetActive retrieves active offers with pagination
func (r *OfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*entity.Offer, error) {
	var offers []Offer
	err := r.db.Conn.WithContext(ctx).
		Where("status = ?", "ACTIVE").
		Order("created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&offers).Error
	if err != nil {
		return nil, err
	}
	return ToOffersEntity(offers), nil
}

// Search performs full-text search on offers using PostgreSQL tsvector
func (r *OfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*entity.Offer, error) {
	var offers []Offer

	// Use medication-optimized search query (Arabic normalization + OR-based)
	tsQuery := BuildMedicationSearchQuery(query)
	if tsQuery == "" {
		// Empty query, return empty results
		return []*entity.Offer{}, nil
	}

	err := r.db.Conn.WithContext(ctx).
		Raw(`
			SELECT id, raw_message_id, source_phone, source_name, source_group, group_name,
				medication, medication_raw, quantity, unit, price, currency, expiry_date, batch_number,
				notes, raw_message, status, created_at, updated_at,
				ts_rank(search_vector, to_tsquery('simple', ?)) as rank
			FROM offers
			WHERE search_vector @@ to_tsquery('simple', ?) AND status = 'ACTIVE'
			ORDER BY rank DESC
			LIMIT ? OFFSET ?
		`, tsQuery, tsQuery, limit, offset).
		Scan(&offers).Error

	if err != nil {
		return nil, err
	}
	return ToOffersEntity(offers), nil
}

// UpdateStatus updates the status of an offer
func (r *OfferRepo) UpdateStatus(ctx context.Context, id string, status entity.ItemStatus) error {
	return r.db.Conn.WithContext(ctx).
		Model(&Offer{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":     string(status),
			"updated_at": time.Now(),
		}).Error
}

// CountActive returns the count of active offers
func (r *OfferRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&Offer{}).
		Where("status = ?", "ACTIVE").
		Count(&count).Error
	return count, err
}
