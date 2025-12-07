package storage

import (
	"context"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormStatsRepo implements domain.StatsRepository using GORM
type GormStatsRepo struct {
	db *GormDB
}

// NewGormStatsRepo creates a new GORM-based stats repository
func NewGormStatsRepo(db *GormDB) *GormStatsRepo {
	return &GormStatsRepo{db: db}
}

// NewStatsRepo is a backwards-compatible constructor
func NewStatsRepo(db *GormDB) *GormStatsRepo {
	return NewGormStatsRepo(db)
}

// GetStats returns aggregated statistics for the dashboard
func (r *GormStatsRepo) GetStats(ctx context.Context) (*domain.Stats, error) {
	stats := &domain.Stats{}

	// Active offers count
	var activeOffers int64
	r.db.DB.WithContext(ctx).Model(&models.Offer{}).Where("status = 'ACTIVE'").Count(&activeOffers)
	stats.ActiveOffers = activeOffers

	// Active requests count
	var activeRequests int64
	r.db.DB.WithContext(ctx).Model(&models.Request{}).Where("status = 'ACTIVE'").Count(&activeRequests)
	stats.ActiveRequests = activeRequests

	// Pending matches count
	var pendingMatches int64
	r.db.DB.WithContext(ctx).Model(&models.Match{}).Where("status = 'PENDING'").Count(&pendingMatches)
	stats.PendingMatches = pendingMatches

	// Confirmed today count
	var confirmedToday int64
	r.db.DB.WithContext(ctx).Model(&models.Match{}).
		Where("status = 'CONFIRMED' AND date(confirmed_at) = date('now')").
		Count(&confirmedToday)
	stats.ConfirmedToday = confirmedToday

	// Monitored groups count
	var monitoredGroups int64
	r.db.DB.WithContext(ctx).Model(&models.Group{}).Where("monitored = 1").Count(&monitoredGroups)
	stats.MonitoredGroups = int(monitoredGroups)

	return stats, nil
}

// GetProcessedToday returns count of messages processed today
func (r *GormStatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.RawMessage{}).
		Where("date(processed_at) = date('now')").
		Count(&count).Error
	return count, err
}
