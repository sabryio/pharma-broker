// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"gorm.io/gorm/clause"

	"pharmabroker/domain/entity"
)

// UnmappedRepo implements unmapped medication storage
type UnmappedRepo struct {
	db *DB
}

// NewUnmappedRepo creates a new unmapped medication repository
func NewUnmappedRepo(db *DB) *UnmappedRepo {
	return &UnmappedRepo{db: db}
}

// Save creates or updates an unmapped medication record
func (r *UnmappedRepo) Save(ctx context.Context, rawText, aiOutput, sourceMessage, sourceGroup, messageID string) error {
	record := &UnmappedMedication{
		RawText:       rawText,
		AIOutput:      aiOutput,
		SourceMessage: sourceMessage,
		SourceGroup:   sourceGroup,
		MessageID:     messageID,
		Count:         1,
		CreatedAt:     time.Now(),
		UpdatedAt:     time.Now(),
	}

	return r.db.Conn.WithContext(ctx).Clauses(clause.OnConflict{
		Columns: []clause.Column{{Name: "raw_text"}},
		DoUpdates: clause.Assignments(map[string]any{
			"count":      clause.Expr{SQL: "count + 1"},
			"updated_at": time.Now(),
		}),
	}).Create(record).Error
}

// GetPending returns unmapped medications that haven't been reviewed
func (r *UnmappedRepo) GetPending(ctx context.Context, limit, offset int) ([]*entity.UnmappedMedication, error) {
	var results []UnmappedMedication
	err := r.db.Conn.WithContext(ctx).
		Where("reviewed = ?", false).
		Order("count DESC, created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&results).Error
	if err != nil {
		return nil, err
	}
	return toUnmappedEntities(results), nil
}

// GetByRawText finds an unmapped medication by raw text
func (r *UnmappedRepo) GetByRawText(ctx context.Context, rawText string) (*entity.UnmappedMedication, error) {
	var result UnmappedMedication
	err := r.db.Conn.WithContext(ctx).
		Where("raw_text = ?", rawText).
		First(&result).Error
	if err != nil {
		return nil, err
	}
	return toUnmappedEntity(&result), nil
}

// MarkReviewed marks a medication as reviewed with the approved English name
func (r *UnmappedRepo) MarkReviewed(ctx context.Context, id uint, approvedName, reviewedBy string) error {
	now := time.Now()
	return r.db.Conn.WithContext(ctx).
		Model(&UnmappedMedication{}).
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
func (r *UnmappedRepo) Count(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&UnmappedMedication{}).
		Count(&count).Error
	return count, err
}

// CountPending returns number of pending reviews
func (r *UnmappedRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.Conn.WithContext(ctx).
		Model(&UnmappedMedication{}).
		Where("reviewed = ?", false).
		Count(&count).Error
	return count, err
}

// toUnmappedEntity converts model to entity
func toUnmappedEntity(m *UnmappedMedication) *entity.UnmappedMedication {
	return &entity.UnmappedMedication{
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

// toUnmappedEntities converts model slice to entity slice
func toUnmappedEntities(ms []UnmappedMedication) []*entity.UnmappedMedication {
	result := make([]*entity.UnmappedMedication, len(ms))
	for i := range ms {
		result[i] = toUnmappedEntity(&ms[i])
	}
	return result
}
