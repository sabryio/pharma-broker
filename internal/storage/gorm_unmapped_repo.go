package storage

import (
	"time"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"
)

// GormUnmappedMedicationRepo implements domain.UnmappedMedicationRepo using GORM
type GormUnmappedMedicationRepo struct {
	db *gorm.DB
}

// NewGormUnmappedMedicationRepo creates a new repository
func NewGormUnmappedMedicationRepo(db *gorm.DB) *GormUnmappedMedicationRepo {
	return &GormUnmappedMedicationRepo{db: db}
}

// Save creates or updates an unmapped medication record
// If the same RawText already exists, it increments the count
func (r *GormUnmappedMedicationRepo) Save(rawText, aiOutput, sourceMessage, sourceGroup, messageID string) error {
	// Use upsert pattern: insert or update count on conflict
	record := &models.UnmappedMedication{
		RawText:       rawText,
		AIOutput:      aiOutput,
		SourceMessage: sourceMessage,
		SourceGroup:   sourceGroup,
		MessageID:     messageID,
		Count:         1,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}

	return r.db.Clauses(clause.OnConflict{
		Columns: []clause.Column{{Name: "raw_text"}},
		DoUpdates: clause.Assignments(map[string]any{
			"count":      gorm.Expr("count + 1"),
			"updated_at": time.Now(),
		}),
	}).Create(record).Error
}

// GetPending returns unmapped medications that haven't been reviewed
func (r *GormUnmappedMedicationRepo) GetPending(limit, offset int) ([]*domain.UnmappedMedication, error) {
	var results []models.UnmappedMedication
	err := r.db.Where("reviewed = ?", false).
		Order("count DESC, created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&results).Error
	if err != nil {
		return nil, err
	}
	return toUnmappedDomainSlice(results), nil
}

// GetByRawText finds an unmapped medication by raw text
func (r *GormUnmappedMedicationRepo) GetByRawText(rawText string) (*domain.UnmappedMedication, error) {
	var result models.UnmappedMedication
	err := r.db.Where("raw_text = ?", rawText).First(&result).Error
	if err != nil {
		return nil, err
	}
	return toUnmappedDomain(&result), nil
}

// MarkReviewed marks a medication as reviewed with the approved English name
func (r *GormUnmappedMedicationRepo) MarkReviewed(id uint, approvedName, reviewedBy string) error {
	now := time.Now()
	return r.db.Model(&models.UnmappedMedication{}).
		Where("id = ?", id).
		Updates(map[string]any{
			"reviewed":      true,
			"approved_name": approvedName,
			"reviewed_by":   reviewedBy,
			"reviewed_at":   &now,
			"updated_at":    now,
		}).Error
}

// Count returns the total number of unmapped medications
func (r *GormUnmappedMedicationRepo) Count() (int64, error) {
	var count int64
	err := r.db.Model(&models.UnmappedMedication{}).Count(&count).Error
	return count, err
}

// CountPending returns number of pending reviews
func (r *GormUnmappedMedicationRepo) CountPending() (int64, error) {
	var count int64
	err := r.db.Model(&models.UnmappedMedication{}).Where("reviewed = ?", false).Count(&count).Error
	return count, err
}

// toUnmappedDomain converts model to domain
func toUnmappedDomain(m *models.UnmappedMedication) *domain.UnmappedMedication {
	return &domain.UnmappedMedication{
		ID:            m.ID,
		RawText:       m.RawText,
		AIOutput:      m.AIOutput,
		SourceMessage: m.SourceMessage,
		SourceGroup:   m.SourceGroup,
		MessageID:     m.MessageID,
		Count:         m.Count,
		Reviewed:      m.Reviewed,
		ApprovedName:  m.ApprovedName,
		ReviewedAt:    m.ReviewedAt,
		ReviewedBy:    m.ReviewedBy,
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}
}

// toUnmappedDomainSlice converts model slice to domain slice
func toUnmappedDomainSlice(ms []models.UnmappedMedication) []*domain.UnmappedMedication {
	result := make([]*domain.UnmappedMedication, len(ms))
	for i := range ms {
		result[i] = toUnmappedDomain(&ms[i])
	}
	return result
}
