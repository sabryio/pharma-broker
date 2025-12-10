package repository

import (
	"context"

	"pharmabroker/domain/entity"
)

// BotUserRepository handles bot user storage and cross-platform matching.
type BotUserRepository interface {
	// Create/Update
	Save(ctx context.Context, user *entity.BotUser) error
	UpdateLastActive(ctx context.Context, id string) error

	// Find by platform ID
	GetByID(ctx context.Context, id string) (*entity.BotUser, error)
	GetByTelegramID(ctx context.Context, telegramID int64) (*entity.BotUser, error)
	GetByWhatsAppJID(ctx context.Context, jid string) (*entity.BotUser, error)
	GetByPhone(ctx context.Context, phone string) (*entity.BotUser, error)

	// Cross-platform linking
	LinkTelegram(ctx context.Context, userID string, telegramID int64, name string) error
	LinkWhatsApp(ctx context.Context, userID string, jid string) error
	LinkPhone(ctx context.Context, userID string, phone string) error

	// Authorization
	Authorize(ctx context.Context, userID string, role entity.UserRole, authorizedBy string) error
	Deauthorize(ctx context.Context, userID string) error
	GetAuthorized(ctx context.Context) ([]*entity.BotUser, error)

	// Queries
	GetAll(ctx context.Context, limit, offset int) ([]*entity.BotUser, error)
	Count(ctx context.Context) (int64, error)
}
