// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"pharmabroker/domain/entity"
)

// LeaderboardRepo implements storage for demand leaderboard
type LeaderboardRepo struct {
	db *DB
}

// NewLeaderboardRepo creates a new leaderboard repository
func NewLeaderboardRepo(db *DB) *LeaderboardRepo {
	return &LeaderboardRepo{db: db}
}

// GetTopDemand returns medications with highest demand ratio
func (r *LeaderboardRepo) GetTopDemand(ctx context.Context, limit int) ([]*entity.DemandStats, error) {
	var results []struct {
		Medication   string
		RequestCount int
		OfferCount   int
		DemandRatio  float64
	}

	err := r.db.Conn.WithContext(ctx).Raw(`
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
	`, limit).Scan(&results).Error

	if err != nil {
		return nil, err
	}

	stats := make([]*entity.DemandStats, len(results))
	for i, r := range results {
		trend := "STABLE"
		if r.DemandRatio > 2.0 {
			trend = "UP"
		} else if r.DemandRatio < 0.5 {
			trend = "DOWN"
		}
		stats[i] = &entity.DemandStats{
			Medication:   r.Medication,
			RequestCount: r.RequestCount,
			OfferCount:   r.OfferCount,
			DemandRatio:  r.DemandRatio,
			Trend:        trend,
		}
	}
	return stats, nil
}

// GetDemandForMedication returns demand stats for a specific medication
func (r *LeaderboardRepo) GetDemandForMedication(ctx context.Context, medication string) (*entity.DemandStats, error) {
	var result struct {
		RequestCount int
		OfferCount   int
	}

	err := r.db.Conn.WithContext(ctx).Raw(`
		SELECT 
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
	`, medication, medication).Scan(&result).Error

	if err != nil {
		return nil, err
	}

	stat := &entity.DemandStats{
		Medication:   medication,
		RequestCount: result.RequestCount,
		OfferCount:   result.OfferCount,
		Trend:        "STABLE",
	}

	if stat.OfferCount > 0 {
		stat.DemandRatio = float64(stat.RequestCount) / float64(stat.OfferCount)
	} else if stat.RequestCount > 0 {
		stat.DemandRatio = 999.0
	}

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
	if err := r.db.Conn.WithContext(ctx).Where("1=1").Delete(&DemandLeaderboard{}).Error; err != nil {
		return err
	}

	// Insert fresh data
	return r.db.Conn.WithContext(ctx).Exec(`
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
	`).Error
}

// GetCachedLeaderboard returns the cached leaderboard
func (r *LeaderboardRepo) GetCachedLeaderboard(ctx context.Context, limit int) ([]*entity.DemandStats, error) {
	var leaderboard []DemandLeaderboard
	err := r.db.Conn.WithContext(ctx).
		Order("demand_ratio DESC").
		Limit(limit).
		Find(&leaderboard).Error
	if err != nil {
		return nil, err
	}

	stats := make([]*entity.DemandStats, len(leaderboard))
	for i, lb := range leaderboard {
		trend := "STABLE"
		if lb.DemandRatio > 2.0 {
			trend = "UP"
		} else if lb.DemandRatio < 0.5 {
			trend = "DOWN"
		}
		stats[i] = &entity.DemandStats{
			Medication:   lb.Medication,
			RequestCount: lb.RequestCount,
			OfferCount:   lb.OfferCount,
			DemandRatio:  lb.DemandRatio,
			Trend:        trend,
		}
	}
	return stats, nil
}

// GetLastRefreshTime returns when the leaderboard was last updated
func (r *LeaderboardRepo) GetLastRefreshTime(ctx context.Context) (time.Time, error) {
	var result struct {
		LastUpdated time.Time
	}
	err := r.db.Conn.WithContext(ctx).
		Model(&DemandLeaderboard{}).
		Select("MAX(last_updated) as last_updated").
		Scan(&result).Error
	return result.LastUpdated, err
}
