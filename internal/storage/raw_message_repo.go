package storage

import (
	"context"
	"database/sql"
	"fmt"
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
	// If ExternalID is present, use it for conflict resolution
	query := `
		INSERT INTO raw_messages (id, external_id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(external_id) DO UPDATE SET
			content = excluded.content,
			timestamp = excluded.timestamp,
			group_name = excluded.group_name
		WHERE excluded.external_id IS NOT NULL
	`

	// If no ExternalID (legacy), fallback to ID conflict (less likely to happen with new logic but safe)
	if msg.ExternalID == "" {
		query = `
			INSERT INTO raw_messages (id, external_id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
			ON CONFLICT(id) DO UPDATE SET
				content = excluded.content
		`
	}

	_, err := r.db.conn.ExecContext(ctx, query,
		msg.ID, msg.ExternalID, msg.GroupJID, msg.GroupName,
		msg.SenderJID, msg.SenderPhone, msg.SenderName,
		msg.Content, msg.Timestamp)
	return err
}

func (r *RawMessageRepo) GetByID(ctx context.Context, id string) (*domain.RawMessage, error) {
	row := r.db.conn.QueryRowContext(ctx, `
		SELECT id, external_id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp, processed_at, error
		FROM raw_messages WHERE id = ?
	`, id)

	msg := &domain.RawMessage{}
	var externalID sql.NullString
	var processedAt sql.NullTime
	var errStr sql.NullString

	err := row.Scan(&msg.ID, &externalID, &msg.GroupJID, &msg.GroupName, &msg.SenderJID, &msg.SenderPhone, &msg.SenderName,
		&msg.Content, &msg.Timestamp, &processedAt, &errStr)
	if err != nil {
		return nil, err
	}

	if externalID.Valid {
		msg.ExternalID = externalID.String
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

func (r *RawMessageRepo) GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*domain.RawMessage, error) {
	row := r.db.conn.QueryRowContext(ctx, `
		SELECT id, external_id, group_jid, group_name, sender_jid, sender_phone, sender_name, content, timestamp
		FROM raw_messages
		WHERE group_jid = ? AND sender_jid = ?
		ORDER BY timestamp DESC
		LIMIT 1
	`, groupJID, senderJID)

	msg := &domain.RawMessage{}
	var externalID sql.NullString

	err := row.Scan(&msg.ID, &externalID, &msg.GroupJID, &msg.GroupName, &msg.SenderJID, &msg.SenderPhone,
		&msg.SenderName, &msg.Content, &msg.Timestamp)
	if err != nil {
		return nil, err // potentially sql.ErrNoRows
	}

	if externalID.Valid {
		msg.ExternalID = externalID.String
	}

	return msg, nil
}

// ArchiveOldMessages moves messages older than cutoff to the archive database.
// It uses SQLite's ATTACH DATABASE to perform the move transactionally.
func (r *RawMessageRepo) ArchiveOldMessages(ctx context.Context, archivePath string, cutoff time.Time) (int64, error) {
	// 1. Attach Archive DB
	// We use "archive" as the schema alias.
	// Note: We need to handle potential relative paths if CWD varies, but config usually provides valid paths.
	_, err := r.db.conn.ExecContext(ctx, fmt.Sprintf("ATTACH DATABASE '%s' AS archive", archivePath))
	if err != nil {
		return 0, fmt.Errorf("failed to attach archive db: %w", err)
	}
	defer r.db.conn.ExecContext(ctx, "DETACH DATABASE archive")

	// 2. Ensure Schema Exists in Archive
	// We replicate the raw_messages table structure matching migration v1 (Consolidated)
	_, err = r.db.conn.ExecContext(ctx, `
		CREATE TABLE IF NOT EXISTS archive.raw_messages (
			id TEXT PRIMARY KEY,
			external_id TEXT UNIQUE, -- WhatsApp Deduplication ID
			group_jid TEXT NOT NULL, 
			group_name TEXT NOT NULL,
			sender_jid TEXT NOT NULL,
			sender_phone TEXT NOT NULL,
			sender_name TEXT,
			content TEXT NOT NULL,
			timestamp DATETIME NOT NULL,
			processed_at DATETIME,
			error TEXT,
			created_at DATETIME DEFAULT CURRENT_TIMESTAMP
		);
		CREATE UNIQUE INDEX IF NOT EXISTS archive.idx_raw_messages_external_id ON raw_messages(external_id);
		CREATE INDEX IF NOT EXISTS archive.idx_raw_messages_timestamp ON raw_messages(timestamp);
	`)
	if err != nil {
		return 0, fmt.Errorf("failed to create archive schema: %w", err)
	}

	// 3. Perform Copy-Delete Transaction
	tx, err := r.db.conn.BeginTx(ctx, nil)
	if err != nil {
		return 0, fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer tx.Rollback()

	// 3a. Copy to Archive
	res, err := tx.ExecContext(ctx, `
		INSERT OR IGNORE INTO archive.raw_messages 
		SELECT * FROM main.raw_messages WHERE timestamp < ?
	`, cutoff)
	if err != nil {
		return 0, fmt.Errorf("failed to copy messages: %w", err)
	}

	rowsAffected, _ := res.RowsAffected()

	// 3b. Delete from Main
	if rowsAffected > 0 {
		_, err = tx.ExecContext(ctx, `
			DELETE FROM main.raw_messages WHERE timestamp < ?
		`, cutoff)
		if err != nil {
			return 0, fmt.Errorf("failed to delete messages: %w", err)
		}
	}

	if err := tx.Commit(); err != nil {
		return 0, fmt.Errorf("failed to commit archive transaction: %w", err)
	}

	return rowsAffected, nil
}
