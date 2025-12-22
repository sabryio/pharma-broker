// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that RawMessageRepo implements the interface
var _ repository.RawMessageRepository = (*RawMessageRepo)(nil)

// RawMessageRepo implements repository.RawMessageRepository using GORM
type RawMessageRepo struct {
	db *DB
}

// NewRawMessageRepo creates a new GORM-based raw message repository
func NewRawMessageRepo(db *DB) *RawMessageRepo {
	return &RawMessageRepo{db: db}
}

// Save creates a new raw message
func (r *RawMessageRepo) Save(ctx context.Context, msg *entity.RawMessage) error {
	model := ToRawMessageModel(msg)
	return r.db.Conn.WithContext(ctx).Create(model).Error
}

// GetByID retrieves a raw message by its ID
func (r *RawMessageRepo) GetByID(ctx context.Context, id string) (*entity.RawMessage, error) {
	var model RawMessage
	err := r.db.Conn.WithContext(ctx).Where("id = ?", id).First(&model).Error
	if err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return ToRawMessageEntity(&model), nil
}

// GetUnprocessed retrieves unprocessed messages
func (r *RawMessageRepo) GetUnprocessed(ctx context.Context, limit int) ([]*entity.RawMessage, error) {
	var messages []RawMessage
	err := r.db.Conn.WithContext(ctx).
		Where("processed_at IS NULL").
		Order("timestamp ASC").
		Limit(limit).
		Find(&messages).Error
	if err != nil {
		return nil, err
	}
	return ToRawMessagesEntity(messages), nil
}

// MarkProcessed marks a message as processed
func (r *RawMessageRepo) MarkProcessed(ctx context.Context, id string, processErr error) error {
	updates := map[string]interface{}{
		"processed_at": time.Now(),
	}
	if processErr != nil {
		errStr := processErr.Error()
		updates["error"] = &errStr
	}
	return r.db.Conn.WithContext(ctx).
		Model(&RawMessage{}).
		Where("id = ?", id).
		Updates(updates).Error
}

// GetLastMessageBySender retrieves the last message from a sender in a group
func (r *RawMessageRepo) GetLastMessageBySender(ctx context.Context, groupJID, senderJID string) (*entity.RawMessage, error) {
	var msgs []RawMessage
	err := r.db.Conn.WithContext(ctx).
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
	return ToRawMessageEntity(&msgs[0]), nil
}

// DeleteOldMessages deletes messages older than cutoff that have been processed
func (r *RawMessageRepo) DeleteOldMessages(ctx context.Context, cutoff time.Time) (int64, error) {
	result := r.db.Conn.WithContext(ctx).
		Where("timestamp < ? AND processed_at IS NOT NULL", cutoff).
		Delete(&RawMessage{})
	return result.RowsAffected, result.Error
}
