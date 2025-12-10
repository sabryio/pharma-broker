package notify

import (
	"context"
	"fmt"
	"time"
)

// AlertSeverity represents the severity level of an alert.
type AlertSeverity string

const (
	AlertInfo     AlertSeverity = "info"
	AlertWarning  AlertSeverity = "warning"
	AlertCritical AlertSeverity = "critical"
)

// TelegramAlertAdapter adapts TelegramNotifier to the whatsapp.AlertNotifier interface.
type TelegramAlertAdapter struct {
	notifier *TelegramNotifier
}

// NewTelegramAlertAdapter creates a new alert adapter.
func NewTelegramAlertAdapter(notifier *TelegramNotifier) *TelegramAlertAdapter {
	return &TelegramAlertAdapter{notifier: notifier}
}

// SendAlert sends an alert via Telegram.
// Implements whatsapp.AlertNotifier interface.
func (a *TelegramAlertAdapter) SendAlert(ctx context.Context, severity, title, message string) error {
	if a.notifier == nil {
		return fmt.Errorf("telegram notifier not configured")
	}

	// Format alert with emoji based on severity
	emoji := getAlertEmoji(AlertSeverity(severity))
	timestamp := time.Now().Format("2006-01-02 15:04:05")

	formattedMessage := fmt.Sprintf(
		"%s <b>%s</b>\n\n%s\n\n<i>Time: %s</i>",
		emoji, title, message, timestamp,
	)

	return a.notifier.SendMessage(ctx, formattedMessage)
}

func getAlertEmoji(severity AlertSeverity) string {
	switch severity {
	case AlertCritical:
		return "🚨"
	case AlertWarning:
		return "⚠️"
	case AlertInfo:
		return "ℹ️"
	default:
		return "📢"
	}
}
