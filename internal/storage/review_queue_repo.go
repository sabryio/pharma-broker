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

// ReviewQueueRepo implements domain.ReviewQueueRepository using GORM
type ReviewQueueRepo struct {
	db *GormDB
}

// NewReviewQueueRepo creates a new review queue repository
func NewReviewQueueRepo(db *GormDB) *ReviewQueueRepo {
	return &ReviewQueueRepo{db: db}
}

// Save creates or updates a review queue item
func (r *ReviewQueueRepo) Save(ctx context.Context, item *domain.ReviewQueueItem) error {
	if item.ID == "" {
		item.ID = uuid.New().String()
	}
	model := toReviewQueueModel(item)
	return r.db.DB.WithContext(ctx).Save(model).Error
}

// GetByID retrieves a review queue item by ID
func (r *ReviewQueueRepo) GetByID(ctx context.Context, id string) (*domain.ReviewQueueItem, error) {
	var model models.ReviewQueue
	err := r.db.DB.WithContext(ctx).Where("id = ?", id).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return toReviewQueueDomain(&model), nil
}

// GetPending retrieves pending review items with pagination
func (r *ReviewQueueRepo) GetPending(ctx context.Context, limit, offset int) ([]*domain.ReviewQueueItem, error) {
	var items []models.ReviewQueue
	err := r.db.DB.WithContext(ctx).
		Where("status = ?", string(domain.ReviewStatusPending)).
		Order("created_at ASC").
		Limit(limit).
		Offset(offset).
		Find(&items).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.ReviewQueueItem, len(items))
	for i := range items {
		result[i] = toReviewQueueDomain(&items[i])
	}
	return result, nil
}

// CountPending returns the number of pending reviews
func (r *ReviewQueueRepo) CountPending(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.DB.WithContext(ctx).
		Model(&models.ReviewQueue{}).
		Where("status = ?", string(domain.ReviewStatusPending)).
		Count(&count).Error
	return count, err
}

// Approve marks an item as approved with optional corrections
func (r *ReviewQueueRepo) Approve(ctx context.Context, id string, reviewedBy string, correctedItems []domain.ParsedItem, note string) error {
	now := time.Now()
	updates := map[string]interface{}{
		"status":      string(domain.ReviewStatusApproved),
		"reviewed_by": reviewedBy,
		"reviewed_at": now,
		"review_note": note,
		"updated_at":  now,
	}

	if len(correctedItems) > 0 {
		correctedJSON, err := json.Marshal(correctedItems)
		if err != nil {
			return err
		}
		updates["corrected_items"] = string(correctedJSON)
	}

	return r.db.DB.WithContext(ctx).
		Model(&models.ReviewQueue{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// Reject marks an item as rejected
func (r *ReviewQueueRepo) Reject(ctx context.Context, id string, reviewedBy string, reason string) error {
	now := time.Now()
	return r.db.DB.WithContext(ctx).
		Model(&models.ReviewQueue{}).
		Where("id = ?", id).
		Updates(map[string]interface{}{
			"status":      string(domain.ReviewStatusRejected),
			"reviewed_by": reviewedBy,
			"reviewed_at": now,
			"review_note": reason,
			"updated_at":  now,
		}).Error
}

// GetByRawMessageID finds review items for a specific message
func (r *ReviewQueueRepo) GetByRawMessageID(ctx context.Context, rawMessageID string) (*domain.ReviewQueueItem, error) {
	var model models.ReviewQueue
	err := r.db.DB.WithContext(ctx).Where("raw_message_id = ?", rawMessageID).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return toReviewQueueDomain(&model), nil
}

// toReviewQueueModel converts domain to GORM model
func toReviewQueueModel(d *domain.ReviewQueueItem) *models.ReviewQueue {
	var partialJSON, correctedJSON, failureReason, replyContext, reviewedBy, reviewNote *string

	if len(d.PartialItems) > 0 {
		data, _ := json.Marshal(d.PartialItems)
		s := string(data)
		partialJSON = &s
	}

	if len(d.CorrectedItems) > 0 {
		data, _ := json.Marshal(d.CorrectedItems)
		s := string(data)
		correctedJSON = &s
	}

	if d.FailureReason != "" {
		failureReason = &d.FailureReason
	}

	if d.ReplyContext != "" {
		replyContext = &d.ReplyContext
	}

	if d.ReviewedBy != "" {
		reviewedBy = &d.ReviewedBy
	}

	if d.ReviewNote != "" {
		reviewNote = &d.ReviewNote
	}

	return &models.ReviewQueue{
		ID:             d.ID,
		RawMessageID:   d.RawMessageID,
		GroupName:      d.GroupName,
		SenderName:     d.SenderName,
		Content:        d.Content,
		ReplyContext:   replyContext,
		PartialItems:   deref(partialJSON),
		ParsePass:      d.ParsePass,
		AvgConfidence:  d.AvgConfidence,
		FailureReason:  failureReason,
		Status:         string(d.Status),
		ReviewedBy:     reviewedBy,
		ReviewedAt:     d.ReviewedAt,
		ReviewNote:     reviewNote,
		CorrectedItems: correctedJSON,
		CreatedAt:      d.CreatedAt,
		UpdatedAt:      d.UpdatedAt,
	}
}

// toReviewQueueDomain converts GORM model to domain
func toReviewQueueDomain(m *models.ReviewQueue) *domain.ReviewQueueItem {
	item := &domain.ReviewQueueItem{
		ID:            m.ID,
		RawMessageID:  m.RawMessageID,
		GroupName:     m.GroupName,
		SenderName:    m.SenderName,
		Content:       m.Content,
		ParsePass:     m.ParsePass,
		AvgConfidence: m.AvgConfidence,
		Status:        domain.ReviewStatus(m.Status),
		ReviewedAt:    m.ReviewedAt,
		CreatedAt:     m.CreatedAt,
		UpdatedAt:     m.UpdatedAt,
	}

	if m.ReplyContext != nil {
		item.ReplyContext = *m.ReplyContext
	}

	if m.FailureReason != nil {
		item.FailureReason = *m.FailureReason
	}

	if m.ReviewedBy != nil {
		item.ReviewedBy = *m.ReviewedBy
	}

	if m.ReviewNote != nil {
		item.ReviewNote = *m.ReviewNote
	}

	// Parse JSON fields
	if m.PartialItems != "" {
		_ = json.Unmarshal([]byte(m.PartialItems), &item.PartialItems)
	}

	if m.CorrectedItems != nil && *m.CorrectedItems != "" {
		_ = json.Unmarshal([]byte(*m.CorrectedItems), &item.CorrectedItems)
	}

	return item
}
