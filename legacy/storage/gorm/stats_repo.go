// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that StatsRepo implements the interface
var _ repository.StatsRepository = (*StatsRepo)(nil)

// StatsRepo implements repository.StatsRepository using GORM
type StatsRepo struct {
	db *DB
}

// NewStatsRepo creates a new GORM-based stats repository
func NewStatsRepo(db *DB) *StatsRepo {
	return &StatsRepo{db: db}
}

// GetStats returns aggregated statistics for the dashboard
func (r *StatsRepo) GetStats(ctx context.Context) (*entity.Stats, error) {
	stats := &entity.Stats{}

	// Active offers count
	var activeOffers int64
	r.db.Conn.WithContext(ctx).Model(&Offer{}).Where("status = 'ACTIVE'").Count(&activeOffers)
	stats.ActiveOffers = activeOffers

	// Active requests count
	var activeRequests int64
	r.db.Conn.WithContext(ctx).Model(&Request{}).Where("status = 'ACTIVE'").Count(&activeRequests)
	stats.ActiveRequests = activeRequests

	// Pending matches count
	var pendingMatches int64
	r.db.Conn.WithContext(ctx).Model(&Match{}).Where("status = 'PENDING'").Count(&pendingMatches)
	stats.PendingMatches = pendingMatches

	// Confirmed today count
	var confirmedToday int64
	r.db.Conn.WithContext(ctx).Model(&Match{}).
		Where("status = 'CONFIRMED' AND date(confirmed_at) = date('now')").
		Count(&confirmedToday)
	stats.ConfirmedToday = confirmedToday

	// Monitored groups count
	var monitoredGroups int64
	r.db.Conn.WithContext(ctx).Model(&Group{}).Where("monitored = ?", true).Count(&monitoredGroups)
	stats.MonitoredGroups = int(monitoredGroups)

	return stats, nil
}

// GetProcessedToday returns count of messages processed today
func (r *StatsRepo) GetProcessedToday(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&RawMessage{}).
		Where("date(processed_at) = date('now')").
		Count(&count).Error
	return count, err
}
