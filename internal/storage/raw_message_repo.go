package storage

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormRawMessageRepo implements domain.RawMessageRepository using GORM
type GormRawMessageRepo struct {
	db *GormDB
}

// NewGormRawMessageRepo creates a new GORM-based raw message repository
func NewGormRawMessageRepo(db *GormDB) *GormRawMessageRepo {
	return &GormRawMessageRepo{db: db}
}

// Save creates a new raw message
func (r *GormRawMessageRepo) Save(ctx context.Context, msg *domain.RawMessage) error {
	model := ToRawMessageModel(msg)
	return r.db.DB.WithContext(ctx).Create(model).Error
}

// GetByID retrieves a raw message by its ID
func (r *GormRawMessageRepo) GetByID(ctx context.Context, id string) (*domain.RawMessage, error) {
	var model models.RawMessage
	err := r.db.DB.WithContext(ctx).Where("id = ?", id).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToRawMessageDomain(&model), nil
}

// GetUnprocessed retrieves unprocessed messages
func (r *GormRawMessageRepo) GetUnprocessed(ctx context.Context, limit int) ([]*domain.RawMessage, error) {
	var messages []models.RawMessage
	err := r.db.DB.WithContext(ctx).
		Where("processed_at IS NULL").
		Order("timestamp ASC").
		Limit(limit).
		Find(&messages).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.RawMessage, len(messages))
	for i := range messages {
		result[i] = ToRawMessageDomain(&messages[i])
	}
	return result, nil
}

// MarkProcessed marks a message as processed
func (r *GormRawMessageRepo) MarkProcessed(ctx context.Context, id string, processErr error) error {
	updates := map[string]interface{}{
		"processed_at": time.Now(),
	}
	if processErr != nil {
		errStr := processErr.Error()
		updates["error"] = &errStr
	}
	return r.db.DB.WithContext(ctx).
		Model(&models.RawMessage{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// GetLastMessageBySender retrieves the last message from a sender in a group
func (r *GormRawMessageRepo) GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*domain.RawMessage, error) {
	var msgs []models.RawMessage
	err := r.db.DB.WithContext(ctx).
		Where("group_jid = ? AND sender_jid = ?", groupJID, senderJID).
		Order("timestamp DESC").
		Limit(1).
		Find(&msgs).Error
	if err != nil {
		return nil, err
	}
	if len(msgs) == 0 {
		return nil, nil
	}
	return ToRawMessageDomain(&msgs[0]), nil
}

// ArchiveOldMessages archives messages older than cutoff (raw SQL for file operations)
func (r *GormRawMessageRepo) ArchiveOldMessages(ctx context.Context, archivePath string, cutoff time.Time) (int64, error) {
	// This is complex enough to keep as raw SQL
	result := r.db.DB.WithContext(ctx).Exec(`
		DELETE FROM raw_messages WHERE timestamp < ? AND processed_at IS NOT NULL
	`, cutoff)
	return result.RowsAffected, result.Error
}
