package storage

import (
	"context"
	"time"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormMatchRepo implements domain.MatchRepository using GORM
type GormMatchRepo struct {
	db *GormDB
}

// NewGormMatchRepo creates a new GORM-based match repository
func NewGormMatchRepo(db *GormDB) *GormMatchRepo {
	return &GormMatchRepo{db: db}
}

// Save creates or updates a match (upsert on offer_id + request_id)
func (r *GormMatchRepo) Save(ctx context.Context, match *domain.Match) error {
	model := ToMatchModel(match)

	return r.db.DB.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "offer_id"}, {Name: "request_id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"score", "reasoning",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves a match by its ID
func (r *GormMatchRepo) GetByID(ctx context.Context, id string) (*domain.Match, error) {
	var model models.Match
	err := r.db.DB.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToMatchDomain(&model), nil
}

// GetPending retrieves pending matches with full offer and request details (using Preload)
func (r *GormMatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.MatchWithDetails, error) {
	var matches []models.Match
	err := r.db.DB.WithContext(ctx).
		Preload("Offer").
		Preload("Request").
		Where("status = ?", "PENDING").
		Order("score DESC, created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}
	return ToMatchesWithDetailsDomain(matches), nil
}

// GetByOfferID retrieves all matches for a given offer
func (r *GormMatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*domain.Match, error) {
	var matches []models.Match
	err := r.db.DB.WithContext(ctx).
		Where("offer_id = ?", offerID).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}
	return ToMatchesDomain(matches), nil
}

// GetByRequestID retrieves all matches for a given request
func (r *GormMatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*domain.Match, error) {
	var matches []models.Match
	err := r.db.DB.WithContext(ctx).
		Where("request_id = ?", requestID).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}
	return ToMatchesDomain(matches), nil
}

// UpdateStatus updates the status of a match
func (r *GormMatchRepo) UpdateStatus(ctx context.Context, id string, status domain.MatchStatus, matchedBy string) error {
	updates := map[string]interface{}{
		"status":     string(status),
		"matched_by": matchedBy,
	}

	if status == domain.MatchStatusConfirmed {
		updates["confirmed_at"] = time.Now()
	}

	return r.db.DB.WithContext(ctx).
		Model(&models.Match{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// CountPending returns the count of pending matches
func (r *GormMatchRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.Match{}).
		Where("status = ?", "PENDING").
		Count(&count).Error
	return count, err
}

// CountConfirmedToday returns the count of matches confirmed today
func (r *GormMatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.Match{}).
		Where("status = ? AND date(confirmed_at) = date('now')", "CONFIRMED").
		Count(&count).Error
	return count, err
}
