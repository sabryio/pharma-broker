package storage

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage/models"
)

// GormGroupRepo implements domain.GroupRepository using GORM
type GormGroupRepo struct {
	db *GormDB
}

// NewGormGroupRepo creates a new GORM-based group repository
func NewGormGroupRepo(db *GormDB) *GormGroupRepo {
	return &GormGroupRepo{db: db}
}

// Save creates or updates a group
func (r *GormGroupRepo) Save(ctx context.Context, group *domain.Group) error {
	model := ToGroupModel(group)
	// Use Select("*") to save all fields including zero values like Monitored=false
	return r.db.DB.WithContext(ctx).Save(model).Error
}

// GetAll retrieves all groups
func (r *GormGroupRepo) GetAll(ctx context.Context) ([]*domain.Group, error) {
	var groups []models.Group
	err := r.db.DB.WithContext(ctx).Find(&groups).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.Group, len(groups))
	for i := range groups {
		result[i] = ToGroupDomain(&groups[i])
	}
	return result, nil
}

// GetMonitored retrieves only monitored groups
func (r *GormGroupRepo) GetMonitored(ctx context.Context) ([]*domain.Group, error) {
	var groups []models.Group
	err := r.db.DB.WithContext(ctx).
		Where("monitored = ?", true).
		Find(&groups).Error
	if err != nil {
		return nil, err
	}

	result := make([]*domain.Group, len(groups))
	for i := range groups {
		result[i] = ToGroupDomain(&groups[i])
	}
	return result, nil
}

// SetMonitored updates the monitored status of a group
func (r *GormGroupRepo) SetMonitored(ctx context.Context, jid string, monitored bool) error {
	return r.db.DB.WithContext(ctx).
		Model(&models.Group{}).
		Where("jid = ?", jid).
		Update("monitored", monitored).Error
}

// UpdateLastMessage updates the last message timestamp
func (r *GormGroupRepo) UpdateLastMessage(ctx context.Context, jid string) error {
	now := time.Now()
	return r.db.DB.WithContext(ctx).
		Model(&models.Group{}).
		Where("jid = ?", jid).
		Update("last_message", &now).Error
}

// IncrementMessageCount increments the message count for a group
func (r *GormGroupRepo) IncrementMessageCount(ctx context.Context, jid string) error {
	return r.db.DB.WithContext(ctx).
		Model(&models.Group{}).
		Where("jid = ?", jid).
		UpdateColumn("message_count", gorm.Expr("message_count + 1")).Error
}

// SaveFromSync creates or updates a group from WhatsApp sync
func (r *GormGroupRepo) SaveFromSync(ctx context.Context, jid, name, description string) error {
	now := time.Now()
	var descPtr *string
	if description != "" {
		descPtr = &description
	}

	group := &models.Group{
		JID:         jid,
		Name:        name,
		Description: descPtr,
		Monitored:   false, // New groups are not monitored by default
		AddedAt:     now,
	}

	// Upsert: update name/description if exists, otherwise create
	return r.db.DB.WithContext(ctx).
		Where("jid = ?", jid).
		Assign(map[string]any{
			"name":        name,
			"description": descPtr,
		}).
		FirstOrCreate(group).Error
}

// EnableFromConfig enables monitoring for groups from config (returns count of enabled groups)
func (r *GormGroupRepo) EnableFromConfig(ctx context.Context, jids []string) (int, error) {
	if len(jids) == 0 {
		return 0, nil
	}

	result := r.db.DB.WithContext(ctx).
		Model(&models.Group{}).
		Where("jid IN ?", jids).
		Update("monitored", true)

	return int(result.RowsAffected), result.Error
}
