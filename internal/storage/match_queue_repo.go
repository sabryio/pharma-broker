package storage

import (
	"context"
	"time"

	"pharmabroker/internal/domain"

	"github.com/google/uuid"
)

// MatchQueueRepo implements domain.MatchQueueRepository
type MatchQueueRepo struct {
	db *DB
}

// NewMatchQueueRepo creates a new repository
func NewMatchQueueRepo(db *DB) *MatchQueueRepo {
	return &MatchQueueRepo{db: db}
}

// Enqueue adds an item to the queue
func (r *MatchQueueRepo) Enqueue(ctx context.Context, item *domain.MatchQueueItem) error {
	query := `INSERT INTO match_queue (id, source_type, source_id, created_at) VALUES (?, ?, ?, ?)`

	if item.ID == "" {
		item.ID = uuid.New().String()
	}
	if item.CreatedAt.IsZero() {
		item.CreatedAt = time.Now()
	}

	_, err := r.db.Conn().ExecContext(ctx, query, item.ID, item.SourceType, item.SourceID, item.CreatedAt)
	return err
}

// DequeueBatch retrieves oldest N items
func (r *MatchQueueRepo) DequeueBatch(ctx context.Context, limit int) ([]*domain.MatchQueueItem, error) {
	query := `SELECT id, source_type, source_id, created_at FROM match_queue ORDER BY created_at ASC LIMIT ?`

	rows, err := r.db.Conn().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var items []*domain.MatchQueueItem
	for rows.Next() {
		var i domain.MatchQueueItem
		if err := rows.Scan(&i.ID, &i.SourceType, &i.SourceID, &i.CreatedAt); err != nil {
			return nil, err
		}
		items = append(items, &i)
	}
	return items, nil
}

// Delete removes an item from the queue (after processing)
func (r *MatchQueueRepo) Delete(ctx context.Context, id string) error {
	_, err := r.db.Conn().ExecContext(ctx, "DELETE FROM match_queue WHERE id = ?", id)
	return err
}

// Count returns queue size
func (r *MatchQueueRepo) Count(ctx context.Context) (int, error) {
	var count int
	err := r.db.Conn().QueryRowContext(ctx, "SELECT COUNT(*) FROM match_queue").Scan(&count)
	return count, err
}
