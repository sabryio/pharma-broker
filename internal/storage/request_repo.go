package storage

import (
	"context"
	"database/sql"
	"strings"
	"time"

	"pharmabroker/internal/domain"
)

// RequestRepo implements domain.RequestRepository
type RequestRepo struct {
	db *DB
}

func NewRequestRepo(db *DB) *RequestRepo {
	return &RequestRepo{db: db}
}

func (r *RequestRepo) Save(ctx context.Context, req *domain.Request) error {
	now := time.Now()
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO requests (id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, max_price, currency, urgent,
			notes, raw_message, status, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			medication = excluded.medication,
			quantity = excluded.quantity,
			max_price = excluded.max_price,
			status = excluded.status,
			updated_at = excluded.updated_at
	`, req.ID, req.RawMessageID, req.SourcePhone, req.SourceName, req.SourceGroup, req.GroupName,
		req.Medication, req.MedicationRaw, req.Quantity, req.Unit, req.MaxPrice, req.Currency,
		req.Urgent, req.Notes, req.RawMessage, req.Status, now, now)
	return err
}

func (r *RequestRepo) GetByID(ctx context.Context, id string) (*domain.Request, error) {
	row := r.db.conn.QueryRowContext(ctx, `
		SELECT id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, max_price, currency, urgent,
			notes, raw_message, status, created_at, updated_at
		FROM requests WHERE id = ?
	`, id)

	return scanRequest(row)
}

func (r *RequestRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Request, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, max_price, currency, urgent,
			notes, raw_message, status, created_at, updated_at
		FROM requests
		WHERE status = 'ACTIVE'
		ORDER BY urgent DESC, created_at DESC
		LIMIT ? OFFSET ?
	`, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanRequests(rows)
}

func (r *RequestRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Request, error) {
	// Escape FTS5 special characters by quoting the query
	safeQuery := "\"" + strings.ReplaceAll(query, "\"", "\"\"") + "\"" + "*"

	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT r.id, r.raw_message_id, r.source_phone, r.source_name, r.source_group, r.group_name,
			r.medication, r.medication_raw, r.quantity, r.unit, r.max_price, r.currency, r.urgent,
			r.notes, r.raw_message, r.status, r.created_at, r.updated_at
		FROM requests r
		JOIN requests_fts f ON r.rowid = f.rowid
		WHERE requests_fts MATCH ? AND r.status = 'ACTIVE'
		ORDER BY r.urgent DESC, rank
		LIMIT ? OFFSET ?
	`, safeQuery, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanRequests(rows)
}

func (r *RequestRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE requests SET status = ?, updated_at = ? WHERE id = ?
	`, status, time.Now(), id)
	return err
}

func (r *RequestRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM requests WHERE status = 'ACTIVE'`).Scan(&count)
	return count, err
}

func scanRequest(row *sql.Row) (*domain.Request, error) {
	req := &domain.Request{}
	var unit, notes, currency, rawMsgID, sourceName, groupName sql.NullString
	var maxPrice sql.NullFloat64

	err := row.Scan(&req.ID, &rawMsgID, &req.SourcePhone, &sourceName, &req.SourceGroup, &groupName,
		&req.Medication, &req.MedicationRaw, &req.Quantity, &unit, &maxPrice, &currency,
		&req.Urgent, &notes, &req.RawMessage, &req.Status, &req.CreatedAt, &req.UpdatedAt)
	if err != nil {
		return nil, err
	}

	if rawMsgID.Valid {
		req.RawMessageID = rawMsgID.String
	}
	if sourceName.Valid {
		req.SourceName = sourceName.String
	}
	if groupName.Valid {
		req.GroupName = groupName.String
	}
	if unit.Valid {
		req.Unit = unit.String
	}
	if maxPrice.Valid {
		req.MaxPrice = maxPrice.Float64
	}
	if currency.Valid {
		req.Currency = currency.String
	}
	if notes.Valid {
		req.Notes = notes.String
	}

	return req, nil
}

func scanRequests(rows *sql.Rows) ([]*domain.Request, error) {
	var requests []*domain.Request
	for rows.Next() {
		req := &domain.Request{}
		var unit, notes, currency, rawMsgID, sourceName, groupName sql.NullString
		var maxPrice sql.NullFloat64

		err := rows.Scan(&req.ID, &rawMsgID, &req.SourcePhone, &sourceName, &req.SourceGroup, &groupName,
			&req.Medication, &req.MedicationRaw, &req.Quantity, &unit, &maxPrice, &currency,
			&req.Urgent, &notes, &req.RawMessage, &req.Status, &req.CreatedAt, &req.UpdatedAt)
		if err != nil {
			return nil, err
		}

		if rawMsgID.Valid {
			req.RawMessageID = rawMsgID.String
		}
		if sourceName.Valid {
			req.SourceName = sourceName.String
		}
		if groupName.Valid {
			req.GroupName = groupName.String
		}
		if unit.Valid {
			req.Unit = unit.String
		}
		if maxPrice.Valid {
			req.MaxPrice = maxPrice.Float64
		}
		if currency.Valid {
			req.Currency = currency.String
		}
		if notes.Valid {
			req.Notes = notes.String
		}

		requests = append(requests, req)
	}

	return requests, rows.Err()
}
