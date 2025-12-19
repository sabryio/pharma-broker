package entity

import "time"

// UserRole defines authorization levels for bot users.
type UserRole string

const (
	UserRoleViewer   UserRole = "viewer"   // Can view status only
	UserRoleOperator UserRole = "operator" // Can confirm/reject matches
	UserRoleAdmin    UserRole = "admin"    // Full access
)

// BotUser represents a user who interacts with the bot across platforms.
type BotUser struct {
	ID string `json:"id"` // UUID

	// Platform identifiers (for cross-platform matching)
	TelegramID   *int64  `json:"telegram_id,omitempty"`   // Telegram user ID
	TelegramName string  `json:"telegram_name,omitempty"` // @username or display name
	WhatsAppJID  *string `json:"whatsapp_jid,omitempty"`  // WhatsApp JID
	Phone        *string `json:"phone,omitempty"`         // Phone number (E.164 format)

	// User info
	DisplayName  string `json:"display_name"`
	FirstName    string `json:"first_name,omitempty"`
	LastName     string `json:"last_name,omitempty"`
	LanguageCode string `json:"language_code"` // e.g., "en", "ar"

	// Authorization
	Role         UserRole   `json:"role"`
	IsAuthorized bool       `json:"is_authorized"`
	AuthorizedAt *time.Time `json:"authorized_at,omitempty"`
	AuthorizedBy string     `json:"authorized_by,omitempty"`

	// Metadata
	CreatedAt    time.Time `json:"created_at"`
	UpdatedAt    time.Time `json:"updated_at"`
	LastActiveAt time.Time `json:"last_active_at"`
	Platform     string    `json:"platform"` // First seen on: "telegram", "whatsapp"
}

// IsAdmin returns true if user has admin role.
func (u *BotUser) IsAdmin() bool {
	return u.Role == UserRoleAdmin
}

// IsOperator returns true if user can confirm/reject matches.
func (u *BotUser) IsOperator() bool {
	return u.Role == UserRoleOperator || u.Role == UserRoleAdmin
}

// CanExecuteCommands returns true if user is authorized to use bot.
func (u *BotUser) CanExecuteCommands() bool {
	return u.IsAuthorized
}

// FullName returns the user's full name.
func (u *BotUser) FullName() string {
	if u.LastName != "" {
		return u.FirstName + " " + u.LastName
	}
	return u.FirstName
}

// HasTelegram returns true if user has linked Telegram.
func (u *BotUser) HasTelegram() bool {
	return u.TelegramID != nil
}

// HasWhatsApp returns true if user has linked WhatsApp.
func (u *BotUser) HasWhatsApp() bool {
	return u.WhatsAppJID != nil && *u.WhatsAppJID != ""
}

// HasPhone returns true if user has a phone number.
func (u *BotUser) HasPhone() bool {
	return u.Phone != nil && *u.Phone != ""
}
