package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"
)

// FailedMessageRepo implements storage for failed messages (dead-letter queue)
type FailedMessageRepo struct {
	db *DB
}

// NewFailedMessageRepo creates a new FailedMessageRepo
func NewFailedMessageRepo(db *DB) *FailedMessageRepo {
	return &FailedMessageRepo{db: db}
}

// Save stores a failed message for later retry or investigation
func (r *FailedMessageRepo) Save(ctx context.Context, msg *domain.FailedMessage) error {
	query := `
		INSERT INTO failed_messages (id, raw_message_id, failure_reason, retry_count, failed_at, resolved_at)
		VALUES (?, ?, ?, ?, ?, ?)
		ON CONFLICT(raw_message_id) DO UPDATE SET
			failure_reason = excluded.failure_reason,
			retry_count = failed_messages.retry_count + 1,
			failed_at = excluded.failed_at
	`
	_, err := r.db.Conn().ExecContext(ctx, query,
		msg.ID,
		msg.RawMessageID,
		msg.FailureReason,
		msg.RetryCount,
		msg.FailedAt,
		msg.ResolvedAt,
	)
	return err
}

// GetUnresolved returns unresolved failed messages for retry
func (r *FailedMessageRepo) GetUnresolved(ctx context.Context, limit int) ([]*domain.FailedMessage, error) {
	query := `
		SELECT id, raw_message_id, failure_reason, retry_count, failed_at, resolved_at
		FROM failed_messages
		WHERE resolved_at IS NULL AND retry_count < 5
		ORDER BY failed_at ASC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []*domain.FailedMessage
	for rows.Next() {
		var msg domain.FailedMessage
		var resolvedAt sql.NullTime
		if err := rows.Scan(&msg.ID, &msg.RawMessageID, &msg.FailureReason, &msg.RetryCount, &msg.FailedAt, &resolvedAt); err != nil {
			return nil, err
		}
		if resolvedAt.Valid {
			msg.ResolvedAt = &resolvedAt.Time
		}
		result = append(result, &msg)
	}
	return result, rows.Err()
}

// MarkResolved marks a failed message as resolved
func (r *FailedMessageRepo) MarkResolved(ctx context.Context, id string) error {
	query := `UPDATE failed_messages SET resolved_at = ? WHERE id = ?`
	_, err := r.db.Conn().ExecContext(ctx, query, time.Now(), id)
	return err
}

// Count returns the count of unresolved failed messages
func (r *FailedMessageRepo) Count(ctx context.Context) (int, error) {
	var count int
	err := r.db.Reader().QueryRowContext(ctx, `SELECT COUNT(*) FROM failed_messages WHERE resolved_at IS NULL`).Scan(&count)
	return count, err
}

// IncrementRetry increments the retry count for a failed message
func (r *FailedMessageRepo) IncrementRetry(ctx context.Context, id string) error {
	query := `UPDATE failed_messages SET retry_count = retry_count + 1, failed_at = ? WHERE id = ?`
	_, err := r.db.Conn().ExecContext(ctx, query, time.Now(), id)
	return err
}
