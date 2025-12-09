// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"encoding/json"
	"time"

	"github.com/google/uuid"
	"gorm.io/gorm"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that ReviewQueueRepo implements the interface
var _ repository.ReviewQueueRepository = (*ReviewQueueRepo)(nil)

// ReviewQueueRepo implements repository.ReviewQueueRepository using GORM
type ReviewQueueRepo struct {
	db *DB
}

// NewReviewQueueRepo creates a new GORM-based review queue repository
func NewReviewQueueRepo(db *DB) *ReviewQueueRepo {
	return &ReviewQueueRepo{db: db}
}

// Save creates or updates a review queue item
func (r *ReviewQueueRepo) Save(ctx context.Context, item *entity.ReviewQueueItem) error {
	if item.ID == "" {
		item.ID = uuid.New().String()
	}
	model := ToReviewQueueModel(item)
	return r.db.Conn.WithContext(ctx).Save(model).Error
}

// GetByID retrieves a review queue item by ID
func (r *ReviewQueueRepo) GetByID(ctx context.Context, id string) (*entity.ReviewQueueItem, error) {
	var model ReviewQueue
	err := r.db.Conn.WithContext(ctx).Where("id = ?", id).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToReviewQueueItemEntity(&model), nil
}

// GetPending retrieves pending review items with pagination
func (r *ReviewQueueRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.ReviewQueueItem, error) {
	var items []ReviewQueue
	err := r.db.Conn.WithContext(ctx).
		Where("status = ?", string(entity.ReviewStatusPending)).
		Order("created_at ASC").
		Limit(limit).
		Offset(offset).
		Find(&items).Error
	if err != nil {
		return nil, err
	}
	return ToReviewQueueItemsEntity(items), nil
}

// CountPending returns the number of pending reviews
func (r *ReviewQueueRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&ReviewQueue{}).
		Where("status = ?", string(entity.ReviewStatusPending)).
		Count(&count).Error
	return count, err
}

// Approve marks an item as approved with optional corrections
func (r *ReviewQueueRepo) Approve(ctx context.Context, id string, reviewedBy string, correctedItems []entity.ParsedItem, note string) error {
	now := time.Now()
	updates := map[string]interface{}{
		"status":      string(entity.ReviewStatusApproved),
		"reviewed_by": reviewedBy,
		"reviewed_at": now,
		"review_note": note,
		"updated_at":  now,
	}

	if len(correctedItems) > 0 {
		correctedJSON, err := json.Marshal(correctedItems)
		if err != nil {
			return err
		}
		updates["corrected_items"] = string(correctedJSON)
	}

	return r.db.Conn.WithContext(ctx).
		Model(&ReviewQueue{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// Reject marks an item as rejected
func (r *ReviewQueueRepo) Reject(ctx context.Context, id string, reviewedBy string, reason string) error {
	now := time.Now()
	return r.db.Conn.WithContext(ctx).
		Model(&ReviewQueue{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":      string(entity.ReviewStatusRejected),
			"reviewed_by": reviewedBy,
			"reviewed_at": now,
			"review_note": reason,
			"updated_at":  now,
		}).Error
}

// GetByRawMessageID finds review items for a specific message
func (r *ReviewQueueRepo) GetByRawMessageID(ctx context.Context, rawMessageID string) (*entity.ReviewQueueItem, error) {
	var model ReviewQueue
	err := r.db.Conn.WithContext(ctx).Where("raw_message_id = ?", rawMessageID).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToReviewQueueItemEntity(&model), nil
}
