package storage

import (
	"context"

	"gorm.io/gorm"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormMedicationMappingRepo implements domain.MedicationMappingRepository using GORM
type GormMedicationMappingRepo struct {
	db *GormDB
}

// NewGormMedicationMappingRepo creates a new GORM-based medication mapping repository
func NewGormMedicationMappingRepo(db *GormDB) *GormMedicationMappingRepo {
	return &GormMedicationMappingRepo{db: db}
}

// Save creates or updates a medication mapping
func (r *GormMedicationMappingRepo) Save(ctx context.Context, mapping *domain.MedicationMapping) error {
	model := ToMedicationMappingModel(mapping)
	return r.db.DB.WithContext(ctx).Save(model).Error
}

// GetByArabicName retrieves a mapping by Arabic name
func (r *GormMedicationMappingRepo) GetByArabicName(ctx context.Context, arabicName string) (*domain.MedicationMapping, error) {
	var model models.MedicationMapping
	err := r.db.DB.WithContext(ctx).
		Where("arabic_name = ?", arabicName).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToMedicationMappingDomain(&model), nil
}

// GetAll retrieves all medication mappings
func (r *GormMedicationMappingRepo) GetAll(ctx context.Context) ([]*domain.MedicationMapping, error) {
	var mappings []models.MedicationMapping
	err := r.db.DB.WithContext(ctx).Find(&mappings).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.MedicationMapping, len(mappings))
	for i := range mappings {
		result[i] = ToMedicationMappingDomain(&mappings[i])
	}
	return result, nil
}

// Search performs FTS search on medication mappings (raw SQL for FTS5)
func (r *GormMedicationMappingRepo) Search(ctx context.Context, query string) ([]*domain.MedicationMapping, error) {
	var mappings []models.MedicationMapping

	// FTS queries require raw SQL - using trigram tokenizer
	err := r.db.DB.WithContext(ctx).
		Raw(`
			SELECT m.id, m.arabic_name, m.english_name, m.synonyms, m.embedding, m.created_at, m.updated_at
			FROM medication_mappings m
			JOIN medication_mappings_fts f ON m.rowid = f.rowid
			WHERE medication_mappings_fts MATCH ?
		`, query).
		Scan(&mappings).Error

	if err != nil {
		return nil, err
	}

	result := make([]*domain.MedicationMapping, len(mappings))
	for i := range mappings {
		result[i] = ToMedicationMappingDomain(&mappings[i])
	}
	return result, nil
}

// Count returns the total number of medication mappings
func (r *GormMedicationMappingRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.MedicationMapping{}).
		Count(&count).Error
	return int(count), err
}
