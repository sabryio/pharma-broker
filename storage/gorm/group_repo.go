// Package gorm provides GORM-based repository implementations.
package gorm

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
)

// Compile-time check that GroupRepo implements the interface
var _ repository.GroupRepository = (*GroupRepo)(nil)

// GroupRepo implements repository.GroupRepository using GORM
type GroupRepo struct {
	db *DB
}

// NewGroupRepo creates a new GORM-based group repository
func NewGroupRepo(db *DB) *GroupRepo {
	return &GroupRepo{db: db}
}

// Save creates or updates a group
func (r *GroupRepo) Save(ctx context.Context, group *entity.Group) error {
	model := ToGroupModel(group)
	return r.db.Conn.WithContext(ctx).Save(model).Error
}

// GetAll retrieves all groups
func (r *GroupRepo) GetAll(ctx context.Context) ([]*entity.Group, error) {
	var groups []Group
	err := r.db.Conn.WithContext(ctx).Find(&groups).Error
	if err != nil {
		return nil, err
	}
	return ToGroupsEntity(groups), nil
}

// GetMonitored retrieves only monitored groups
func (r *GroupRepo) GetMonitored(ctx context.Context) ([]*entity.Group, error) {
	var groups []Group
	err := r.db.Conn.WithContext(ctx).
		Where("monitored = ?", true).
		Find(&groups).Error
	if err != nil {
		return nil, err
	}
	return ToGroupsEntity(groups), nil
}

// SetMonitored updates the monitored status of a group
func (r *GroupRepo) SetMonitored(ctx context.Context, jid string, monitored bool) error {
	return r.db.Conn.WithContext(ctx).
		Model(&Group{}).
		Where("jid = ?", jid).
		Update("monitored", monitored).Error
}

// UpdateLastMessage updates the last message timestamp
func (r *GroupRepo) UpdateLastMessage(ctx context.Context, jid string) error {
	now := time.Now()
	return r.db.Conn.WithContext(ctx).
		Model(&Group{}).
		Where("jid = ?", jid).
		Update("last_message", &now).Error
}

// IncrementMessageCount increments the message count for a group
func (r *GroupRepo) IncrementMessageCount(ctx context.Context, jid string) error {
	return r.db.Conn.WithContext(ctx).
		Model(&Group{}).
		Where("jid = ?", jid).
		UpdateColumn("message_count", gorm.Expr("message_count + 1")).Error
}

// SaveFromSync creates or updates a group from WhatsApp sync
func (r *GroupRepo) SaveFromSync(ctx context.Context, jid, name, description string) error {
	now := time.Now()
	var descPtr *string
	if description != "" {
		descPtr = &description
	}

	group := &Group{
		JID:         jid,
		Name:        name,
		Description: descPtr,
		Monitored:   false, // New groups are not monitored by default
		AddedAt:     now,
	}

	// Upsert: update name/description if exists, otherwise create
	return r.db.Conn.WithContext(ctx).
		Where("jid = ?", jid).
		Assign(map[string]any{
			"name":        name,
			"description": descPtr,
		}).
		FirstOrCreate(group).Error
}

// EnableFromConfig enables monitoring for groups from config
func (r *GroupRepo) EnableFromConfig(ctx context.Context, jids []string) (int, error) {
	if len(jids) == 0 {
		return 0, nil
	}

	result := r.db.Conn.WithContext(ctx).
		Model(&Group{}).
		Where("jid IN ?", jids).
		Update("monitored", true)

	return int(result.RowsAffected), result.Error
}
