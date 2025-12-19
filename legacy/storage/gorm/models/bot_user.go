package models

import (
	"time"

	"pharmabroker/domain/entity"
)

// BotUser represents the GORM model for bot users.
type BotUser struct {
	ID string `gorm:"primaryKey;type:text"`

	// Platform identifiers
	TelegramID   *int64  `gorm:"uniqueIndex;type:integer"`
	TelegramName string  `gorm:"type:text"`
	WhatsAppJID  *string `gorm:"uniqueIndex;type:text"`
	Phone        *string `gorm:"uniqueIndex;type:text"`

	// User info
	DisplayName  string `gorm:"type:text;not null"`
	FirstName    string `gorm:"type:text"`
	LastName     string `gorm:"type:text"`
	LanguageCode string `gorm:"type:text;default:'en'"`

	// Authorization
	Role         string     `gorm:"type:text;default:'viewer'"`
	IsAuthorized bool       `gorm:"default:false"`
	AuthorizedAt *time.Time `gorm:"type:timestamptz"`
	AuthorizedBy string     `gorm:"type:text"`

	// Metadata
	CreatedAt    time.Time `gorm:"type:timestamptz;not null"`
	UpdatedAt    time.Time `gorm:"type:timestamptz;not null"`
	LastActiveAt time.Time `gorm:"type:timestamptz"`
	Platform     string    `gorm:"type:text;not null"`
}

// TableName returns the table name.
func (BotUser) TableName() string {
	return "bot_users"
}

// ToEntity converts GORM model to domain entity.
func (m *BotUser) ToEntity() *entity.BotUser {
	return &entity.BotUser{
		ID:           m.ID,
		TelegramID:   m.TelegramID,
		TelegramName: m.TelegramName,
		WhatsAppJID:  m.WhatsAppJID,
		Phone:        m.Phone,
		DisplayName:  m.DisplayName,
		FirstName:    m.FirstName,
		LastName:     m.LastName,
		LanguageCode: m.LanguageCode,
		Role:         entity.UserRole(m.Role),
		IsAuthorized: m.IsAuthorized,
		AuthorizedAt: m.AuthorizedAt,
		AuthorizedBy: m.AuthorizedBy,
		CreatedAt:    m.CreatedAt,
		UpdatedAt:    m.UpdatedAt,
		LastActiveAt: m.LastActiveAt,
		Platform:     m.Platform,
	}
}

// FromEntity creates a GORM model from domain entity.
func BotUserFromEntity(e *entity.BotUser) *BotUser {
	return &BotUser{
		ID:           e.ID,
		TelegramID:   e.TelegramID,
		TelegramName: e.TelegramName,
		WhatsAppJID:  e.WhatsAppJID,
		Phone:        e.Phone,
		DisplayName:  e.DisplayName,
		FirstName:    e.FirstName,
		LastName:     e.LastName,
		LanguageCode: e.LanguageCode,
		Role:         string(e.Role),
		IsAuthorized: e.IsAuthorized,
		AuthorizedAt: e.AuthorizedAt,
		AuthorizedBy: e.AuthorizedBy,
		CreatedAt:    e.CreatedAt,
		UpdatedAt:    e.UpdatedAt,
		LastActiveAt: e.LastActiveAt,
		Platform:     e.Platform,
	}
}
