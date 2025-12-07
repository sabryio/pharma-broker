package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"
)

// OfferRepo implements domain.OfferRepository
type OfferRepo struct {
	db *DB
}

func NewOfferRepo(db *DB) *OfferRepo {
	return &OfferRepo{db: db}
}

func (r *OfferRepo) Save(ctx context.Context, offer *domain.Offer) error {
	now := time.Now()
	_, err := r.db.Conn().ExecContext(ctx, `
		INSERT INTO offers (id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, price, currency, expiry_date, batch_number,
			notes, raw_message, status, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			medication = excluded.medication,
			quantity = excluded.quantity,
			price = excluded.price,
			status = excluded.status,
			updated_at = excluded.updated_at
	`, offer.ID, offer.RawMessageID, offer.SourcePhone, offer.SourceName, offer.SourceGroup, offer.GroupName,
		offer.Medication, offer.MedicationRaw, offer.Quantity, offer.Unit, offer.Price, offer.Currency,
		offer.ExpiryDate, offer.BatchNumber, offer.Notes, offer.RawMessage, offer.Status, now, now)
	return err
}

func (r *OfferRepo) GetByID(ctx context.Context, id string) (*domain.Offer, error) {
	row := r.db.Conn().QueryRowContext(ctx, `
		SELECT id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, price, currency, expiry_date, batch_number,
			notes, raw_message, status, created_at, updated_at
		FROM offers WHERE id = ?
	`, id)

	return scanOffer(row)
}

func (r *OfferRepo) GetActive(ctx context.Context, limit, offset int) ([]*domain.Offer, error) {
	rows, err := r.db.Conn().QueryContext(ctx, `
		SELECT id, raw_message_id, source_phone, source_name, source_group, group_name,
			medication, medication_raw, quantity, unit, price, currency, expiry_date, batch_number,
			notes, raw_message, status, created_at, updated_at
		FROM offers
		WHERE status = 'ACTIVE'
		ORDER BY created_at DESC
		LIMIT ? OFFSET ?
	`, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanOffers(rows)
}

func (r *OfferRepo) Search(ctx context.Context, query string, limit, offset int) ([]*domain.Offer, error) {
	// Pass query directly to FTS (caller must format it correctly)
	// We just ensure it's safe from SQL injection via parameter binding

	rows, err := r.db.Conn().QueryContext(ctx, `
		SELECT o.id, o.raw_message_id, o.source_phone, o.source_name, o.source_group, o.group_name,
			o.medication, o.medication_raw, o.quantity, o.unit, o.price, o.currency, o.expiry_date, o.batch_number,
			o.notes, o.raw_message, o.status, o.created_at, o.updated_at
		FROM offers o
		JOIN offers_fts f ON o.rowid = f.rowid
		WHERE offers_fts MATCH ? AND o.status = 'ACTIVE'
		ORDER BY rank
		LIMIT ? OFFSET ?
	`, query, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanOffers(rows)
}

func (r *OfferRepo) UpdateStatus(ctx context.Context, id string, status domain.ItemStatus) error {
	_, err := r.db.Conn().ExecContext(ctx, `
		UPDATE offers SET status = ?, updated_at = ? WHERE id = ?
	`, status, time.Now(), id)
	return err
}

func (r *OfferRepo) CountActive(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn().QueryRowContext(ctx, `SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE'`).Scan(&count)
	return count, err
}

func scanOffer(row *sql.Row) (*domain.Offer, error) {
	offer := &domain.Offer{}
	var expiryDate sql.NullTime
	var batchNumber, unit, notes, currency, rawMsgID, sourceName, groupName sql.NullString
	var price sql.NullFloat64

	err := row.Scan(&offer.ID, &rawMsgID, &offer.SourcePhone, &sourceName, &offer.SourceGroup, &groupName,
		&offer.Medication, &offer.MedicationRaw, &offer.Quantity, &unit, &price, &currency,
		&expiryDate, &batchNumber, &notes, &offer.RawMessage, &offer.Status, &offer.CreatedAt, &offer.UpdatedAt)
	if err != nil {
		return nil, err
	}

	if rawMsgID.Valid {
		offer.RawMessageID = rawMsgID.String
	}
	if sourceName.Valid {
		offer.SourceName = sourceName.String
	}
	if groupName.Valid {
		offer.GroupName = groupName.String
	}
	if unit.Valid {
		val := unit.String
		offer.Unit = &val
	}
	if price.Valid {
		offer.Price = price.Float64
	}
	if currency.Valid {
		offer.Currency = currency.String
	}
	if expiryDate.Valid {
		offer.ExpiryDate = &expiryDate.Time
	}
	if batchNumber.Valid {
		offer.BatchNumber = batchNumber.String
	}
	if notes.Valid {
		offer.Notes = notes.String
	}

	return offer, nil
}

func scanOffers(rows *sql.Rows) ([]*domain.Offer, error) {
	var offers []*domain.Offer
	for rows.Next() {
		offer := &domain.Offer{}
		var expiryDate sql.NullTime
		var batchNumber, unit, notes, currency, rawMsgID, sourceName, groupName sql.NullString
		var price sql.NullFloat64

		err := rows.Scan(&offer.ID, &rawMsgID, &offer.SourcePhone, &sourceName, &offer.SourceGroup, &groupName,
			&offer.Medication, &offer.MedicationRaw, &offer.Quantity, &unit, &price, &currency,
			&expiryDate, &batchNumber, &notes, &offer.RawMessage, &offer.Status, &offer.CreatedAt, &offer.UpdatedAt)
		if err != nil {
			return nil, err
		}

		if rawMsgID.Valid {
			offer.RawMessageID = rawMsgID.String
		}
		if sourceName.Valid {
			offer.SourceName = sourceName.String
		}
		if groupName.Valid {
			offer.GroupName = groupName.String
		}
		if unit.Valid {
			val := unit.String
			offer.Unit = &val
		}
		if price.Valid {
			offer.Price = price.Float64
		}
		if currency.Valid {
			offer.Currency = currency.String
		}
		if expiryDate.Valid {
			offer.ExpiryDate = &expiryDate.Time
		}
		if batchNumber.Valid {
			offer.BatchNumber = batchNumber.String
		}
		if notes.Valid {
			offer.Notes = notes.String
		}

		offers = append(offers, offer)
	}

	return offers, rows.Err()
}
