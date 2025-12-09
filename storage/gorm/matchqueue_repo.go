// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"github.com/google/uuid"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that MatchQueueRepo implements the interface
var _ repository.MatchQueueRepository = (*MatchQueueRepo)(nil)

// MatchQueueRepo implements repository.MatchQueueRepository using GORM
type MatchQueueRepo struct {
	db *DB
}

// NewMatchQueueRepo creates a new GORM-based match queue repository
func NewMatchQueueRepo(db *DB) *MatchQueueRepo {
	return &MatchQueueRepo{db: db}
}

// Enqueue adds a new item to the match queue
func (r *MatchQueueRepo) Enqueue(ctx context.Context, item *entity.MatchQueueItem) error {
	// Generate ID if not provided
	if item.ID == "" {
		item.ID = uuid.New().String()
	}
	if item.CreatedAt.IsZero() {
		item.CreatedAt = time.Now()
	}
	model := ToMatchQueueModel(item)
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// DequeueBatch retrieves a batch of items from the queue (oldest first)
func (r *MatchQueueRepo) DequeueBatch(ctx context.Context, limit int) ([]*entity.MatchQueueItem, error) {
	var items []MatchQueue
	err := r.db.Conn.WithContext(ctx).
		Order("created_at ASC").
		Limit(limit).
		Find(&items).Error
	if err != nil {
		return nil, err
	}
	return ToMatchQueueItemsEntity(items), nil
}

// Delete removes an item from the queue by ID
func (r *MatchQueueRepo) Delete(ctx context.Context, id string) error {
	return r.db.Conn.WithContext(ctx).
		Where("id = ?", id).
		Delete(&MatchQueue{}).Error
}

// Count returns the number of items in the queue
func (r *MatchQueueRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&MatchQueue{}).
		Count(&count).Error
	return int(count), err
}
