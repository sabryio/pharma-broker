package storage

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// FeedbackRecordRepo handles storage for match feedback records (for learning)
type FeedbackRecordRepo struct {
	db *GormDB
}

// NewFeedbackRecordRepo creates a new feedback record repository
func NewFeedbackRecordRepo(db *GormDB) *FeedbackRecordRepo {
	return &FeedbackRecordRepo{db: db}
}

// Save stores a new feedback record
func (r *FeedbackRecordRepo) Save(ctx context.Context, feedback *domain.FeedbackRecord) error {
	model := ToFeedbackRecordModel(feedback)
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// GetByID retrieves a feedback record by ID
func (r *FeedbackRecordRepo) GetByID(ctx context.Context, id string) (*domain.FeedbackRecord, error) {
	var model models.FeedbackRecord
	err := r.db.DB.WithContext(ctx).
		Where("id = ?", id).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToFeedbackRecordDomain(&model), nil
}

// GetByDateRange retrieves feedback within a date range
func (r *FeedbackRecordRepo) GetByDateRange(ctx context.Context, startDate, endDate time.Time) ([]*domain.FeedbackRecord, error) {
	var models []models.FeedbackRecord
	err := r.db.DB.WithContext(ctx).
		Where("feedback_at BETWEEN ? AND ?", startDate, endDate).
		Order("feedback_at DESC").
		Find(&models).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.FeedbackRecord, len(models))
	for i := range models {
		result[i] = ToFeedbackRecordDomain(&models[i])
	}
	return result, nil
}

// GetFeedbackStats calculates aggregated statistics for a date range
func (r *FeedbackRecordRepo) GetFeedbackStats(ctx context.Context, startDate, endDate time.Time) (*domain.FeedbackStats, error) {
	stats := &domain.FeedbackStats{}

	// Count total feedbacks
	var totalCount int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Where("feedback_at BETWEEN ? AND ?", startDate, endDate).
		Count(&totalCount).Error
	if err != nil {
		return nil, err
	}
	stats.TotalFeedbacks = int(totalCount)

	// Count confirmed
	var confirmedCount int64
	err = r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Where("feedback_at BETWEEN ? AND ? AND action = ?", startDate, endDate, "CONFIRMED").
		Count(&confirmedCount).Error
	if err != nil {
		return nil, err
	}
	stats.ConfirmedCount = int(confirmedCount)

	// Count rejected
	var rejectedCount int64
	err = r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Where("feedback_at BETWEEN ? AND ? AND action = ?", startDate, endDate, "REJECTED").
		Count(&rejectedCount).Error
	if err != nil {
		return nil, err
	}
	stats.RejectedCount = int(rejectedCount)

	// Calculate confirmation rate
	if stats.TotalFeedbacks > 0 {
		stats.ConfirmationRate = float64(stats.ConfirmedCount) / float64(stats.TotalFeedbacks)
	}

	// Get average scores for confirmed matches
	type AvgScores struct {
		AvgMedication float64
		AvgDosage     float64
		AvgQuantity   float64
		AvgPrice      float64
		AvgRecency    float64
		AvgTotal      float64
	}

	var confirmedAvg AvgScores
	err = r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Select(`
			AVG(medication_score) as avg_medication,
			AVG(dosage_score) as avg_dosage,
			AVG(quantity_score) as avg_quantity,
			AVG(price_score) as avg_price,
			AVG(recency_score) as avg_recency,
			AVG(total_score) as avg_total
		`).
		Where("feedback_at BETWEEN ? AND ? AND action = ?", startDate, endDate, "CONFIRMED").
		Scan(&confirmedAvg).Error
	if err != nil {
		return nil, err
	}

	stats.ConfirmedAvgMedication = confirmedAvg.AvgMedication
	stats.ConfirmedAvgDosage = confirmedAvg.AvgDosage
	stats.ConfirmedAvgQuantity = confirmedAvg.AvgQuantity
	stats.ConfirmedAvgPrice = confirmedAvg.AvgPrice
	stats.ConfirmedAvgRecency = confirmedAvg.AvgRecency
	stats.ConfirmedAvgTotal = confirmedAvg.AvgTotal

	// Get average scores for rejected matches
	var rejectedAvg AvgScores
	err = r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Select(`
			AVG(medication_score) as avg_medication,
			AVG(dosage_score) as avg_dosage,
			AVG(quantity_score) as avg_quantity,
			AVG(price_score) as avg_price,
			AVG(recency_score) as avg_recency,
			AVG(total_score) as avg_total
		`).
		Where("feedback_at BETWEEN ? AND ? AND action = ?", startDate, endDate, "REJECTED").
		Scan(&rejectedAvg).Error
	if err != nil {
		return nil, err
	}

	stats.RejectedAvgMedication = rejectedAvg.AvgMedication
	stats.RejectedAvgDosage = rejectedAvg.AvgDosage
	stats.RejectedAvgQuantity = rejectedAvg.AvgQuantity
	stats.RejectedAvgPrice = rejectedAvg.AvgPrice
	stats.RejectedAvgRecency = rejectedAvg.AvgRecency
	stats.RejectedAvgTotal = rejectedAvg.AvgTotal

	// Calculate differences (indicator of importance)
	stats.MedicationDiff = stats.ConfirmedAvgMedication - stats.RejectedAvgMedication
	stats.DosageDiff = stats.ConfirmedAvgDosage - stats.RejectedAvgDosage
	stats.QuantityDiff = stats.ConfirmedAvgQuantity - stats.RejectedAvgQuantity
	stats.PriceDiff = stats.ConfirmedAvgPrice - stats.RejectedAvgPrice
	stats.RecencyDiff = stats.ConfirmedAvgRecency - stats.RejectedAvgRecency

	return stats, nil
}

// CountByAction counts feedback records by action type
func (r *FeedbackRecordRepo) CountByAction(ctx context.Context, action domain.FeedbackAction, startDate, endDate time.Time) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.FeedbackRecord{}).
		Where("action = ? AND feedback_at BETWEEN ? AND ?", action, startDate, endDate).
		Count(&count).Error
	return count, err
}
