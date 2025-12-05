package storage

import (
	"context"
	"database/sql"
	"time"

	"pharmabroker/internal/domain"
)

// GroupRepo implements domain.GroupRepository
type GroupRepo struct {
	db *DB
}

func NewGroupRepo(db *DB) *GroupRepo {
	return &GroupRepo{db: db}
}

func (r *GroupRepo) Save(ctx context.Context, group *domain.Group) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO groups (jid, name, description, monitored, added_at, last_message, message_count)
		VALUES (?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(jid) DO UPDATE SET
			name = excluded.name,
			description = excluded.description
	`, group.JID, group.Name, group.Description, group.Monitored, group.AddedAt, group.LastMessage, group.MessageCount)
	return err
}

// SaveFromSync saves a group from WhatsApp sync (creates if not exists)
func (r *GroupRepo) SaveFromSync(ctx context.Context, jid, name, description string) error {
	_, err := r.db.conn.ExecContext(ctx, `
		INSERT INTO groups (jid, name, description, monitored, added_at, message_count)
		VALUES (?, ?, ?, FALSE, datetime('now'), 0)
		ON CONFLICT(jid) DO UPDATE SET
			name = excluded.name,
			description = excluded.description
	`, jid, name, description)
	return err
}

func (r *GroupRepo) GetAll(ctx context.Context) ([]*domain.Group, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT jid, name, description, monitored, added_at, last_message, message_count
		FROM groups ORDER BY name
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanGroups(rows)
}

func (r *GroupRepo) GetMonitored(ctx context.Context) ([]*domain.Group, error) {
	rows, err := r.db.conn.QueryContext(ctx, `
		SELECT jid, name, description, monitored, added_at, last_message, message_count
		FROM groups WHERE monitored = TRUE ORDER BY name
	`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	return scanGroups(rows)
}

func (r *GroupRepo) SetMonitored(ctx context.Context, jid string, monitored bool) error {
	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE groups SET monitored = ? WHERE jid = ?
	`, monitored, jid)
	return err
}

func (r *GroupRepo) UpdateLastMessage(ctx context.Context, jid string) error {
	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE groups SET last_message = ? WHERE jid = ?
	`, time.Now(), jid)
	return err
}

func (r *GroupRepo) IncrementMessageCount(ctx context.Context, jid string) error {
	_, err := r.db.conn.ExecContext(ctx, `
		UPDATE groups SET message_count = message_count + 1 WHERE jid = ?
	`, jid)
	return err
}

// EnableFromConfig enables monitoring for groups specified in config.
// This allows running without frontend by specifying groups in config.yaml.
func (r *GroupRepo) EnableFromConfig(ctx context.Context, jids []string) (int, error) {
	if len(jids) == 0 {
		return 0, nil
	}

	var enabled int
	for _, jid := range jids {
		result, err := r.db.conn.ExecContext(ctx, `
			UPDATE groups SET monitored = TRUE WHERE jid = ?
		`, jid)
		if err != nil {
			return enabled, err
		}
		rows, _ := result.RowsAffected()
		if rows > 0 {
			enabled++
		}
	}
	return enabled, nil
}

func scanGroups(rows *sql.Rows) ([]*domain.Group, error) {
	var groups []*domain.Group
	for rows.Next() {
		group := &domain.Group{}
		var description sql.NullString
		var lastMessage sql.NullTime

		err := rows.Scan(&group.JID, &group.Name, &description, &group.Monitored, &group.AddedAt, &lastMessage, &group.MessageCount)
		if err != nil {
			return nil, err
		}

		if description.Valid {
			group.Description = description.String
		}
		if lastMessage.Valid {
			group.LastMessage = &lastMessage.Time
		}

		groups = append(groups, group)
	}

	return groups, rows.Err()
}

// StatsRepo implements domain.StatsRepository
type StatsRepo struct {
	db *DB
}

func NewStatsRepo(db *DB) *StatsRepo {
	return &StatsRepo{db: db}
}

func (r *StatsRepo) GetStats(ctx context.Context) (*domain.Stats, error) {
	stats := &domain.Stats{}

	// Count active offers
	if err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE'`).Scan(&stats.ActiveOffers); err != nil {
		return nil, err
	}

	// Count active requests
	if err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM requests WHERE status = 'ACTIVE'`).Scan(&stats.ActiveRequests); err != nil {
		return nil, err
	}

	// Count pending matches
	if err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM matches WHERE status = 'PENDING'`).Scan(&stats.PendingMatches); err != nil {
		return nil, err
	}

	// Count confirmed today
	if err := r.db.conn.QueryRowContext(ctx, `
		SELECT COUNT(*) FROM matches WHERE status = 'CONFIRMED' AND date(confirmed_at) = date('now')
	`).Scan(&stats.ConfirmedToday); err != nil {
		return nil, err
	}

	// Count processed today
	if err := r.db.conn.QueryRowContext(ctx, `
		SELECT COUNT(*) FROM raw_messages WHERE date(processed_at) = date('now')
	`).Scan(&stats.ProcessedToday); err != nil {
		return nil, err
	}

	// Average match score
	if err := r.db.conn.QueryRowContext(ctx, `
		SELECT COALESCE(AVG(score), 0) FROM matches WHERE status = 'PENDING'
	`).Scan(&stats.AvgMatchScore); err != nil {
		return nil, err
	}

	// Count monitored groups
	if err := r.db.conn.QueryRowContext(ctx, `SELECT COUNT(*) FROM groups WHERE monitored = TRUE`).Scan(&stats.MonitoredGroups); err != nil {
		return nil, err
	}

	return stats, nil
}

func (r *StatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.conn.QueryRowContext(ctx, `
		SELECT COUNT(*) FROM raw_messages WHERE date(processed_at) = date('now')
	`).Scan(&count)
	return count, err
}
