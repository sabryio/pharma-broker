package gorm

import (
	"context"
	"time"

	"gorm.io/gorm"

	"pharmabroker/domain/entity"
	"pharmabroker/domain/repository"
	"pharmabroker/storage/gorm/models"
)

// BotUserRepo implements repository.BotUserRepository.
type BotUserRepo struct {
	db *gorm.DB
}

// NewBotUserRepo creates a new BotUserRepo.
func NewBotUserRepo(db *DB) *BotUserRepo {
	return &BotUserRepo{db: db.Conn}
}

// Ensure interface compliance
var _ repository.BotUserRepository = (*BotUserRepo)(nil)

// Save creates or updates a bot user.
func (r *BotUserRepo) Save(ctx context.Context, user *entity.BotUser) error {
	model := models.BotUserFromEntity(user)
	model.UpdatedAt = time.Now()
	if model.CreatedAt.IsZero() {
		model.CreatedAt = time.Now()
	}
	return r.db.WithContext(ctx).Save(model).Error
}

// UpdateLastActive updates the last active timestamp.
func (r *BotUserRepo) UpdateLastActive(ctx context.Context, id string) error {
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", id).
		Update("last_active_at", time.Now()).Error
}

// GetByID finds a user by ID.
func (r *BotUserRepo) GetByID(ctx context.Context, id string) (*entity.BotUser, error) {
	var model models.BotUser
	if err := r.db.WithContext(ctx).Where("id = ?", id).First(&model).Error; err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return model.ToEntity(), nil
}

// GetByTelegramID finds a user by Telegram ID.
func (r *BotUserRepo) GetByTelegramID(ctx context.Context, telegramID int64) (*entity.BotUser, error) {
	var model models.BotUser
	if err := r.db.WithContext(ctx).Where("telegram_id = ?", telegramID).First(&model).Error; err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return model.ToEntity(), nil
}

// GetByWhatsAppJID finds a user by WhatsApp JID.
func (r *BotUserRepo) GetByWhatsAppJID(ctx context.Context, jid string) (*entity.BotUser, error) {
	var model models.BotUser
	if err := r.db.WithContext(ctx).Where("whatsapp_jid = ?", jid).First(&model).Error; err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return model.ToEntity(), nil
}

// GetByPhone finds a user by phone number.
func (r *BotUserRepo) GetByPhone(ctx context.Context, phone string) (*entity.BotUser, error) {
	var model models.BotUser
	if err := r.db.WithContext(ctx).Where("phone = ?", phone).First(&model).Error; err != nil {
		if err == gorm.ErrRecordNotFound {
			return nil, nil
		}
		return nil, err
	}
	return model.ToEntity(), nil
}

// LinkTelegram links a Telegram account to an existing user.
func (r *BotUserRepo) LinkTelegram(ctx context.Context, userID string, telegramID int64, name string) error {
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", userID).
		Updates(map[string]interface{}{
			"telegram_id":   telegramID,
			"telegram_name": name,
			"updated_at":    time.Now(),
		}).Error
}

// LinkWhatsApp links a WhatsApp account to an existing user.
func (r *BotUserRepo) LinkWhatsApp(ctx context.Context, userID string, jid string) error {
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", userID).
		Updates(map[string]interface{}{
			"whatsapp_jid": jid,
			"updated_at":   time.Now(),
		}).Error
}

// LinkPhone links a phone number to an existing user.
func (r *BotUserRepo) LinkPhone(ctx context.Context, userID string, phone string) error {
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", userID).
		Updates(map[string]interface{}{
			"phone":      phone,
			"updated_at": time.Now(),
		}).Error
}

// Authorize grants authorization to a user.
func (r *BotUserRepo) Authorize(ctx context.Context, userID string, role entity.UserRole, authorizedBy string) error {
	now := time.Now()
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", userID).
		Updates(map[string]interface{}{
			"is_authorized": true,
			"role":          string(role),
			"authorized_at": now,
			"authorized_by": authorizedBy,
			"updated_at":    now,
		}).Error
}

// Deauthorize revokes authorization from a user.
func (r *BotUserRepo) Deauthorize(ctx context.Context, userID string) error {
	return r.db.WithContext(ctx).
		Model(&models.BotUser{}).
		Where("id = ?", userID).
		Updates(map[string]interface{}{
			"is_authorized": false,
			"updated_at":    time.Now(),
		}).Error
}

// GetAuthorized returns all authorized users.
func (r *BotUserRepo) GetAuthorized(ctx context.Context) ([]*entity.BotUser, error) {
	var results []models.BotUser
	if err := r.db.WithContext(ctx).
		Where("is_authorized = ?", true).
		Order("created_at DESC").
		Find(&results).Error; err != nil {
		return nil, err
	}

	users := make([]*entity.BotUser, len(results))
	for i, m := range results {
		users[i] = m.ToEntity()
	}
	return users, nil
}

// GetAll returns all users with pagination.
func (r *BotUserRepo) GetAll(ctx context.Context, limit, offset int) ([]*entity.BotUser, error) {
	var results []models.BotUser
	if err := r.db.WithContext(ctx).
		Order("created_at DESC").
		Limit(limit).
		Offset(offset).
		Find(&results).Error; err != nil {
		return nil, err
	}

	users := make([]*entity.BotUser, len(results))
	for i, m := range results {
		users[i] = m.ToEntity()
	}
	return users, nil
}

// Count returns the total number of users.
func (r *BotUserRepo) Count(ctx context.Context) (int64, error) {
	var count int64
	err := r.db.WithContext(ctx).Model(&models.BotUser{}).Count(&count).Error
	return count, err
}
