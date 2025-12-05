package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"
)

// MatchRepo implements domain.MatchRepository
type MatchRepo struct {
	db *DB
}

func NewMatchRepo(db *DB) *MatchRepo {
	return &MatchRepo{db: db}
}

func (r *MatchRepo) Save(ctx context.Context, match *domain.Match) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO matches (id, offer_id, request_id, score, reasoning, matched_by, status, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(offer_id, request_id) DO UPDATE SET
			score = excluded.score,
			reasoning = excluded.reasoning
	`, match.ID, match.OfferID, match.RequestID, match.Score, match.Reasoning, match.MatchedBy, match.Status, match.CreatedAt)
	return err
}

func (r *MatchRepo) GetByID(ctx context.Context, id string) (*domain.Match, error) {
	row := r.db.conn.QueryRowContext(ctx, `
		SELECT id, offer_id, request_id, score, reasoning, matched_by, status, created_at, confirmed_at, notes
		FROM matches WHERE id = ?
	`, id)

	match := &domain.Match{}
	var reasoning, matchedBy, notes sql.NullString
	var confirmedAt sql.NullTime

	err := row.Scan(&match.ID, &match.OfferID, &match.RequestID, &match.Score, &reasoning,
		&matchedBy, &match.Status, &match.CreatedAt, &confirmedAt, &notes)
	if err != nil {
		return nil, err
	}

	if reasoning.Valid {
		match.Reasoning = reasoning.String
	}
	if matchedBy.Valid {
		match.MatchedBy = matchedBy.String
	}
	if confirmedAt.Valid {
		match.ConfirmedAt = &confirmedAt.Time
	}
	if notes.Valid {
		match.Notes = notes.String
	}

	return match, nil
}

func (r *MatchRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.MatchWithDetails, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT m.id, m.offer_id, m.request_id, m.score, m.reasoning, m.matched_by, m.status, m.created_at, m.confirmed_at, m.notes,
			o.id, o.raw_message_id, o.source_phone, o.source_name, o.source_group, o.group_name,
			o.medication, o.medication_raw, o.quantity, o.unit, o.price, o.currency, o.expiry_date, o.batch_number,
			o.notes, o.raw_message, o.status, o.created_at, o.updated_at,
			r.id, r.raw_message_id, r.source_phone, r.source_name, r.source_group, r.group_name,
			r.medication, r.medication_raw, r.quantity, r.unit, r.max_price, r.currency, r.urgent,
			r.notes, r.raw_message, r.status, r.created_at, r.updated_at
		FROM matches m
		JOIN offers o ON m.offer_id = o.id
		JOIN requests r ON m.request_id = r.id
		WHERE m.status = 'PENDING'
		ORDER BY m.score DESC, m.created_at DESC
		LIMIT ? OFFSET ?
	`, limit, offset)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var matches []*domain.MatchWithDetails
	for rows.Next() {
		mwd := &domain.MatchWithDetails{
			Offer:   &domain.Offer{},
			Request: &domain.Request{},
		}

		var mReasoning, mMatchedBy, mNotes sql.NullString
		var mConfirmedAt sql.NullTime

		var oExpiryDate sql.NullTime
		var oBatchNumber, oUnit, oNotes, oCurrency, oRawMsgID, oSourceName, oGroupName sql.NullString
		var oPrice sql.NullFloat64

		var rUnit, rNotes, rCurrency, rRawMsgID, rSourceName, rGroupName sql.NullString
		var rMaxPrice sql.NullFloat64

		err := rows.Scan(
			&mwd.ID, &mwd.OfferID, &mwd.RequestID, &mwd.Score, &mReasoning, &mMatchedBy, &mwd.Status, &mwd.CreatedAt, &mConfirmedAt, &mNotes,
			&mwd.Offer.ID, &oRawMsgID, &mwd.Offer.SourcePhone, &oSourceName, &mwd.Offer.SourceGroup, &oGroupName,
			&mwd.Offer.Medication, &mwd.Offer.MedicationRaw, &mwd.Offer.Quantity, &oUnit, &oPrice, &oCurrency,
			&oExpiryDate, &oBatchNumber, &oNotes, &mwd.Offer.RawMessage, &mwd.Offer.Status, &mwd.Offer.CreatedAt, &mwd.Offer.UpdatedAt,
			&mwd.Request.ID, &rRawMsgID, &mwd.Request.SourcePhone, &rSourceName, &mwd.Request.SourceGroup, &rGroupName,
			&mwd.Request.Medication, &mwd.Request.MedicationRaw, &mwd.Request.Quantity, &rUnit, &rMaxPrice, &rCurrency,
			&mwd.Request.Urgent, &rNotes, &mwd.Request.RawMessage, &mwd.Request.Status, &mwd.Request.CreatedAt, &mwd.Request.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}

		// Match nullables
		if mReasoning.Valid {
			mwd.Reasoning = mReasoning.String
		}
		if mMatchedBy.Valid {
			mwd.MatchedBy = mMatchedBy.String
		}
		if mConfirmedAt.Valid {
			mwd.ConfirmedAt = &mConfirmedAt.Time
		}
		if mNotes.Valid {
			mwd.Notes = mNotes.String
		}

		// Offer nullables
		if oRawMsgID.Valid {
			mwd.Offer.RawMessageID = oRawMsgID.String
		}
		if oSourceName.Valid {
			mwd.Offer.SourceName = oSourceName.String
		}
		if oGroupName.Valid {
			mwd.Offer.GroupName = oGroupName.String
		}
		if oUnit.Valid {
			mwd.Offer.Unit = oUnit.String
		}
		if oPrice.Valid {
			mwd.Offer.Price = oPrice.Float64
		}
		if oCurrency.Valid {
			mwd.Offer.Currency = oCurrency.String
		}
		if oExpiryDate.Valid {
			mwd.Offer.ExpiryDate = &oExpiryDate.Time
		}
		if oBatchNumber.Valid {
			mwd.Offer.BatchNumber = oBatchNumber.String
		}
		if oNotes.Valid {
			mwd.Offer.Notes = oNotes.String
		}

		// Request nullables
		if rRawMsgID.Valid {
			mwd.Request.RawMessageID = rRawMsgID.String
		}
		if rSourceName.Valid {
			mwd.Request.SourceName = rSourceName.String
		}
		if rGroupName.Valid {
			mwd.Request.GroupName = rGroupName.String
		}
		if rUnit.Valid {
			mwd.Request.Unit = rUnit.String
		}
		if rMaxPrice.Valid {
			mwd.Request.MaxPrice = rMaxPrice.Float64
		}
		if rCurrency.Valid {
			mwd.Request.Currency = rCurrency.String
		}
		if rNotes.Valid {
			mwd.Request.Notes = rNotes.String
		}

		matches = append(matches, mwd)
	}

	return matches, rows.Err()
}

func (r *MatchRepo) GetByOfferID(ctx context.Context, offerID string) ([]*domain.Match, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT id, offer_id, request_id, score, reasoning, matched_by, status, created_at, confirmed_at, notes
		FROM matches WHERE offer_id = ?
	`, offerID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanMatches(rows)
}

func (r *MatchRepo) GetByRequestID(ctx context.Context, requestID string) ([]*domain.Match, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT id, offer_id, request_id, score, reasoning, matched_by, status, created_at, confirmed_at, notes
		FROM matches WHERE request_id = ?
	`, requestID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanMatches(rows)
}

func (r *MatchRepo) UpdateStatus(ctx context.Context, id string, status domain.MatchStatus, matchedBy string) error {
	var confirmedAt interface{}
	if status == domain.MatchStatusConfirmed {
		confirmedAt = time.Now()
	}

	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE matches SET status = ?, matched_by = ?, confirmed_at = ? WHERE id = ?
	`, status, matchedBy, confirmedAt, id)
	return err
}

func (r *MatchRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM matches WHERE status = 'PENDING'`).Scan(&count)
	return count, err
}

func (r *MatchRepo) CountConfirmedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.conn.QueryRowContext(ctx, `
		SELECT COUNT(*) FROM matches 
		WHERE status = 'CONFIRMED' AND date(confirmed_at) = date('now')
	`).Scan(&count)
	return count, err
}

func scanMatches(rows *sql.Rows) ([]*domain.Match, error) {
	var matches []*domain.Match
	for rows.Next() {
		match := &domain.Match{}
		var reasoning, matchedBy, notes sql.NullString
		var confirmedAt sql.NullTime

		err := rows.Scan(&match.ID, &match.OfferID, &match.RequestID, &match.Score, &reasoning,
			&matchedBy, &match.Status, &match.CreatedAt, &confirmedAt, &notes)
		if err != nil {
			return nil, err
		}

		if reasoning.Valid {
			match.Reasoning = reasoning.String
		}
		if matchedBy.Valid {
			match.MatchedBy = matchedBy.String
		}
		if confirmedAt.Valid {
			match.ConfirmedAt = &confirmedAt.Time
		}
		if notes.Valid {
			match.Notes = notes.String
		}

		matches = append(matches, match)
	}

	return matches, rows.Err()
}
