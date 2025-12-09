// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"

	"github.com/google/uuid"
	"gorm.io/gorm"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that MedicationMappingRepo implements the interface
var _ repository.MedicationMappingRepository = (*MedicationMappingRepo)(nil)

// MedicationMappingRepo implements repository.MedicationMappingRepository using GORM
type MedicationMappingRepo struct {
	db *DB
}

// NewMedicationMappingRepo creates a new GORM-based medication mapping repository
func NewMedicationMappingRepo(db *DB) *MedicationMappingRepo {
	return &MedicationMappingRepo{db: db}
}

// Save creates or updates a medication mapping
func (r *MedicationMappingRepo) Save(ctx context.Context, mapping *entity.MedicationMapping) error {
	// Generate ID if not provided
	if mapping.ID == "" {
		mapping.ID = uuid.NewString()
	}
	model := ToMedicationMappingModel(mapping)
	return r.db.Conn.WithContext(ctx).Save(model).Error
}

// GetByArabicName retrieves a mapping by Arabic name
func (r *MedicationMappingRepo) GetByArabicName(ctx context.Context, arabicName string) (*entity.MedicationMapping, error) {
	var model MedicationMapping
	err := r.db.Conn.WithContext(ctx).
		Where("arabic_name = ?", arabicName).
		First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToMedicationMappingEntity(&model), nil
}

// GetAll retrieves all medication mappings
func (r *MedicationMappingRepo) GetAll(ctx context.Context) ([]*entity.MedicationMapping, error) {
	var mappings []MedicationMapping
	err := r.db.Conn.WithContext(ctx).Find(&mappings).Error
	if err != nil {
		return nil, err
	}
	return ToMedicationMappingsEntity(mappings), nil
}

// Search performs FTS search on medication mappings
func (r *MedicationMappingRepo) Search(ctx context.Context, query string) ([]*entity.MedicationMapping, error) {
	var mappings []MedicationMapping

	sanitizedQuery := SanitizeFTSQuery(query)
	err := r.db.Conn.WithContext(ctx).
		Raw(`
			SELECT m.id, m.arabic_name, m.english_name, m.synonyms, m.embedding, m.created_at, m.updated_at
			FROM medication_mappings m
			JOIN medication_mappings_fts f ON m.rowid = f.rowid
			WHERE medication_mappings_fts MATCH ?
		`, sanitizedQuery).
		Scan(&mappings).Error

	if err != nil {
		return nil, err
	}
	return ToMedicationMappingsEntity(mappings), nil
}

// Count returns the total number of medication mappings
func (r *MedicationMappingRepo) Count(ctx context.Context) (int, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&MedicationMapping{}).
		Count(&count).Error
	return int(count), err
}
