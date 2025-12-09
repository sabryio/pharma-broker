// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"github.com/google/uuid"
	"gorm.io/gorm"

	"pharmabroker/domain/entity"
)

// WeightHistoryRepo handles storage for weight configuration history
type WeightHistoryRepo struct {
	db *DB
}

// NewWeightHistoryRepo creates a new weight history repository
func NewWeightHistoryRepo(db *DB) *WeightHistoryRepo {
	return &WeightHistoryRepo{db: db}
}

// Save stores a new weight configuration
func (r *WeightHistoryRepo) Save(ctx context.Context, history *entity.WeightHistory) error {
	model := ToWeightHistoryModel(history)
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// GetCurrent retrieves the most recently applied weight configuration
func (r *WeightHistoryRepo) GetCurrent(ctx context.Context) (*entity.WeightHistory, error) {
	var model WeightHistory
	err := r.db.Conn.WithContext(ctx).
		Order("applied_at DESC").
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToWeightHistoryEntity(&model), nil
}

// GetHistory retrieves weight history ordered by application time
func (r *WeightHistoryRepo) GetHistory(ctx context.Context, limit int) ([]*entity.WeightHistory, error) {
	var models []WeightHistory
	query := r.db.Conn.WithContext(ctx).Order("applied_at DESC")

	if limit > 0 {
		query = query.Limit(limit)
	}

	err := query.Find(&models).Error
	if err != nil {
		return nil, err
	}

	return ToWeightHistoriesEntity(models), nil
}

// GetBySource retrieves weight configurations by source type
func (r *WeightHistoryRepo) GetBySource(ctx context.Context, source entity.WeightSource, limit int) ([]*entity.WeightHistory, error) {
	var models []WeightHistory
	query := r.db.Conn.WithContext(ctx).
		Where("source = ?", source).
		Order("applied_at DESC")

	if limit > 0 {
		query = query.Limit(limit)
	}

	err := query.Find(&models).Error
	if err != nil {
		return nil, err
	}

	return ToWeightHistoriesEntity(models), nil
}

// GetByDateRange retrieves weight history within a date range
func (r *WeightHistoryRepo) GetByDateRange(ctx context.Context, startDate, endDate time.Time) ([]*entity.WeightHistory, error) {
	var models []WeightHistory
	err := r.db.Conn.WithContext(ctx).
		Where("applied_at BETWEEN ? AND ?", startDate, endDate).
		Order("applied_at DESC").
		Find(&models).Error
	if err != nil {
		return nil, err
	}

	return ToWeightHistoriesEntity(models), nil
}

// SaveWithMetrics saves weight configuration with performance metrics
func (r *WeightHistoryRepo) SaveWithMetrics(ctx context.Context,
	medicationWeight, dosageWeight, quantityWeight, priceWeight, recencyWeight float64,
	source entity.WeightSource,
	metrics *entity.PerformanceMetrics,
	notes string) error {

	history := &entity.WeightHistory{
		ID:               uuid.New().String(),
		MedicationWeight: medicationWeight,
		DosageWeight:     dosageWeight,
		QuantityWeight:   quantityWeight,
		PriceWeight:      priceWeight,
		RecencyWeight:    recencyWeight,
		Source:           source,
		AppliedAt:        time.Now(),
		Notes:            notes,
	}

	// Note: metrics can be stored in notes field or a separate table if needed
	if metrics != nil {
		// Could serialize metrics to notes or store separately
	}

	return r.Save(ctx, history)
}
