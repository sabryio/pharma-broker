package storage

import (
	"context"
	"encoding/json"
	"time"

	"github.com/google/uuid"
	"gorm.io/gorm"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// WeightHistoryRepo handles storage for weight configuration history
type WeightHistoryRepo struct {
	db *GormDB
}

// NewWeightHistoryRepo creates a new weight history repository
func NewWeightHistoryRepo(db *GormDB) *WeightHistoryRepo {
	return &WeightHistoryRepo{db: db}
}

// Save stores a new weight configuration
func (r *WeightHistoryRepo) Save(ctx context.Context, history *domain.WeightHistory) error {
	model := ToWeightHistoryModel(history)
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// GetCurrent retrieves the most recently applied weight configuration
func (r *WeightHistoryRepo) GetCurrent(ctx context.Context) (*domain.WeightHistory, error) {
	var model models.WeightHistory
	err := r.db.DB.WithContext(ctx).
		Order("applied_at DESC").
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToWeightHistoryDomain(&model), nil
}

// GetHistory retrieves weight history ordered by application time
func (r *WeightHistoryRepo) GetHistory(ctx context.Context, limit int) ([]*domain.WeightHistory, error) {
	var models []models.WeightHistory
	query := r.db.DB.WithContext(ctx).Order("applied_at DESC")

	if limit > 0 {
		query = query.Limit(limit)
	}

	err := query.Find(&models).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.WeightHistory, len(models))
	for i := range models {
		result[i] = ToWeightHistoryDomain(&models[i])
	}
	return result, nil
}

// GetBySource retrieves weight configurations by source type
func (r *WeightHistoryRepo) GetBySource(ctx context.Context, source domain.WeightSource, limit int) ([]*domain.WeightHistory, error) {
	var models []models.WeightHistory
	query := r.db.DB.WithContext(ctx).
		Where("source = ?", source).
		Order("applied_at DESC")

	if limit > 0 {
		query = query.Limit(limit)
	}

	err := query.Find(&models).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.WeightHistory, len(models))
	for i := range models {
		result[i] = ToWeightHistoryDomain(&models[i])
	}
	return result, nil
}

// GetByDateRange retrieves weight history within a date range
func (r *WeightHistoryRepo) GetByDateRange(ctx context.Context, startDate, endDate time.Time) ([]*domain.WeightHistory, error) {
	var models []models.WeightHistory
	err := r.db.DB.WithContext(ctx).
		Where("applied_at BETWEEN ? AND ?", startDate, endDate).
		Order("applied_at DESC").
		Find(&models).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.WeightHistory, len(models))
	for i := range models {
		result[i] = ToWeightHistoryDomain(&models[i])
	}
	return result, nil
}

// SaveWithMetrics saves weight configuration with performance metrics
func (r *WeightHistoryRepo) SaveWithMetrics(ctx context.Context,
	medicationWeight, dosageWeight, quantityWeight, priceWeight, recencyWeight float64,
	source domain.WeightSource,
	metrics *domain.PerformanceMetrics,
	notes string) error {

	// Serialize metrics to JSON
	metricsJSON := ""
	if metrics != nil {
		data, err := json.Marshal(metrics)
		if err != nil {
			return err
		}
		metricsJSON = string(data)
	}

	history := &domain.WeightHistory{
		ID:                 uuid.New().String(),
		MedicationWeight:   medicationWeight,
		DosageWeight:       dosageWeight,
		QuantityWeight:     quantityWeight,
		PriceWeight:        priceWeight,
		RecencyWeight:      recencyWeight,
		Source:             source,
		PerformanceMetrics: metricsJSON,
		AppliedAt:          time.Now(),
		Notes:              notes,
	}

	return r.Save(ctx, history)
}
