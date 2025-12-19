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

// Compile-time check that MatchRepo implements the interface
var _ repository.MatchRepository = (*MatchRepo)(nil)

// MatchRepo implements repository.MatchRepository using GORM
type MatchRepo struct {
	db *DB
}

// NewMatchRepo creates a new GORM-based match repository
func NewMatchRepo(db *DB) *MatchRepo {
	return &MatchRepo{db: db}
}

// Save creates or updates a match (upsert on offer_id + request_id)
func (r *MatchRepo) Save(ctx context.Context, match *entity.Match) error {
	model := ToMatchModel(match)

	return r.db.Conn.WithContext(ctx).
		Clauses(clause.OnConflict{
			Columns: []clause.Column{{Name: "offer_id"}, {Name: "request_id"}},
			DoUpdates: clause.AssignmentColumns([]string{
				"score", "reasoning",
			}),
		}).
		Create(model).Error
}

// GetByID retrieves a match by its ID
func (r *MatchRepo) GetByID(ctx context.Context, id string) (*entity.Match, error) {
	var model Match
	err := r.db.Conn.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToMatchEntity(&model), nil
}

// GetPending retrieves pending matches with full offer and request details
func (r *MatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.MatchWithDetails, error) {
	var matches []Match
	err := r.db.Conn.WithContext(ctx).
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
	return ToMatchesWithDetailsEntity(matches), nil
}

// GetByOfferID retrieves all matches for a given offer
func (r *MatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*entity.Match, error) {
	var matches []Match
	err := r.db.Conn.WithContext(ctx).
		Where("offer_id = ?", offerID).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}
	return ToMatchesEntity(matches), nil
}

// GetByRequestID retrieves all matches for a given request
func (r *MatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*entity.Match, error) {
	var matches []Match
	err := r.db.Conn.WithContext(ctx).
		Where("request_id = ?", requestID).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}
	return ToMatchesEntity(matches), nil
}

// UpdateStatus updates the status of a match
func (r *MatchRepo) UpdateStatus(ctx context.Context, id string, status entity.MatchStatus, matchedBy, notes string) error {
	updates := map[string]interface{}{
		"status":     string(status),
		"matched_by": matchedBy,
		"notes":      notes,
	}

	if status == entity.MatchStatusConfirmed {
		updates["confirmed_at"] = time.Now()
	}

	return r.db.Conn.WithContext(ctx).
		Model(&Match{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// CountPending returns the count of pending matches
func (r *MatchRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&Match{}).
		Where("status = ?", "PENDING").
		Count(&count).Error
	return count, err
}

// CountConfirmedToday returns the count of matches confirmed today
func (r *MatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&Match{}).
		Where("status = ? AND date(confirmed_at) = date('now')", "CONFIRMED").
		Count(&count).Error
	return count, err
}

// GetStaleMatches finds matches older than maxAge with specified statuses (for escalation)
func (r *MatchRepo) GetStaleMatches(ctx context.Context, statuses []entity.MatchStatus, maxAge time.Duration, limit int) ([]*entity.Match, error) {
	if len(statuses) == 0 {
		return nil, nil
	}

	// Convert statuses to strings
	statusStrings := make([]string, len(statuses))
	for i, s := range statuses {
		statusStrings[i] = string(s)
	}

	cutoff := time.Now().Add(-maxAge)

	var matches []Match
	err := r.db.Conn.WithContext(ctx).
		Where("status IN ?", statusStrings).
		Where("created_at < ?", cutoff).
		Where("confirmed_at IS NULL"). // Only unresolved matches
		Order("created_at ASC").       // Oldest first
		Limit(limit).
		Find(&matches).Error
	if err != nil {
		return nil, err
	}

	return ToMatchesEntity(matches), nil
}
