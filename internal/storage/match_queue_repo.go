package storage

import (
	"context"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormMatchQueueRepo implements domain.MatchQueueRepository using GORM
type GormMatchQueueRepo struct {
	db *GormDB
}

// NewGormMatchQueueRepo creates a new GORM-based match queue repository
func NewGormMatchQueueRepo(db *GormDB) *GormMatchQueueRepo {
	return &GormMatchQueueRepo{db: db}
}

// Enqueue adds a new item to the match queue
func (r *GormMatchQueueRepo) Enqueue(ctx context.Context, item *domain.MatchQueueItem) error {
	model := ToMatchQueueModel(item)
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// DequeueBatch retrieves and removes a batch of items from the queue
func (r *GormMatchQueueRepo) DequeueBatch(ctx context.Context, limit int) ([]*domain.MatchQueueItem, error) {
	var items []models.MatchQueue
	err := r.db.DB.WithContext(ctx).
		Order("created_at ASC").
		Limit(limit).
		Find(&items).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.MatchQueueItem, len(items))
	for i := range items {
		result[i] = ToMatchQueueItemDomain(&items[i])
	}
	return result, nil
}

// Delete removes an item from the queue by ID
func (r *GormMatchQueueRepo) Delete(ctx context.Context, id string) error {
	return r.db.DB.WithContext(ctx).
		Where("id = ?", id).
		Delete(&models.MatchQueue{}).Error
}

// Count returns the number of items in the queue
func (r *GormMatchQueueRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.MatchQueue{}).
		Count(&count).Error
	return int(count), err
}
