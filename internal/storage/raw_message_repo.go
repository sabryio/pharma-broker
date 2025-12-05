package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"
)

// RawMessageRepo implements domain.RawMessageRepository
type RawMessageRepo struct {
	db *DB
}

func NewRawMessageRepo(db *DB) *RawMessageRepo {
	return &RawMessageRepo{db: db}
}

func (r *RawMessageRepo) Save(ctx context.Context, msg *domain.RawMessage) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO raw_messages (id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			content = excluded.content,
			timestamp = excluded.timestamp
	`, msg.ID, msg.GroupJID, msg.GroupName, msg.SenderJID, msg.SenderPhone, msg.SenderName, msg.Content, msg.Timestamp)
	return err
}

func (r *RawMessageRepo) GetByID(ctx context.Context, id string) (*domain.RawMessage, error) {
	row := r.db.conn.QueryRowContext(ctx, `
		SELECT id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp, processed_at, error
		FROM raw_messages WHERE id = ?
	`, id)

	msg := &domain.RawMessage{}
	var processedAt sql.NullTime
	var errStr sql.NullString

	err := row.Scan(&msg.ID, &msg.GroupJID, &msg.GroupName, &msg.SenderJID, &msg.SenderPhone, &msg.SenderName,
		&msg.Content, &msg.Timestamp, &processedAt, &errStr)
	if err != nil {
		return nil, err
	}

	if processedAt.Valid {
		msg.ProcessedAt = &processedAt.Time
	}
	if errStr.Valid {
		msg.Error = errStr.String
	}

	return msg, nil
}

func (r *RawMessageRepo) GetUnprocessed(ctx context.Context, limit int) ([]*domain.RawMessage, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp
		FROM raw_messages
		WHERE processed_at IS NULL
		ORDER BY timestamp ASC
		LIMIT ?
	`, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var messages []*domain.RawMessage
	for rows.Next() {
		msg := &domain.RawMessage{}
		if err := rows.Scan(&msg.ID, &msg.GroupJID, &msg.GroupName, &msg.SenderJID, &msg.SenderPhone,
			&msg.SenderName, &msg.Content, &msg.Timestamp); err != nil {
			return nil, err
		}
		messages = append(messages, msg)
	}

	return messages, rows.Err()
}

func (r *RawMessageRepo) MarkProcessed(ctx context.Context, id string, processErr error) error {
	var errStr sql.NullString
	if processErr != nil {
		errStr = sql.NullString{String: processErr.Error(), Valid: true}
	}

	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE raw_messages SET processed_at = ?, error = ? WHERE id = ?
	`, time.Now(), errStr, id)
	return err
}
