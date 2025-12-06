package storage

import (
	"context"
	"time"

	"pharmabroker/internal/domain"
)

// LeaderboardRepo implements storage for demand leaderboard
type LeaderboardRepo struct {
	db *DB
}

// NewLeaderboardRepo creates a new LeaderboardRepo
func NewLeaderboardRepo(db *DB) *LeaderboardRepo {
	return &LeaderboardRepo{db: db}
}

// GetTopDemand returns medications with highest demand ratio
func (r *LeaderboardRepo) GetTopDemand(ctx context.Context, limit int) ([]*domain.DemandStats, error) {
	// Query active requests and offers directly for real-time stats
	query := `
		SELECT 
			r.medication,
			COUNT(DISTINCT r.id) as request_count,
			COALESCE(o.offer_count, 0) as offer_count,
			CASE WHEN COALESCE(o.offer_count, 0) = 0 
				 THEN 999.0 
				 ELSE CAST(COUNT(DISTINCT r.id) AS REAL) / o.offer_count 
			END as demand_ratio
		FROM requests r
		LEFT JOIN (
			SELECT medication, COUNT(*) as offer_count 
			FROM offers WHERE status = 'ACTIVE' 
			GROUP BY medication
		) o ON LOWER(r.medication) = LOWER(o.medication)
		WHERE r.status = 'ACTIVE'
		GROUP BY r.medication
		ORDER BY demand_ratio DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []*domain.DemandStats
	for rows.Next() {
		stat := &domain.DemandStats{}
		if err := rows.Scan(&stat.Medication, &stat.RequestCount, &stat.OfferCount, &stat.DemandRatio); err != nil {
			return nil, err
		}
		// Determine trend (simplified: no historical data yet)
		stat.Trend = "STABLE"
		if stat.DemandRatio > 2.0 {
			stat.Trend = "UP" // High demand
		} else if stat.DemandRatio < 0.5 {
			stat.Trend = "DOWN" // Low demand
		}
		result = append(result, stat)
	}
	return result, rows.Err()
}

// GetDemandForMedication returns demand stats for a specific medication
func (r *LeaderboardRepo) GetDemandForMedication(ctx context.Context, medication string) (*domain.DemandStats, error) {
	query := `
		SELECT 
			? as medication,
			COALESCE(r.request_count, 0) as request_count,
			COALESCE(o.offer_count, 0) as offer_count
		FROM (SELECT 1) dummy
		LEFT JOIN (
			SELECT COUNT(*) as request_count 
			FROM requests 
			WHERE LOWER(medication) = LOWER(?) AND status = 'ACTIVE'
		) r ON 1=1
		LEFT JOIN (
			SELECT COUNT(*) as offer_count 
			FROM offers 
			WHERE LOWER(medication) = LOWER(?) AND status = 'ACTIVE'
		) o ON 1=1
	`
	stat := &domain.DemandStats{}
	if err := r.db.Reader().QueryRowContext(ctx, query, medication, medication, medication).Scan(
		&stat.Medication, &stat.RequestCount, &stat.OfferCount,
	); err != nil {
		return nil, err
	}

	// Calculate demand ratio
	if stat.OfferCount > 0 {
		stat.DemandRatio = float64(stat.RequestCount) / float64(stat.OfferCount)
	} else if stat.RequestCount > 0 {
		stat.DemandRatio = 999.0 // Very high demand, no supply
	}

	// Determine trend
	stat.Trend = "STABLE"
	if stat.DemandRatio > 2.0 {
		stat.Trend = "UP"
	} else if stat.DemandRatio < 0.5 {
		stat.Trend = "DOWN"
	}

	return stat, nil
}

// RefreshLeaderboard updates the materialized leaderboard table
func (r *LeaderboardRepo) RefreshLeaderboard(ctx context.Context) error {
	// Delete old entries
	_, err := r.db.Conn().ExecContext(ctx, `DELETE FROM demand_leaderboard`)
	if err != nil {
		return err
	}

	// Insert fresh data
	query := `
		INSERT INTO demand_leaderboard (medication, request_count, offer_count, demand_ratio, last_updated)
		SELECT 
			r.medication,
			COUNT(DISTINCT r.id) as request_count,
			COALESCE(o.offer_count, 0) as offer_count,
			CASE WHEN COALESCE(o.offer_count, 0) = 0 
				 THEN 999.0 
				 ELSE CAST(COUNT(DISTINCT r.id) AS REAL) / o.offer_count 
			END as demand_ratio,
			datetime('now')
		FROM requests r
		LEFT JOIN (
			SELECT medication, COUNT(*) as offer_count 
			FROM offers WHERE status = 'ACTIVE' 
			GROUP BY medication
		) o ON LOWER(r.medication) = LOWER(o.medication)
		WHERE r.status = 'ACTIVE'
		GROUP BY r.medication
	`
	_, err = r.db.Conn().ExecContext(ctx, query)
	return err
}

// GetCachedLeaderboard returns the cached leaderboard (for fast reads)
func (r *LeaderboardRepo) GetCachedLeaderboard(ctx context.Context, limit int) ([]*domain.DemandStats, error) {
	query := `
		SELECT medication, request_count, offer_count, demand_ratio
		FROM demand_leaderboard
		ORDER BY demand_ratio DESC
		LIMIT ?
	`
	rows, err := r.db.Reader().QueryContext(ctx, query, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var result []*domain.DemandStats
	for rows.Next() {
		stat := &domain.DemandStats{}
		if err := rows.Scan(&stat.Medication, &stat.RequestCount, &stat.OfferCount, &stat.DemandRatio); err != nil {
			return nil, err
		}
		stat.Trend = "STABLE"
		if stat.DemandRatio > 2.0 {
			stat.Trend = "UP"
		} else if stat.DemandRatio < 0.5 {
			stat.Trend = "DOWN"
		}
		result = append(result, stat)
	}
	return result, rows.Err()
}

// GetLastRefreshTime returns when the leaderboard was last updated
func (r *LeaderboardRepo) GetLastRefreshTime(ctx context.Context) (time.Time, error) {
	var lastUpdated time.Time
	err := r.db.Reader().QueryRowContext(ctx, `SELECT MAX(last_updated) FROM demand_leaderboard`).Scan(&lastUpdated)
	return lastUpdated, err
}
