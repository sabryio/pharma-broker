// Package notify provides notification service interfaces.
package notify

import "context"

// Service coordinates all notification channels
type Service interface {
	// SendMessage sends a text message via all enabled channels
	SendMessage(ctx context.Context, message string) error

	// SendDocument sends a file via all enabled channels
	SendDocument(ctx context.Context, filename string, data []byte, caption string) error

	// SendReport sends a full report via all channels
	SendReport(ctx context.Context, summary, htmlReport string, csvData []byte, csvFilename string) error
}

// TelegramNotifier sends notifications via Telegram
type TelegramNotifier interface {
	SendMessage(ctx context.Context, message string) error
	SendDocument(ctx context.Context, filename string, data []byte, caption string) error
}

// EmailNotifier sends notifications via SMTP
type EmailNotifier interface {
	SendReport(ctx context.Context, subject, htmlBody string, csvData []byte, csvFilename string) error
}

// TelegramConfig holds Telegram bot settings
type TelegramConfig struct {
	Enabled  bool
	BotToken string
	ChatIDs  []string
}

// EmailConfig holds email settings
type EmailConfig struct {
	Enabled    bool
	SMTPHost   string
	SMTPPort   int
	Username   string
	Password   string
	FromName   string
	FromEmail  string
	Recipients []string
}

// Config holds all notification configuration
type Config struct {
	Telegram TelegramConfig
	Email    EmailConfig
}
